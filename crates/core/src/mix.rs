//! Per-stem mute / solo / volume → gains, and the offline mixdown used for export.
//! The desktop app's live player applies the same gains, so an export sounds
//! exactly like what was auditioned.

use anyhow::{bail, Result};

use crate::audio::StereoAudio;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackMix {
    pub mute: bool,
    pub solo: bool,
    /// Linear gain, `0.0..=1.0`.
    pub volume: f32,
}

impl Default for TrackMix {
    fn default() -> Self {
        Self {
            mute: false,
            solo: false,
            volume: 1.0,
        }
    }
}

/// Effective gain per track: muted tracks are silent; if any track is soloed,
/// only soloed tracks sound; otherwise every track plays at its volume.
/// `out` is cleared first, so a real-time caller can reuse its buffer.
pub fn gains_into(tracks: &[TrackMix], out: &mut Vec<f32>) {
    let any_solo = tracks.iter().any(|t| t.solo);
    out.clear();
    out.extend(tracks.iter().map(|t| {
        if t.mute || (any_solo && !t.solo) {
            0.0
        } else {
            t.volume
        }
    }));
}

pub fn gains(tracks: &[TrackMix]) -> Vec<f32> {
    let mut out = Vec::with_capacity(tracks.len());
    gains_into(tracks, &mut out);
    out
}

/// Weighted sum of equally long tracks at the same sample rate.
pub fn mixdown(tracks: &[&StereoAudio], gains: &[f32]) -> Result<StereoAudio> {
    let Some(first) = tracks.first() else {
        bail!("nothing to mix")
    };
    if gains.len() != tracks.len() {
        bail!("expected {} gains, got {}", tracks.len(), gains.len());
    }
    let mut out = StereoAudio::silent(first.len(), first.sample_rate);
    for (track, &gain) in tracks.iter().zip(gains) {
        if track.len() != out.len() || track.sample_rate != out.sample_rate {
            bail!("cannot mix tracks of different length or sample rate");
        }
        if gain == 0.0 {
            continue;
        }
        for (acc, s) in out.left.iter_mut().zip(&track.left) {
            *acc += gain * s;
        }
        for (acc, s) in out.right.iter_mut().zip(&track.right) {
            *acc += gain * s;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::StereoAudio;

    fn t(mute: bool, solo: bool, volume: f32) -> TrackMix {
        TrackMix { mute, solo, volume }
    }

    #[test]
    fn without_solo_each_track_plays_at_its_volume() {
        assert_eq!(
            gains(&[t(false, false, 1.0), t(false, false, 0.5), t(false, false, 0.0)]),
            vec![1.0, 0.5, 0.0]
        );
    }

    #[test]
    fn mute_silences_a_track() {
        assert_eq!(gains(&[t(true, false, 1.0), t(false, false, 0.8)]), vec![0.0, 0.8]);
    }

    #[test]
    fn solo_silences_the_others() {
        assert_eq!(
            gains(&[t(false, true, 0.7), t(false, false, 1.0), t(false, true, 0.3)]),
            vec![0.7, 0.0, 0.3]
        );
    }

    #[test]
    fn mute_wins_over_solo() {
        assert_eq!(gains(&[t(true, true, 1.0), t(false, false, 1.0)]), vec![0.0, 0.0]);
    }

    #[test]
    fn gains_into_reuses_the_buffer() {
        let mut out = Vec::with_capacity(4);
        out.push(9.0);
        gains_into(&[t(false, false, 0.25)], &mut out);
        assert_eq!(out, vec![0.25]);
    }

    #[test]
    fn mixdown_weights_and_sums() {
        let a = StereoAudio { left: vec![1.0, 2.0], right: vec![-1.0, 0.0], sample_rate: 10 };
        let b = StereoAudio { left: vec![4.0, 4.0], right: vec![2.0, 2.0], sample_rate: 10 };
        let mix = mixdown(&[&a, &b], &[1.0, 0.5]).unwrap();
        assert_eq!(mix.left, vec![3.0, 4.0]);
        assert_eq!(mix.right, vec![0.0, 1.0]);
        assert_eq!(mix.sample_rate, 10);
    }

    #[test]
    fn mixdown_rejects_mismatches() {
        let a = StereoAudio { left: vec![1.0], right: vec![1.0], sample_rate: 10 };
        let b = StereoAudio { left: vec![1.0, 2.0], right: vec![1.0, 2.0], sample_rate: 10 };
        assert!(mixdown(&[&a, &b], &[1.0, 1.0]).is_err());
        assert!(mixdown(&[&a], &[1.0, 1.0]).is_err());
        assert!(mixdown(&[], &[]).is_err());
    }
}
