//! Whole-buffer resampling (rubato), for formats that can't take every rate.

use anyhow::{anyhow, bail, Result};
use rubato::audioadapter::Adapter;
use rubato::audioadapter_buffers::direct::SequentialSliceOfSlices;
use rubato::{Fft, FixedSync, Resampler};

use crate::audio::StereoAudio;

pub fn resample(audio: &StereoAudio, to_rate: u32) -> Result<StereoAudio> {
    if to_rate == 0 {
        bail!("target sample rate must be > 0");
    }
    if to_rate == audio.sample_rate || audio.is_empty() {
        return Ok(StereoAudio {
            sample_rate: to_rate,
            ..audio.clone()
        });
    }
    let frames = audio.len();
    let input: [&[f32]; 2] = [&audio.left, &audio.right];
    let adapter = SequentialSliceOfSlices::new(&input[..], 2, frames)
        .map_err(|e| anyhow!("resample input: {e:?}"))?;
    let mut resampler = Fft::<f32>::new(
        audio.sample_rate as usize,
        to_rate as usize,
        1024,
        2,
        FixedSync::Both,
    )?;
    // process_all trims the resampler delay and returns exactly the resampled length.
    let out = resampler.process_all(&adapter, frames, None)?;
    let n = out.frames();
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        left.push(out.read_sample(0, i).unwrap_or(0.0));
        right.push(out.read_sample(1, i).unwrap_or(0.0));
    }
    Ok(StereoAudio {
        left,
        right,
        sample_rate: to_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(sr: u32, secs: f32) -> StereoAudio {
        let n = (sr as f32 * secs) as usize;
        let s: Vec<f32> = (0..n)
            .map(|i| 0.5 * (std::f32::consts::TAU * 440.0 * i as f32 / sr as f32).sin())
            .collect();
        StereoAudio { left: s.clone(), right: s, sample_rate: sr }
    }

    #[test]
    fn halves_the_length_from_96k_to_48k() {
        let out = resample(&sine(96_000, 2.0), 48_000).unwrap();
        assert_eq!(out.sample_rate, 48_000);
        assert_eq!(out.left.len(), out.right.len());
        assert!((out.left.len() as i64 - 96_000).abs() <= 2, "{}", out.left.len());
    }

    #[test]
    fn same_rate_is_a_copy() {
        let a = sine(44_100, 0.1);
        assert_eq!(resample(&a, 44_100).unwrap(), a);
    }
}
