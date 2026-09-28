//! Encode stereo audio as WAV, FLAC, MP3, M4A (AAC) or OGG Vorbis.

use std::path::Path;

use anyhow::{bail, Result};

use crate::audio::{write_wav, StereoAudio};

mod flac;
mod m4a;
mod mp3;
mod ogg;
mod resample;

pub use resample::resample;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Wav,
    Flac,
    Mp3,
    M4a,
    Ogg,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [
        ExportFormat::Wav,
        ExportFormat::Flac,
        ExportFormat::Mp3,
        ExportFormat::M4a,
        ExportFormat::Ogg,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Wav => "wav",
            ExportFormat::Flac => "flac",
            ExportFormat::Mp3 => "mp3",
            ExportFormat::M4a => "m4a",
            ExportFormat::Ogg => "ogg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Wav => "WAV",
            ExportFormat::Flac => "FLAC",
            ExportFormat::Mp3 => "MP3",
            ExportFormat::M4a => "M4A",
            ExportFormat::Ogg => "OGG",
        }
    }
}

/// WAV: 32-bit float. FLAC: 24-bit. MP3: 320 kbps CBR. M4A: AAC-LC 256 kbps.
/// OGG: Vorbis quality 0.8. Integer and lossy formats clamp to ±1.0.
pub fn write(path: &Path, audio: &StereoAudio, format: ExportFormat) -> Result<()> {
    if audio.left.len() != audio.right.len() || audio.sample_rate == 0 {
        bail!("invalid audio buffer");
    }
    match format {
        ExportFormat::Wav => write_wav(path, audio),
        ExportFormat::Flac => flac::write(path, audio),
        ExportFormat::Mp3 => mp3::write(path, &for_rate(audio, mp3_rate(audio.sample_rate))?),
        ExportFormat::M4a => m4a::write(path, &for_rate(audio, aac_rate(audio.sample_rate))?),
        ExportFormat::Ogg => ogg::write(path, &clamped(audio)),
    }
}

/// MP3 encodes 32, 44.1 and 48 kHz directly; other rates are resampled.
fn mp3_rate(sample_rate: u32) -> u32 {
    match sample_rate {
        32_000 | 44_100 | 48_000 => sample_rate,
        r if r > 48_000 => 48_000,
        _ => 44_100,
    }
}

/// Apple's AAC encoder only takes 44.1 and 48 kHz directly: 256 kbps CBR at
/// 32 kHz stereo is rejected outright (`'!dat'`, confirmed with `afconvert
/// -d aac -b 256000`), so 32 kHz (and anything else that isn't 44.1/48 kHz or
/// above) is resampled to 44.1 kHz instead of passed through.
fn aac_rate(sample_rate: u32) -> u32 {
    match sample_rate {
        44_100 | 48_000 => sample_rate,
        r if r > 48_000 => 48_000,
        _ => 44_100,
    }
}

fn clamped(audio: &StereoAudio) -> StereoAudio {
    let clamp = |x: &Vec<f32>| x.iter().map(|s| s.clamp(-1.0, 1.0)).collect();
    StereoAudio {
        left: clamp(&audio.left),
        right: clamp(&audio.right),
        sample_rate: audio.sample_rate,
    }
}

