//! Stem separation with demucs-core (HTDemucs 6-stem) on the Metal GPU backend.

use std::sync::Once;

use anyhow::{bail, Context, Result};
use burn::backend::wgpu::{
    graphics::AutoGraphicsApi, init_setup, RuntimeOptions, Wgpu, WgpuDevice,
};
use cubecl::config::{autotune::AutotuneConfig, cache::CacheConfig, GlobalConfig};
use demucs_core::listener::{ForwardEvent, ForwardListener};
use demucs_core::{num_chunks, Demucs, ModelOptions};

use crate::audio::{sum_tracks, StereoAudio};

type B = Wgpu;

pub const GUITAR: &str = "guitar";

/// Forward-pass steps per chunk: 8 encoder + 1 transformer + 8 decoder + 1 denorm.
const STEPS_PER_CHUNK: usize = 18;

static GPU_INIT: Once = Once::new();

/// Loaded model. Inference recurses deeply, so run it on a thread with at least
/// an 8 MB stack (the macOS main thread qualifies; spawned threads default to 2 MB).
pub struct Separator {
    model: Demucs<B>,
}

pub struct Stems {
    pub tracks: Vec<(&'static str, StereoAudio)>,
}

impl Stems {
    pub fn get(&self, name: &str) -> Option<&StereoAudio> {
        self.tracks.iter().find(|(n, _)| *n == name).map(|(_, a)| a)
    }

    /// `(guitar, backing)` — backing is the unity-gain sum of every other stem.
    pub fn guitar_and_backing(&self) -> Result<(StereoAudio, StereoAudio)> {
        let Some(guitar) = self.get(GUITAR) else {
            bail!("model produced no guitar stem");
        };
        let others: Vec<&StereoAudio> = self
            .tracks
            .iter()
            .filter(|(n, _)| *n != GUITAR)
            .map(|(_, a)| a)
            .collect();
        Ok((guitar.clone(), sum_tracks(&others)?))
    }
}

impl Separator {
    pub fn new(weights: &[u8]) -> Result<Self> {
        let device = WgpuDevice::default();
        GPU_INIT.call_once(|| {
            // Persist autotune results so only the very first run pays for kernel tuning.
            GlobalConfig::set(GlobalConfig {
                autotune: AutotuneConfig {
                    cache: CacheConfig::Global,
                    ..Default::default()
                },
                ..Default::default()
            });
            init_setup::<AutoGraphicsApi>(
                &device,
                RuntimeOptions {
                    tasks_max: 128,
                    ..Default::default()
                },
            );
        });
        let model = Demucs::<B>::from_bytes(ModelOptions::SixStem, weights, device)
            .context("failed to load model weights")?;
        Ok(Self { model })
    }

    /// True until GPU kernels have been compiled and autotuned once on this machine.
    pub fn needs_warmup() -> bool {
        let dir = CacheConfig::Global.root().join("autotune");
        !std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_some())
    }

    pub fn warmup(&self) {
        pollster::block_on(self.model.warmup());
    }

    /// Separate `audio` into six stems at its own sample rate.
    /// `on_progress` receives the completed fraction in `0.0..=1.0`.
    pub fn separate(&self, audio: &StereoAudio, on_progress: impl FnMut(f32)) -> Result<Stems> {
        let samples_44k =
            (audio.len() as f64 * 44_100.0 / audio.sample_rate as f64).ceil() as usize;
        let mut listener = ProgressListener {
            done: 0,
            total: STEPS_PER_CHUNK * num_chunks(samples_44k),
            on_progress,
        };
        let stems = pollster::block_on(self.model.separate_with_listener(
            &audio.left,
            &audio.right,
            audio.sample_rate,
            &mut listener,
        ))?;
        let tracks = stems
            .into_iter()
            .map(|s| {
                let track = StereoAudio {
                    left: s.left,
                    right: s.right,
                    sample_rate: audio.sample_rate,
                };
                (s.id.as_str(), track)
            })
            .collect();
        Ok(Stems { tracks })
    }
}

struct ProgressListener<F> {
    done: usize,
    total: usize,
    on_progress: F,
}

impl<F: FnMut(f32)> ForwardListener for ProgressListener<F> {
    fn on_event(&mut self, event: ForwardEvent) {
        if matches!(
            event,
            ForwardEvent::EncoderDone { .. }
                | ForwardEvent::DecoderDone { .. }
                | ForwardEvent::TransformerDone { .. }
                | ForwardEvent::Denormalized
        ) {
            self.done += 1;
            (self.on_progress)((self.done as f32 / self.total as f32).min(1.0));
        }
    }
}
