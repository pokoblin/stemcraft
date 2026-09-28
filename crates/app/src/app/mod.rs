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
use gpui_kit::component::{ActiveTheme as _, Root, Theme, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::chords::Chord;

use crate::controls::MixControls;
use crate::i18n::t;
use crate::naming;
use crate::player::Player;
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
