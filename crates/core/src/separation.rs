//! Stem separation with demucs-core (HTDemucs 6-stem) on the Metal GPU backend.

use std::path::{Path, PathBuf};
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

/// Where cubecl keeps its persistent autotune results
/// (`~/Library/Application Support/autotune`). The folder isn't app-specific:
/// every app using cubecl's global autotune cache shares it.
pub fn gpu_cache_dir() -> PathBuf {
    CacheConfig::Global.root().join("autotune")
}

/// Total size in bytes of the GPU optimization cache (0 when absent).
pub fn gpu_cache_size() -> u64 {
    dir_size(&gpu_cache_dir())
}

/// Delete the GPU optimization cache; the next separation re-tunes.
pub fn clear_gpu_cache() -> Result<()> {
    remove_dir(&gpu_cache_dir())
}

/// True once this process has set up the GPU (via [`Separator::new`]).
/// Deleting the cache folder after that point can make cubecl re-tune from
/// scratch mid-session or spin forever locking a now-deleted folder, so
/// callers should defer the clear instead.
pub fn gpu_initialized() -> bool {
    GPU_INIT.is_completed()
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
            Ok(_) => entry.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

fn remove_dir(dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("cannot remove {}", dir.display())),
    }
}

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
        let dir = gpu_cache_dir();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_size_sums_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), [0u8; 10]).unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/b"), [0u8; 32]).unwrap();
        assert_eq!(dir_size(dir.path()), 42);
        assert_eq!(dir_size(&dir.path().join("missing")), 0);
    }

    #[test]
    fn remove_dir_deletes_and_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("autotune");
        std::fs::create_dir_all(target.join("0.9.0")).unwrap();
        std::fs::write(target.join("0.9.0/x.json.log"), b"x").unwrap();
        remove_dir(&target).unwrap();
        assert!(!target.exists());
        remove_dir(&target).unwrap();
    }

    #[test]
    fn gpu_cache_dir_is_cubecl_autotune_folder() {
        assert!(gpu_cache_dir().ends_with("autotune"));
    }
}
