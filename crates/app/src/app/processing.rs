//! Processing stage: runs (and retries) the separation worker, then hands
//! its result off to the mixer stage.

use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::*;
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::separation::Separator;

use crate::controls::MixControls;
use crate::i18n::t;
use crate::worker::{self, SepMsg, Separated, Step};

use super::{AppView, MixerState, ProcessingState, Stage};

impl AppView {
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        let Stage::Processing(st) = &self.stage else { return };
        if st.error.is_none() { return; }
        let (path, song, audio) = (st.path.clone(), st.song.clone(), st.audio.clone());
        self.begin_processing(path, song, audio, cx);
    }

    pub(super) fn begin_processing(&mut self, path: PathBuf, song: String, audio: Arc<StereoAudio>, cx: &mut Context<Self>) {
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

    pub(super) fn poll_separation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        // The `if let` below returns before using `st` again, so `st`'s borrow
        // of `self.stage` has ended by the time `enter_mixer` runs, letting it
        // take `&mut self` on this path.
        if let Some((path, song, result)) = done {
            self.enter_mixer(path, song, result, window, cx);
            return;
        }
        st.rx = Some(rx);
        if changed {
            cx.notify();
        }
    }

    pub(super) fn enter_mixer(&mut self, path: PathBuf, song: String, result: Separated, window: &mut Window, cx: &mut Context<Self>) {
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
        let player = self.open_player(stems.clone(), controls.clone(), window, cx);
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
}
