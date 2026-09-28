//! Root view: owns the current stage (empty → trim → processing → mixer),
//! polls background workers on a ~30 fps tick, and routes the space key.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::{ActiveTheme as _, Root, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::chords::Chord;

use crate::app_state::AppSettings;
use crate::controls::MixControls;
use crate::i18n::t;
use crate::naming;
use crate::player::Player;
use crate::settings::Appearance;
use crate::style;
use crate::timeline::SelectionError;
use crate::worker::{self, Decoded, SepMsg, Step};

mod export;
mod mixer;
mod processing;
mod trim;

pub use export::ExportProgress;

actions!(stemcraft, [TogglePlay]);

const KEY_CONTEXT: &str = "Stemcraft";

pub fn bind_keys(cx: &mut App) {
    // `!Input` lets text fields receive spaces.
    cx.bind_keys([KeyBinding::new("space", TogglePlay, Some("Stemcraft && !Input"))]);
}

// `Trim` is much larger than `Empty`, and the other stages (Processing,
// Mixer) are of similar size; boxing would just move the imbalance around,
// so it's accepted here rather than restructured.
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
    /// Selected part of the song, in seconds.
    pub range: (f64, f64),
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
    /// Selected part of the song, in seconds.
    pub range: (f64, f64),
    _subscriptions: Vec<Subscription>,
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
    _subscriptions: Vec<Subscription>,
    settings_changed: bool,
    active_device: Option<String>,
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
        let subscriptions = vec![
            // Handled on the next tick, which has a window for notifications.
            cx.observe_global::<AppSettings>(|this, cx| {
                this.settings_changed = true;
                cx.notify();
            }),
            cx.observe_window_appearance(window, |_, _, cx| {
                if AppSettings::get(cx).appearance == Appearance::System {
                    style::apply_appearance(cx);
                }
            }),
        ];
        let active_device = AppSettings::get(cx).output_device_id().map(str::to_string);
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
            _subscriptions: subscriptions,
            settings_changed: false,
            active_device,
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
        if std::mem::take(&mut self.settings_changed) {
            self.apply_settings(window, cx);
        }
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
                    Notification::error(format!("{}{}{msg}", t().open_failed, t().reason_sep)),
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
                let message = format!("{}{}{e}", t().no_audio_device, t().reason_sep);
                window.push_notification(Notification::error(message), cx);
            }
            None => {}
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Read first: global_mut would notify observers again.
        if cx.global::<AppSettings>().save_error.is_some()
            && let Some(error) = cx.global_mut::<AppSettings>().save_error.take()
        {
            let message = format!("{}{}{error}", t().settings_save_failed, t().reason_sep);
            window.push_notification(Notification::error(message), cx);
        }
        let device = AppSettings::get(cx).output_device_id().map(str::to_string);
        if device != self.active_device {
            self.active_device = device;
            self.rebuild_player(window, cx);
        }
    }

    /// Open a player on the output device chosen in the settings, telling the
    /// user when it had to fall back to the system default.
    pub(super) fn open_player(
        &self,
        tracks: Arc<Vec<StereoAudio>>,
        controls: Arc<MixControls>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Player, String> {
        let device = AppSettings::get(cx).output_device_id().map(str::to_string);
        let player = Player::new(tracks, controls, device.as_deref()).map_err(|e| format!("{e:#}"));
        if matches!(&player, Ok(p) if p.fell_back()) {
            window.push_notification(Notification::warning(t().device_fallback), cx);
        }
        player
    }

    /// Reopen the current player on the newly chosen device, keeping the
    /// position and play state.
    fn rebuild_player(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (old, tracks, controls) = match &self.stage {
            Stage::Trim(st) => (&st.player, st.source.clone(), Arc::new(MixControls::new(1))),
            Stage::Mixer(st) => (&st.player, st.stems.clone(), st.controls.clone()),
            _ => return,
        };
        let (position, playing) = old
            .as_ref()
            .map(|p| (p.position_frac(), p.is_playing()))
            .unwrap_or((0.0, false));
        if let Ok(old) = old {
            old.pause();
        }
        let player = self.open_player(tracks, controls, window, cx);
        if let Ok(p) = &player {
            p.seek_frac(position);
            if playing {
                p.toggle();
            }
        }
        match &mut self.stage {
            Stage::Trim(st) => st.player = player,
            Stage::Mixer(st) => st.player = player,
            _ => {}
        }
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // gpui-kit 0.6.6's Root doesn't draw these layers itself.
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let body = match &self.stage {
            Stage::Empty(_) => self.render_empty(cx).into_any_element(),
            Stage::Trim(_) => self.render_trim(window, cx).into_any_element(),
            Stage::Processing(_) => self.render_processing(cx).into_any_element(),
            Stage::Mixer(_) => self.render_mixer(cx).into_any_element(),
        };
        let theme = cx.theme();
        let (background, foreground) = (theme.background, theme.foreground);
        let toolbar = self.render_toolbar(window, cx).into_any_element();
        let status = self.render_status_bar(cx).into_any_element();
        div()
            .id("stemcraft")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::on_toggle_play))
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(foreground)
            .child(toolbar)
            .child(div().flex_1().min_h_0().child(body))
            .child(status)
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
