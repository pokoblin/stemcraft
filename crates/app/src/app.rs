//! Root view: owns the current stage (empty → trim → processing → mixer),
//! polls background workers on a ~30 fps tick, and routes the space key.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Root, Theme, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::audio::StereoAudio;

use crate::controls::MixControls;
use crate::i18n::t;
use crate::naming;
use crate::player::Player;
use crate::timeline::{self, SelectionError};
use crate::worker::{self, Decoded};

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
    _subscriptions: Vec<Subscription>,
}

impl TrimState {
    pub fn duration(&self) -> f64 {
        self.source[0].duration_secs()
    }
}

pub struct AppView {
    focus: FocusHandle,
    pub stage: Stage,
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
            was_playing: false,
            _tick: tick,
            _appearance: appearance,
        }
    }

    fn player(&self) -> Option<&Result<Player, String>> {
        match &self.stage {
            Stage::Trim(st) => Some(&st.player),
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
        let start_input = cx.new(|cx| InputState::new(window, cx).default_value("0:00"));
        let end_input =
            cx.new(|cx| InputState::new(window, cx).default_value(timeline::format_clock(duration)));
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
        st.start_input.update(cx, |s, cx| s.set_value(start, window, cx));
        st.end_input.update(cx, |s, cx| s.set_value(end, window, cx));
        cx.notify();
    }

    fn apply_time_inputs(&mut self, cx: &mut Context<Self>) {
        let Stage::Trim(st) = &mut self.stage else { return };
        let start = st.start_input.read(cx).value().to_string();
        let end = st.end_input.read(cx).value().to_string();
        match timeline::parse_selection(&start, &end, st.duration()) {
            Ok(selection) => {
                st.selection = selection;
                st.input_error = None;
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
