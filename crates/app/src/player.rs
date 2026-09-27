//! Playback: a pure mixing `Engine` (tested without audio hardware) driven by a
//! cpal output stream. The stream always runs at the device's own sample rate —
//! requesting another rate would switch the device system-wide — so the engine
//! resamples on the fly with linear interpolation.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::mix::{self, TrackMix};

use crate::controls::MixControls;

const NO_SEEK: u64 = u64::MAX;

/// Play state shared between the UI and the audio callback.
pub struct Transport {
    playing: AtomicBool,
    /// Source frames, as f64 bits.
    position: AtomicU64,
    /// Pending seek in source frames (f64 bits), or NO_SEEK.
    seek: AtomicU64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            playing: AtomicBool::new(false),
            position: AtomicU64::new(0f64.to_bits()),
            seek: AtomicU64::new(NO_SEEK),
        }
    }
}

impl Transport {
    pub fn is_playing(&self) -> bool {
        self.playing.load(Relaxed)
    }

    pub fn set_playing(&self, on: bool) {
        self.playing.store(on, Relaxed);
    }

    /// Current position in source frames.
    pub fn position(&self) -> f64 {
        f64::from_bits(self.position.load(Relaxed))
    }

    fn set_position(&self, frames: f64) {
        self.position.store(frames.to_bits(), Relaxed);
    }

    /// Jump to `frames`; the position updates at once so a paused UI follows.
    pub fn request_seek(&self, frames: f64) {
        self.set_position(frames);
        self.seek.store(frames.to_bits(), Relaxed);
    }

    fn take_seek(&self) -> Option<f64> {
        let bits = self.seek.swap(NO_SEEK, Relaxed);
        (bits != NO_SEEK).then(|| f64::from_bits(bits))
    }
}

/// Mixes equally long tracks into interleaved device frames.
pub struct Engine {
    tracks: Arc<Vec<StereoAudio>>,
    controls: Arc<MixControls>,
    transport: Arc<Transport>,
    /// Source frames per device frame.
    step: f64,
    channels: usize,
    pos: f64,
    mix: Vec<TrackMix>,
    gains: Vec<f32>,
}

impl Engine {
    pub fn new(
        tracks: Arc<Vec<StereoAudio>>,
        controls: Arc<MixControls>,
        transport: Arc<Transport>,
        device_rate: u32,
        channels: usize,
    ) -> Self {
        let n = tracks.len();
        let step = tracks[0].sample_rate as f64 / device_rate as f64;
        Self {
            tracks,
            controls,
            transport,
            step,
            channels: channels.max(1),
            pos: 0.0,
            mix: Vec::with_capacity(n),
            gains: Vec::with_capacity(n),
        }
    }

    /// Fill one interleaved device buffer. Runs on the audio thread: no locks,
    /// no allocation.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        if let Some(frames) = self.transport.take_seek() {
            self.pos = frames;
        }
        if !self.transport.is_playing() {
            return;
        }
        self.controls.read_into(&mut self.mix);
        mix::gains_into(&self.mix, &mut self.gains);
        let len = self.tracks[0].len();
        for frame in out.chunks_mut(self.channels) {
            let i = self.pos as usize;
            if i >= len {
                self.transport.set_playing(false);
                break;
            }
            let next = (i + 1).min(len - 1);
            let t = (self.pos - i as f64) as f32;
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for (track, &gain) in self.tracks.iter().zip(&self.gains) {
                if gain == 0.0 {
                    continue;
                }
                l += gain * (track.left[i] + t * (track.left[next] - track.left[i]));
                r += gain * (track.right[i] + t * (track.right[next] - track.right[i]));
            }
            if self.channels == 1 {
                frame[0] = 0.5 * (l + r);
            } else {
                frame[0] = l;
                frame[1] = r;
            }
            self.pos += self.step;
        }
        self.transport.set_position(self.pos.min(len as f64));
    }
}

/// Owns the output stream; dropping the player stops playback.
pub struct Player {
    _stream: cpal::Stream,
    transport: Arc<Transport>,
    frames: usize,
    sample_rate: u32,
}

