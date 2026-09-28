//! Playback: a pure mixing `Engine` (tested without audio hardware) driven by a
//! cpal output stream. The stream always runs at the device's own sample rate —
//! requesting another rate would switch the device system-wide — so the engine
//! resamples on the fly with linear interpolation.

use std::str::FromStr;
use std::sync::atomic::{
    AtomicBool, AtomicU64,
    Ordering::{Acquire, Relaxed, Release},
};
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
    /// Acquire, paired with `set_playing`'s Release: once this observes `true`,
    /// a seek requested (Relaxed) before that `set_playing` call is guaranteed
    /// visible to a subsequent `take_seek`.
    pub fn is_playing(&self) -> bool {
        self.playing.load(Acquire)
    }

    /// Release, paired with `is_playing`'s Acquire (see there).
    pub fn set_playing(&self, on: bool) {
        self.playing.store(on, Release);
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

    /// True if a seek has been requested but not yet applied by `render`.
    fn seek_pending(&self) -> bool {
        self.seek.load(Relaxed) != NO_SEEK
    }
}

/// Validates that `tracks` is safe for `Engine`/`Player` to index and iterate
/// over in lockstep: non-empty, every track's left and right channels the same
/// length, every track the same length as the others, and every track sharing
/// one non-zero sample rate. Not re-checked on the audio thread.
fn check_tracks(tracks: &[StereoAudio]) -> Result<()> {
    let Some(first) = tracks.first() else {
        bail!("no tracks to play");
    };
    let (len, sample_rate) = (first.len(), first.sample_rate);
    for track in tracks {
        if track.left.len() != track.right.len() {
            bail!("track's left and right channels have different lengths");
        }
        if track.len() != len {
            bail!("tracks have different lengths");
        }
        if track.sample_rate != sample_rate {
            bail!("tracks have different sample rates");
        }
        if track.sample_rate == 0 {
            bail!("track has a sample rate of 0");
        }
    }
    Ok(())
}

/// An output device as listed for the user. `id` is cpal's stable device id
/// (`DeviceId`'s `Display` form); `name` is only for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

/// Output devices present right now. Doesn't open any stream.
pub fn output_devices() -> Vec<DeviceInfo> {
    let Ok(devices) = cpal::default_host().output_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|d| Some(DeviceInfo { id: d.id().ok()?.to_string(), name: d.to_string() }))
        .collect()
}

/// How a saved device id resolves against the devices present now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceChoice {
    SystemDefault,
    Saved,
    Missing,
}

