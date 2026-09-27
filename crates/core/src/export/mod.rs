//! Encode stereo audio as WAV, FLAC, MP3, M4A (AAC) or OGG Vorbis.

use std::path::Path;

use anyhow::{bail, Result};

use crate::audio::{write_wav, StereoAudio};

mod flac;
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
        ExportFormat::Mp3 | ExportFormat::M4a | ExportFormat::Ogg => {
            bail!("{} export is not implemented yet", format.label())
        }
    }
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
}