/// Resample to a rate the target lossy codec accepts, then clamp (resampling
/// can overshoot).
fn for_rate(audio: &StereoAudio, rate: u32) -> Result<StereoAudio> {
    Ok(clamped(&resample(audio, rate)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decode;

    pub(super) fn sine(sr: u32, secs: f32) -> StereoAudio {
        let n = (sr as f32 * secs) as usize;
        let s: Vec<f32> = (0..n)
            .map(|i| 0.5 * (std::f32::consts::TAU * 440.0 * i as f32 / sr as f32).sin())
            .collect();
        StereoAudio { left: s.clone(), right: s, sample_rate: sr }
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Encode, decode with Symphonia, and compare sample rate, length and level.
    pub(super) fn roundtrip(format: ExportFormat, sr: u32, expected_sr: u32, length_tolerance: f64) {
        let input = sine(sr, 2.0);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("out.{}", format.extension()));
        write(&path, &input, format).unwrap();

        let out = decode(&path).unwrap();
        assert_eq!(out.sample_rate, expected_sr, "{}", format.label());
        assert!(
            (out.duration_secs() - 2.0).abs() <= length_tolerance,
            "{} lasted {}",
            format.label(),
            out.duration_secs()
        );
        // Compare the middle half: encoders add priming/padding at the edges.
        let mid = |x: &[f32]| rms(&x[x.len() / 4..3 * x.len() / 4]);
        let (want, got) = (mid(&input.left), mid(&out.left));
        assert!((got - want).abs() / want <= 0.15, "{}: rms {got} vs {want}", format.label());
    }

    #[test]
    fn wav_roundtrip() {
        roundtrip(ExportFormat::Wav, 44_100, 44_100, 1e-9);
    }

    #[test]
    fn flac_roundtrip() {
        roundtrip(ExportFormat::Flac, 44_100, 44_100, 1e-9);
    }

    #[test]
    fn flac_clamps_to_full_scale() {
        let mut a = sine(44_100, 0.5);
        a.left[1000] = 3.0;
        a.right[1000] = -3.0;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.flac");
        write(&path, &a, ExportFormat::Flac).unwrap();
        let out = decode(&path).unwrap();
        assert!((out.left[1000] - 1.0).abs() < 1e-5, "{}", out.left[1000]);
        assert!((out.right[1000] + 1.0).abs() < 1e-5, "{}", out.right[1000]);
    }

    #[test]
    fn formats_have_distinct_extensions() {
        let mut exts: Vec<_> = ExportFormat::ALL.iter().map(|f| f.extension()).collect();
        exts.sort();
        exts.dedup();
        assert_eq!(exts.len(), 5);
    }

    #[test]
    fn mp3_roundtrip_44k() {
        roundtrip(ExportFormat::Mp3, 44_100, 44_100, 0.1);
    }

    #[test]
    fn mp3_roundtrip_48k() {
        roundtrip(ExportFormat::Mp3, 48_000, 48_000, 0.1);
    }

    #[test]
    fn mp3_downsamples_high_rates_to_48k() {
        roundtrip(ExportFormat::Mp3, 96_000, 48_000, 0.1);
    }

    #[test]
    fn mp3_moves_odd_rates_to_44k() {
        roundtrip(ExportFormat::Mp3, 22_050, 44_100, 0.1);
    }

    #[test]
    fn ogg_roundtrip() {
        roundtrip(ExportFormat::Ogg, 44_100, 44_100, 0.1);
    }

    #[test]
    fn mp3_rate_rule() {
        assert_eq!(mp3_rate(32_000), 32_000);
        assert_eq!(mp3_rate(44_100), 44_100);
        assert_eq!(mp3_rate(48_000), 48_000);
        assert_eq!(mp3_rate(96_000), 48_000);
        assert_eq!(mp3_rate(22_050), 44_100);
    }

    #[test]
    fn aac_rate_rule() {
        // Apple's AAC encoder rejects 256 kbps CBR at 32 kHz stereo, unlike MP3.
        assert_eq!(aac_rate(32_000), 44_100);
        assert_eq!(aac_rate(44_100), 44_100);
        assert_eq!(aac_rate(48_000), 48_000);
        assert_eq!(aac_rate(96_000), 48_000);
        assert_eq!(aac_rate(22_050), 44_100);
    }

    #[test]
    fn m4a_roundtrip_44k() {
        roundtrip(ExportFormat::M4a, 44_100, 44_100, 0.08);
    }

    #[test]
    fn m4a_roundtrip_48k() {
        roundtrip(ExportFormat::M4a, 48_000, 48_000, 0.08);
    }

    #[test]
    fn m4a_roundtrip_32k() {
        // 32 kHz has no direct AAC rate; it must be resampled to 44.1 kHz.
        roundtrip(ExportFormat::M4a, 32_000, 44_100, 0.08);
    }

    #[test]
    fn m4a_downsamples_high_rates_to_48k() {
        roundtrip(ExportFormat::M4a, 96_000, 48_000, 0.08);
    }

    #[test]
    fn m4a_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.m4a");
        std::fs::write(&path, b"junk").unwrap();
        write(&path, &sine(44_100, 0.5), ExportFormat::M4a).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
    }
}