impl Player {
    pub fn new(tracks: Arc<Vec<StereoAudio>>, controls: Arc<MixControls>) -> Result<Self> {
        let frames = tracks[0].len();
        let sample_rate = tracks[0].sample_rate;
        let transport = Arc::new(Transport::default());

        let device = cpal::default_host()
            .default_output_device()
            .context("no audio output device")?;
        let supported = device
            .default_output_config()
            .context("cannot read the output device's configuration")?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            bail!("unsupported output format {}", supported.sample_format());
        }
        let config = supported.config();
        let mut engine = Engine::new(
            tracks,
            controls,
            transport.clone(),
            config.sample_rate,
            config.channels as usize,
        );
        let stream = device.build_output_stream::<f32, _, _>(
            config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| engine.render(out),
            |e| eprintln!("audio output error: {e}"),
            None,
        )?;
        // cpal 0.18 creates streams paused. The engine outputs silence while paused.
        stream.play()?;
        Ok(Self {
            _stream: stream,
            transport,
            frames,
            sample_rate,
        })
    }

    pub fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    /// Play from the current position, restarting if playback reached the end.
    pub fn toggle(&self) {
        if self.is_playing() {
            self.transport.set_playing(false);
        } else {
            if self.transport.position() >= self.frames.saturating_sub(1) as f64 {
                self.transport.request_seek(0.0);
            }
            self.transport.set_playing(true);
        }
    }

    pub fn pause(&self) {
        self.transport.set_playing(false);
    }

    pub fn seek_frac(&self, frac: f32) {
        self.transport
            .request_seek(frac.clamp(0.0, 1.0) as f64 * self.frames as f64);
    }

    pub fn position_frac(&self) -> f32 {
        if self.frames == 0 {
            return 0.0;
        }
        (self.transport.position() / self.frames as f64).min(1.0) as f32
    }

    pub fn position_secs(&self) -> f64 {
        self.transport.position() / self.sample_rate as f64
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames as f64 / self.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(left: Vec<f32>, right: Vec<f32>, sample_rate: u32) -> StereoAudio {
        StereoAudio { left, right, sample_rate }
    }

    fn engine(tracks: Vec<StereoAudio>, device_rate: u32, channels: usize) -> (Engine, Arc<MixControls>, Arc<Transport>) {
        let controls = Arc::new(MixControls::new(tracks.len()));
        let transport = Arc::new(Transport::default());
        let e = Engine::new(Arc::new(tracks), controls.clone(), transport.clone(), device_rate, channels);
        (e, controls, transport)
    }

    #[test]
    fn silent_while_paused() {
        let (mut e, _, _) = engine(vec![track(vec![1.0; 4], vec![1.0; 4], 4)], 4, 2);
        let mut out = vec![9.0; 4];
        e.render(&mut out);
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn mixes_audible_tracks_into_interleaved_stereo() {
        let a = track(vec![1.0, 2.0, 3.0], vec![-1.0, -2.0, -3.0], 4);
        let b = track(vec![10.0, 20.0, 30.0], vec![10.0, 20.0, 30.0], 4);
        let (mut e, controls, transport) = engine(vec![a, b], 4, 2);
        controls.set_volume(1, 0.5);
        transport.set_playing(true);
        let mut out = vec![0.0; 4];
        e.render(&mut out);
        assert_eq!(out, vec![6.0, 4.0, 12.0, 8.0]);
        controls.set_mute(1, true);
        let mut out = vec![0.0; 2];
        e.render(&mut out);
        assert_eq!(out, vec![3.0, -3.0]);
    }

    #[test]
    fn interpolates_when_the_device_runs_faster() {
        let (mut e, _, transport) = engine(vec![track(vec![0.0, 2.0], vec![0.0, 2.0], 2)], 4, 1);
        transport.set_playing(true);
        let mut out = vec![0.0; 4];
        e.render(&mut out);
        assert_eq!(out, vec![0.0, 1.0, 2.0, 2.0]);
        assert_eq!(transport.position(), 2.0);
    }

    #[test]
    fn stops_at_the_end() {
        let (mut e, _, transport) = engine(vec![track(vec![1.0, 1.0], vec![1.0, 1.0], 4)], 4, 2);
        transport.set_playing(true);
        let mut out = vec![0.0; 8];
        e.render(&mut out);
        assert_eq!(out, vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(!transport.is_playing());
    }

    #[test]
    fn applies_a_seek_before_rendering() {
        let (mut e, _, transport) = engine(vec![track(vec![0.0, 1.0, 2.0, 3.0], vec![0.0; 4], 4)], 4, 2);
        transport.request_seek(2.0);
        assert_eq!(transport.position(), 2.0);
        transport.set_playing(true);
        let mut out = vec![0.0; 2];
        e.render(&mut out);
        assert_eq!(out[0], 2.0);
    }
}
