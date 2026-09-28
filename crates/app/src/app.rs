//! Root view: owns the current stage (empty → trim → processing → mixer),
//! polls background workers on a ~30 fps tick, and routes the space key.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme as _, Root, Sizable as _, Theme, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::chords::Chord;
use stemcraft_core::mix;
use stemcraft_core::separation::Separator;

use crate::controls::MixControls;
use crate::export_job::{self, ExportJob, ExportMsg};
use crate::i18n::t;
use crate::naming;
use crate::player::Player;
use crate::timeline::{self, SelectionError};
use crate::views::export_dialog::{ExportDialog, ExportRequest};
use crate::worker::{self, Decoded, SepMsg, Separated, Step};

actions!(stemcraft, [TogglePlay]);

const KEY_CONTEXT: &str = "Stemcraft";

pub fn bind_keys(cx: &mut App) {
    // `!Input` lets text fields receive spaces.
    cx.bind_keys([KeyBinding::new("space", TogglePlay, Some("Stemcraft && !Input"))]);
}

// `Trim` is much larger than `Empty`, and Task 10 adds sibling stages of
// similar size (Processing, Mixer); boxing would just move the imbalance
// around, so it's accepted here rather than restructured.
#[allow(clippy::large_enum_variant)]
pub enum Stage {
    Empty(EmptyState),
    Trim(TrimState),
    Processing(ProcessingState),
    Mixer(MixerState),
}

#[derive(Default)]
pub struct EmptyState {
    /// File being decoded.
    pub loading: Option<(PathBuf, mpsc::Receiver<Result<Decoded, String>>)>,
}

pub struct TrimState {
    pub path: PathBuf,
    pub song: String,
    /// One track: the original song (the player's input).
    pub source: Arc<Vec<StereoAudio>>,
    pub peaks: Arc<[(f32, f32)]>,
    pub selection: (f32, f32),
    pub anchor: Rc<Cell<Option<f32>>>,
    pub start_input: Entity<InputState>,
    pub end_input: Entity<InputState>,
    pub input_error: Option<SharedString>,
    pub player: Result<Player, String>,
    /// The start/end text the app itself last wrote into `start_input` /
    /// `end_input` (initial defaults, or a prior drag/parse). Lets
    /// `apply_time_inputs` tell an untouched Blur/Enter (or a click that
    /// commits nothing) apart from an actual edit, so it never re-parses —
    /// and truncates — a selection the user didn't touch.
    last_written: (String, String),
    _subscriptions: Vec<Subscription>,
}

impl TrimState {
    pub fn duration(&self) -> f64 {
        self.source[0].duration_secs()
    }
}

pub struct ProcessingState {
    pub path: PathBuf,
    pub song: String,
    /// The trimmed audio, kept for retries.
    pub audio: Arc<StereoAudio>,
    pub warmup: bool,
    pub step: Step,
    pub progress: f32,
    pub error: Option<String>,
    pub rx: Option<mpsc::Receiver<SepMsg>>,
}

pub struct MixerState {
    pub path: PathBuf,
    pub song: String,
    /// In `worker::STEM_IDS` order, like every per-track vector here.
    pub stems: Arc<Vec<StereoAudio>>,
    pub peaks: Vec<Arc<[(f32, f32)]>>,
    pub chords: Arc<Vec<Chord>>,
    pub controls: Arc<MixControls>,
    pub player: Result<Player, String>,
    pub sliders: Vec<Entity<SliderState>>,
    _subscriptions: Vec<Subscription>,
}

pub struct ExportProgress {
    pub rx: mpsc::Receiver<ExportMsg>,
    pub done: usize,
    pub total: usize,
}

impl MixerState {
    pub fn duration(&self) -> f64 {
        self.stems[0].duration_secs()
    }
}

pub struct AppView {
    focus: FocusHandle,
    pub stage: Stage,
    /// Export progress, kept here (not in `MixerState`) so it survives a move
    /// to another stage — e.g. "Open another song" while an export is running.
    pub export: Option<ExportProgress>,
    was_playing: bool,
    _tick: Task<()>,
    _appearance: Subscription,
}