pub fn choose_device(saved: Option<&str>, available: &[DeviceInfo]) -> DeviceChoice {
    match saved {
        None => DeviceChoice::SystemDefault,
        Some(id) if available.iter().any(|d| d.id == id) => DeviceChoice::Saved,
        Some(_) => DeviceChoice::Missing,
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
    /// `tracks` must already satisfy `check_tracks` (non-empty; equal left/right
    /// lengths; equal length and equal non-zero sample rate across all tracks).
    /// This is validated once by the caller (`Player::new`) and is not re-checked
    /// here or on the audio thread.
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
        // Read `playing` before the seek (see `Transport::is_playing`): if we
        // observe `true` here, any seek requested before that call is
        // guaranteed visible to `take_seek` below, so a play press right after
        // a seek is never missed.
        let playing = self.transport.is_playing();
        if let Some(frames) = self.transport.take_seek() {
            self.pos = frames; // a seek must still apply while paused
        }
        if !playing {
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
        // Publish the position only if no seek arrived during this callback,
        // so a UI seek made mid-callback isn't overwritten for one buffer.
        if !self.transport.seek_pending() {
            self.transport.set_position(self.pos.min(len as f64));
        }
    }
}

/// Owns the output stream; dropping the player stops playback.
pub struct Player {
    _stream: cpal::Stream,
    transport: Arc<Transport>,
    frames: usize,
    sample_rate: u32,
    device_name: String,
    fell_back: bool,
}

impl Player {
    pub fn new(tracks: Arc<Vec<StereoAudio>>, controls: Arc<MixControls>, device_id: Option<&str>) -> Result<Self> {
        check_tracks(&tracks)?;
        let frames = tracks[0].len();
        let sample_rate = tracks[0].sample_rate;
        let transport = Arc::new(Transport::default());

        let host = cpal::default_host();
        let saved = device_id
            .and_then(|id| cpal::DeviceId::from_str(id).ok())
            .and_then(|id| host.device_by_id(&id));
        // A saved device that's gone (unplugged) falls back to the default.
        let fell_back = device_id.is_some() && saved.is_none();
        let device = match saved {
            Some(device) => device,
            None => host.default_output_device().context("no audio output device")?,
        };
        let device_name = device.to_string();

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
            device_name,
            fell_back,
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

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// True when the saved device was missing and the default was used.
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_device_resolves_saved_ids() {
        let devices = [
            DeviceInfo { id: "coreaudio:A".into(), name: "Speakers".into() },
            DeviceInfo { id: "coreaudio:B".into(), name: "Interface".into() },
        ];
        assert_eq!(choose_device(None, &devices), DeviceChoice::SystemDefault);
        assert_eq!(choose_device(Some("coreaudio:B"), &devices), DeviceChoice::Saved);
        assert_eq!(choose_device(Some("coreaudio:Z"), &devices), DeviceChoice::Missing);
    }

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

    #[test]
    fn seeking_and_playing_after_the_end_resumes_from_zero() {
        let (mut e, _, transport) = engine(vec![track(vec![5.0, 6.0], vec![5.0, 6.0], 4)], 4, 2);
        transport.set_playing(true);
        // Overshoot the 2-frame track so the engine stops mid-buffer.
        let mut out = vec![0.0; 8];
        e.render(&mut out);
        assert!(!transport.is_playing());

        // Mirrors `Player::toggle`: request_seek(0.0) then set_playing(true).
        transport.request_seek(0.0);
        transport.set_playing(true);
        let mut out = vec![0.0; 4];
        e.render(&mut out);
        assert_eq!(out, vec![5.0, 5.0, 6.0, 6.0]);
        assert!(transport.is_playing());
    }

    #[test]
    fn downmixes_to_mono_by_averaging_left_and_right() {
        let (mut e, _, transport) = engine(vec![track(vec![2.0], vec![4.0], 4)], 4, 1);
        transport.set_playing(true);
        let mut out = vec![0.0; 1];
        e.render(&mut out);
        assert_eq!(out, vec![3.0]);
    }

    fn ok_track() -> StereoAudio {
        track(vec![0.0; 4], vec![0.0; 4], 4)
    }

    #[test]
    fn check_tracks_accepts_matching_tracks() {
        assert!(check_tracks(&[ok_track(), ok_track()]).is_ok());
    }

    #[test]
    fn check_tracks_rejects_empty() {
        assert!(check_tracks(&[]).is_err());
    }

    #[test]
    fn check_tracks_rejects_mismatched_channel_lengths() {
        let bad = track(vec![0.0; 4], vec![0.0; 3], 4);
        assert!(check_tracks(&[bad]).is_err());
    }

    #[test]
    fn check_tracks_rejects_different_track_lengths() {
        let short = track(vec![0.0; 3], vec![0.0; 3], 4);
        assert!(check_tracks(&[ok_track(), short]).is_err());
    }

    #[test]
    fn check_tracks_rejects_different_sample_rates() {
        let other_rate = track(vec![0.0; 4], vec![0.0; 4], 8);
        assert!(check_tracks(&[ok_track(), other_rate]).is_err());
    }

    #[test]
    fn check_tracks_rejects_zero_sample_rate() {
        let zero_rate = track(vec![0.0; 4], vec![0.0; 4], 0);
        assert!(check_tracks(&[zero_rate]).is_err());
    }
}
