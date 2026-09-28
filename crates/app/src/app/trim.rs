//! Trim stage: waveform selection, the start/end time fields, and kicking
//! off separation once a selection is confirmed.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;

use crate::controls::MixControls;
use crate::naming;
use crate::timeline;
use crate::worker::Decoded;

use super::{selection_message, AppView, Stage, TrimState};

impl AppView {
    pub(super) fn enter_trim(&mut self, path: PathBuf, decoded: Decoded, window: &mut Window, cx: &mut Context<Self>) {
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
        let player = self.open_player(source.clone(), Arc::new(MixControls::new(1)), window, cx);
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
    pub(super) fn apply_time_inputs(&mut self, cx: &mut Context<Self>) {
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
        let range = (timeline::frac_to_secs(st.selection.0, duration), timeline::frac_to_secs(st.selection.1, duration));
        let mut audio = st.source[0].clone();
        audio.trim(timeline::selection_to_range(st.selection, duration));
        let (path, song) = (st.path.clone(), st.song.clone());
        self.begin_processing(path, song, Arc::new(audio), range, cx);
    }
}