fn selection_message(error: SelectionError) -> &'static str {
    match error {
        SelectionError::BadFormat => t().bad_time,
        SelectionError::TooShort => t().too_short,
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });
        let tick = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(33)).await;
            if this.update_in(cx, |this, window, cx| this.on_tick(window, cx)).is_err() {
                break;
            }
        });
        Self {
            focus,
            stage: Stage::Empty(EmptyState::default()),
            export: None,
            was_playing: false,
            _tick: tick,
            _appearance: appearance,
        }
    }

    fn player(&self) -> Option<&Result<Player, String>> {
        match &self.stage {
            Stage::Trim(st) => Some(&st.player),
            Stage::Mixer(st) => Some(&st.player),
            _ => None,
        }
    }

    fn on_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let playing = matches!(self.player(), Some(Ok(p)) if p.is_playing());
        // Keep redrawing while playing, plus one frame after it stops.
        if playing || self.was_playing {
            cx.notify();
        }
        self.was_playing = playing;
        self.poll_decode(window, cx);
        self.poll_separation(window, cx);
        self.poll_export(window, cx);
    }

    fn poll_decode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Stage::Empty(state) = &mut self.stage else { return };
        let Some((path, rx)) = &state.loading else { return };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(t().worker_stopped.to_string()),
        };
        let path = path.clone();
        state.loading = None;
        match result {
            Ok(decoded) => self.enter_trim(path, decoded, window, cx),
            Err(msg) => {
                window.push_notification(
                    Notification::error(format!("{}: {msg}", t().open_failed)),
                    cx,
                );
                cx.notify();
            }
        }
    }

    pub fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t().choose_file.into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = rx.await else { return };
            let Some(path) = paths.pop() else { return };
            this.update_in(cx, |this, window, cx| this.load_file(path, window, cx))
                .ok();
        })
        .detach();
    }

    pub fn load_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // The open panel has no type filter, so check here.
        if !naming::is_supported_audio(&path) {
            window.push_notification(Notification::error(t().unsupported_file), cx);
            return;
        }
        let rx = worker::spawn_decode(path.clone());
        self.stage = Stage::Empty(EmptyState {
            loading: Some((path, rx)),
        });
        cx.notify();
    }

    pub fn back_to_empty(&mut self, cx: &mut Context<Self>) {
        self.stage = Stage::Empty(EmptyState::default());
        cx.notify();
    }

    fn enter_trim(&mut self, path: PathBuf, decoded: Decoded, window: &mut Window, cx: &mut Context<Self>) {
        let duration = decoded.audio.duration_secs();
        let initial_start = "0:00".to_string();
        let initial_end = timeline::format_clock(duration);
        let start_input = cx.new(|cx| InputState::new(window, cx).default_value(initial_start.clone()));
        let end_input = cx.new(|cx| InputState::new(window, cx).default_value(initial_end.clone()));
        let subscriptions = [&start_input, &end_input]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.apply_time_inputs(cx);
                    }
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.focus.focus(window, cx);
                    }
                })
            })
            .collect();
        let source = Arc::new(vec![decoded.audio]);
        let player = Player::new(source.clone(), Arc::new(MixControls::new(1)))
            .map_err(|e| format!("{e:#}"));
        self.stage = Stage::Trim(TrimState {
            song: naming::song_name(&path),
            path,
            source,
            peaks: decoded.peaks,
            selection: (0.0, 1.0),
            anchor: Rc::new(Cell::new(None)),
            start_input,
            end_input,
            input_error: None,
            player,
            last_written: (initial_start, initial_end),
            _subscriptions: subscriptions,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Selection dragged on the waveform: mirror it into the time fields.
    pub fn set_selection(&mut self, selection: (f32, f32), window: &mut Window, cx: &mut Context<Self>) {
        let Stage::Trim(st) = &mut self.stage else { return };
        st.selection = selection;
        st.input_error = None;
        let duration = st.duration();
        let start = timeline::format_clock(timeline::frac_to_secs(selection.0, duration));
        let end = timeline::format_clock(timeline::frac_to_secs(selection.1, duration));
        st.start_input.update(cx, |s, cx| s.set_value(start.clone(), window, cx));
        st.end_input.update(cx, |s, cx| s.set_value(end.clone(), window, cx));
        st.last_written = (start, end);
        cx.notify();
    }

    /// Apply the trim page's time fields to the selection, parsing only
    /// whichever field(s) the user actually edited since the app last wrote
    /// into them — see `TrimState::last_written`. A no-op Blur/Enter (or a
    /// button click that blurs nothing) must never re-parse and truncate an
    /// untouched field to whole seconds, and reverting a bad edit back to
    /// the remembered text must clear the error rather than leave it stuck
    /// forever (the fields being unchanged means the current selection is
    /// already valid, by construction).
    fn apply_time_inputs(&mut self, cx: &mut Context<Self>) {
        let Stage::Trim(st) = &mut self.stage else { return };
        let start = st.start_input.read(cx).value().to_string();
        let end = st.end_input.read(cx).value().to_string();
        let last_written = (st.last_written.0.as_str(), st.last_written.1.as_str());
        let duration = st.duration();
        match timeline::edited_selection(&start, &end, last_written, st.selection, duration) {
            Ok(selection) => {
                st.selection = selection;
                st.input_error = None;
                st.last_written = (start, end);
            }
            Err(e) => st.input_error = Some(selection_message(e).into()),
        }
        cx.notify();
    }

    pub fn seek(&mut self, frac: f32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Ok(player)) = self.player() {
            player.seek_frac(frac);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn on_toggle_play(&mut self, _: &TogglePlay, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_play(window, cx);
    }

    pub fn toggle_play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.player() {
            Some(Ok(player)) => player.toggle(),
            Some(Err(e)) => {
                let message = format!("{}: {e}", t().no_audio_device);
                window.push_notification(Notification::error(message), cx);
            }
            None => {}
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn start_separation(&mut self, cx: &mut Context<Self>) {
        // A Button's mouse-down calls prevent_default, so clicking this
        // button never blurs a focused time input — apply whatever the user
        // typed here first, or a stale selection would be used.
        self.apply_time_inputs(cx);
        let Stage::Trim(st) = &mut self.stage else { return };
        if st.input_error.is_some() {
            return;
        }
        let duration = st.duration();
        if let Err(e) = timeline::check_selection(st.selection, duration) {
            st.input_error = Some(selection_message(e).into());
            cx.notify();
            return;
        }
        let mut audio = st.source[0].clone();
        audio.trim(timeline::selection_to_range(st.selection, duration));
        let (path, song) = (st.path.clone(), st.song.clone());
        self.begin_processing(path, song, Arc::new(audio), cx);
    }

    pub fn retry(&mut self, cx: &mut Context<Self>) {
        let Stage::Processing(st) = &self.stage else { return };
        if st.error.is_none() { return; }
        let (path, song, audio) = (st.path.clone(), st.song.clone(), st.audio.clone());
        self.begin_processing(path, song, audio, cx);
    }

    fn begin_processing(&mut self, path: PathBuf, song: String, audio: Arc<StereoAudio>, cx: &mut Context<Self>) {
        let rx = worker::spawn_separation(audio.clone());
        // Replacing the stage drops the trim page's player, which stops playback.
        self.stage = Stage::Processing(ProcessingState {
            path,
            song,
            audio,
            warmup: Separator::needs_warmup(),
            step: Step::LoadModel,
            progress: 0.0,
            error: None,
            rx: Some(rx),
        });
        cx.notify();
    }

    fn poll_separation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Stage::Processing(st) = &mut self.stage else { return };
        let Some(rx) = st.rx.take() else { return };
        let mut changed = false;
        let mut done = None;
        loop {
            match rx.try_recv() {
                Ok(SepMsg::Step(step)) => {
                    st.step = step;
                    changed = true;
                }
                Ok(SepMsg::Progress(p)) => {
                    st.progress = p;
                    changed = true;
                }
                Ok(SepMsg::Done(result)) => {
                    done = Some((st.path.clone(), st.song.clone(), *result));
                    break;
                }
                Ok(SepMsg::Failed(e)) => {
                    st.error = Some(e);
                    cx.notify();
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    st.error = Some(t().worker_stopped.to_string());
                    cx.notify();
                    return;
                }
            }
        }
        // `st`'s borrow of `self.stage` ends when the `if let` below executes and
        // returns (at line 379), since `st` is not used after that return.
        // This lets `enter_mixer` below take `&mut self` on this path.
        if let Some((path, song, result)) = done {
            self.enter_mixer(path, song, result, window, cx);
            return;
        }
        st.rx = Some(rx);
        if changed {
            cx.notify();
        }
    }

    fn enter_mixer(&mut self, path: PathBuf, song: String, result: Separated, window: &mut Window, cx: &mut Context<Self>) {
        let stems = Arc::new(result.stems);
        let controls = Arc::new(MixControls::new(stems.len()));
        let mut sliders = Vec::with_capacity(stems.len());
        let mut subscriptions = Vec::with_capacity(stems.len());
        for i in 0..stems.len() {
            let slider = cx.new(|_| {
                SliderState::new().min(0.).max(100.).step(1.).default_value(100.)
            });
            let controls = controls.clone();
            subscriptions.push(cx.subscribe_in(&slider, window, move |_, _, event: &SliderEvent, _, cx| {
                let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                controls.set_volume(i, value.end() / 100.0);
                cx.notify();
            }));
            sliders.push(slider);
        }
        let player = Player::new(stems.clone(), controls.clone()).map_err(|e| format!("{e:#}"));
        self.stage = Stage::Mixer(MixerState {
            path,
            song,
            stems,
            peaks: result.peaks,
            chords: Arc::new(result.chords),
            controls,
            player,
            sliders,
            _subscriptions: subscriptions,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn toggle_mute(&mut self, track: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Mixer(st) = &self.stage {
            st.controls.set_mute(track, !st.controls.get(track).mute);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn toggle_solo(&mut self, track: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Mixer(st) = &self.stage {
            st.controls.set_solo(track, !st.controls.get(track).solo);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn open_export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Stage::Mixer(st) = &self.stage else { return };
        let audible = mix::gains(&st.controls.snapshot()).iter().map(|g| *g > 0.0).collect();
        let dest = st.path.parent().map(PathBuf::from).unwrap_or_default();
        let options = cx.new(|_| ExportDialog::new(dest, audible));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let s = t();
            let options_for_ok = options.clone();
            let view = view.clone();
            dialog
                .title(s.export_title)
                .w(px(500.))
                .child(options.clone())
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(Button::new("export-cancel").outline().label(s.cancel)))
                        .child(DialogAction::new().child(Button::new("export-confirm").primary().label(s.export_button))),
                )
                .on_ok(move |_, _, cx| {
                    let request = options_for_ok.read(cx).request();
                    view.update(cx, |this, cx| this.start_export(request, cx)).ok();
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }

    pub fn start_export(&mut self, request: ExportRequest, cx: &mut Context<Self>) {
        if self.export.is_some() {
            return;
        }
        let Stage::Mixer(st) = &mut self.stage else { return };
        let ids: Vec<&str> = request.stems.iter().map(|&i| worker::STEM_IDS[i]).collect();
        let plan = naming::plan_export(&request.dest, &st.song, request.format.extension(), &ids, request.chords);
        let job = ExportJob {
            plan,
            format: request.format,
            stems: st.stems.clone(),
            gains: mix::gains(&st.controls.snapshot()),
            stem_indices: request.stems,
            chords: st.chords.clone(),
            title: st.song.clone(),
        };
        let total = job.total();
        self.export = Some(ExportProgress {
            rx: export_job::spawn(job),
            done: 0,
            total,
        });
        cx.notify();
    }

    /// Polled every tick regardless of stage, so a stage change (e.g. "Open
    /// another song") while an export runs doesn't drop its Done/Failed
    /// notification.
    fn poll_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(progress) = self.export.as_mut() else { return };
        let finished = loop {
            match progress.rx.try_recv() {
                Ok(ExportMsg::Progress { done, total }) => {
                    progress.done = done;
                    progress.total = total;
                    cx.notify();
                }
                Ok(ExportMsg::Done(dir)) => break Ok(dir),
                Ok(ExportMsg::Failed(e)) => break Err(e),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => break Err(t().worker_stopped.to_string()),
            }
        };
        self.export = None;
        match finished {
            Ok(dir) => notify_export_done(dir, window, cx),
            Err(e) => window.push_notification(
                Notification::error(format!("{}: {e}", t().export_failed)),
                cx,
            ),
        }
        cx.notify();
    }

    /// Opening another song discards the stems, so confirm first.
    pub fn confirm_open_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let s = t();
            alert
                .confirm()
                .title(s.confirm_discard_title)
                .description(s.confirm_discard_body)
                // button_props replaces all props including show_cancel, so set it before the handlers
                // and add show_cancel(true) to restore the Cancel button.
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(s.confirm_open)
                        .cancel_text(s.cancel)
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    // Don't discard the stems until a file is actually chosen —
                    // `load_file` replaces the stage once one is (or reports an
                    // unsupported type and leaves the mixer); cancelling the
                    // picker must leave the current stems in place.
                    view.update(cx, |this, cx| this.open_file_dialog(window, cx)).ok();
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }
}

fn notify_export_done(dir: PathBuf, window: &mut Window, cx: &mut App) {
    let message = dir.display().to_string();
    window.push_notification(
        Notification::new()
            .title(t().export_done)
            .message(message)
            .action(move |_, _, cx| {
                let dir = dir.clone();
                Button::new("reveal-export")
                    .primary()
                    .small()
                    .label(t().reveal)
                    .on_click(cx.listener(move |note, _, window, cx| {
                        cx.reveal_path(&dir);
                        note.dismiss(window, cx);
                    }))
            }),
        cx,
    );
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // gpui-kit 0.6.6's Root doesn't draw these layers itself.
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let body = match &self.stage {
            Stage::Empty(_) => self.render_empty(cx).into_any_element(),
            Stage::Trim(_) => self.render_trim(cx).into_any_element(),
            Stage::Processing(_) => self.render_processing(cx).into_any_element(),
            Stage::Mixer(_) => self.render_mixer(cx).into_any_element(),
        };
        let theme = cx.theme();
        let (background, foreground) = (theme.background, theme.foreground);
        div()
            .id("stemcraft")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::on_toggle_play))
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(body)
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
