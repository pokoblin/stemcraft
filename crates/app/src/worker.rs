//! Background threads for decoding and separation. Each reports over an mpsc
//! channel that the UI drains on its tick.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

use anyhow::{Context, Result};
use stemcraft_core::audio::{self, StereoAudio};
use stemcraft_core::chords::{detect_chords, Chord, ChordOptions};
use stemcraft_core::separation::{Separator, Stems};
use stemcraft_core::weights;

use crate::waveform::{peaks, PEAK_BUCKETS};

/// Display order of the six stems (ids as produced by the model).
pub const STEM_IDS: [&str; 6] = ["drums", "bass", "vocals", "piano", "guitar", "other"];
const GUITAR_INDEX: usize = 4;

/// Model inference recurses deeply; spawned threads default to 2 MB.
const SEPARATION_STACK: usize = 16 << 20;

pub struct Decoded {
    pub audio: StereoAudio,
    pub peaks: Arc<[(f32, f32)]>,
}

/// Turns a `catch_unwind` panic payload into a readable message: the common
/// payload types (a `&str` or `String`, from `panic!("...")`) downcast
/// directly, anything else falls back to a generic message.
pub(crate) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

pub fn spawn_decode(path: PathBuf) -> mpsc::Receiver<Result<Decoded, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            audio::decode(&path)
                .map(|audio| Decoded {
                    peaks: peaks(&audio, PEAK_BUCKETS).into(),
                    audio,
                })
                .map_err(|e| format!("{e:#}"))
        }))
        .unwrap_or_else(|payload| Err(panic_message(payload)));
        let _ = tx.send(result);
    });
    rx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    LoadModel,
    Warmup,
    Separate,
    Chords,
}

/// The steps shown on the processing page.
pub fn steps(warmup: bool) -> Vec<Step> {
    let mut all = vec![Step::LoadModel];
    if warmup {
        all.push(Step::Warmup);
    }
    all.extend([Step::Separate, Step::Chords]);
    all
}

pub struct Separated {
    /// In `STEM_IDS` order, like `peaks`.
    pub stems: Vec<StereoAudio>,
    pub peaks: Vec<Arc<[(f32, f32)]>>,
    pub chords: Vec<Chord>,
}

pub enum SepMsg {
    Step(Step),
    /// Fraction of the separation step, `0.0..=1.0`.
    Progress(f32),
    Done(Box<Separated>),
    Failed(String),
}

pub fn spawn_separation(audio: Arc<StereoAudio>) -> mpsc::Receiver<SepMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("separation".into())
        .stack_size(SEPARATION_STACK)
        .spawn(move || match panic::catch_unwind(AssertUnwindSafe(|| separate(&audio, &tx))) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let _ = tx.send(SepMsg::Failed(format!("{e:#}")));
            }
            Err(payload) => {
                let _ = tx.send(SepMsg::Failed(panic_message(payload)));
            }
        })
        .expect("cannot spawn the separation thread");
    rx
}

fn separate(audio: &StereoAudio, tx: &Sender<SepMsg>) -> Result<()> {
    let send = |msg: SepMsg| {
        let _ = tx.send(msg);
    };
    send(SepMsg::Step(Step::LoadModel));
    let weights = weights::load(|_, _| {})?;
    let separator = Separator::new(&weights)?;
    drop(weights);
    if Separator::needs_warmup() {
        send(SepMsg::Step(Step::Warmup));
        separator.warmup();
    }
    send(SepMsg::Step(Step::Separate));
    let stems = order_stems(separator.separate(audio, |p| send(SepMsg::Progress(p)))?)?;

    send(SepMsg::Step(Step::Chords));
    let guitar = &stems[GUITAR_INDEX];
    let chords = detect_chords(&guitar.to_mono(), guitar.sample_rate, &ChordOptions::default());
    let peaks = stems.iter().map(|s| peaks(s, PEAK_BUCKETS).into()).collect();
    send(SepMsg::Done(Box::new(Separated { stems, peaks, chords })));
    Ok(())
}

fn order_stems(mut stems: Stems) -> Result<Vec<StereoAudio>> {
    STEM_IDS
        .iter()
        .map(|id| {
            let i = stems
                .tracks
                .iter()
                .position(|(name, _)| name == id)
                .with_context(|| format!("the model produced no {id} stem"))?;
            Ok(stems.tracks.swap_remove(i).1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn mono(v: f32) -> StereoAudio {
        StereoAudio { left: vec![v], right: vec![v], sample_rate: 44_100 }
    }

    #[test]
    fn orders_stems_for_display() {
        let model_order = ["drums", "bass", "other", "vocals", "guitar", "piano"];
        let stems = Stems {
            tracks: model_order.iter().enumerate().map(|(i, id)| (*id, mono(i as f32))).collect(),
        };
        let firsts: Vec<f32> = order_stems(stems).unwrap().iter().map(|s| s.left[0]).collect();
        assert_eq!(firsts, vec![0.0, 1.0, 3.0, 5.0, 4.0, 2.0]);
    }

    #[test]
    fn reports_a_missing_stem() {
        let stems = Stems { tracks: vec![("drums", mono(0.0))] };
        assert!(order_stems(stems).is_err());
    }

    #[test]
    fn guitar_index_matches_core() {
        assert_eq!(STEM_IDS[GUITAR_INDEX], stemcraft_core::separation::GUITAR);
    }

    #[test]
    fn warmup_step_only_when_needed() {
        assert_eq!(steps(false), vec![Step::LoadModel, Step::Separate, Step::Chords]);
        assert_eq!(steps(true), vec![Step::LoadModel, Step::Warmup, Step::Separate, Step::Chords]);
    }

    #[test]
    fn decode_worker_returns_audio_and_peaks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let audio = StereoAudio { left: vec![0.5; 100], right: vec![-0.5; 100], sample_rate: 8000 };
        stemcraft_core::audio::write_wav(&path, &audio).unwrap();
        let decoded = spawn_decode(path).recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        assert_eq!(decoded.audio, audio);
        assert_eq!(decoded.peaks.len(), 100);
    }

    #[test]
    fn decode_worker_reports_errors() {
        let result = spawn_decode("/nonexistent/x.wav".into())
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn panic_message_downcasts_known_payload_types() {
        let str_payload: Box<dyn std::any::Any + Send> = Box::new("boom");
        assert_eq!(panic_message(str_payload), "boom");

        let string_payload: Box<dyn std::any::Any + Send> = Box::new(String::from("kaboom"));
        assert_eq!(panic_message(string_payload), "kaboom");

        let other_payload: Box<dyn std::any::Any + Send> = Box::new(42i32);
        assert_eq!(panic_message(other_payload), "unknown panic");
    }
}
