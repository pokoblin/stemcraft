//! Audio decoding (Symphonia), trimming, mixing and WAV output.

use std::path::Path;

use anyhow::{bail, Context, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::timerange::TimeRange;

/// Planar stereo audio. Mono sources are duplicated into both channels.
#[derive(Debug, Clone, PartialEq)]
pub struct StereoAudio {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

impl StereoAudio {
    pub fn silent(len: usize, sample_rate: u32) -> Self {
        Self {
            left: vec![0.0; len],
            right: vec![0.0; len],
            sample_rate,
        }
    }

    pub fn len(&self) -> usize {
        self.left.len()
    }

    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    pub fn duration_secs(&self) -> f64 {
        self.len() as f64 / self.sample_rate as f64
    }

    pub fn to_mono(&self) -> Vec<f32> {
        self.left
            .iter()
            .zip(&self.right)
            .map(|(l, r)| 0.5 * (l + r))
            .collect()
    }

    /// Keep only `range` (clamped to the audio's length).
    pub fn trim(&mut self, range: TimeRange) {
        let to_index =
            |secs: f64| ((secs * self.sample_rate as f64).round() as usize).min(self.len());
        let start = to_index(range.start);
        let end = range.end.map_or(self.len(), to_index);
        for channel in [&mut self.left, &mut self.right] {
            channel.truncate(end);
            channel.drain(..start.min(end));
        }
    }
}

/// Unity-gain sum of equally long tracks at the same sample rate.
pub fn sum_tracks(tracks: &[&StereoAudio]) -> Result<StereoAudio> {
    crate::mix::mixdown(tracks, &vec![1.0; tracks.len()])
}

/// Decode an audio file (WAV, AIFF, FLAC, MP3, OGG Vorbis, AAC/ALAC in M4A).
pub fn decode(path: &Path) -> Result<StereoAudio> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("unsupported audio format: {}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("no audio track found")?
        .clone();
    let sample_rate = track
        .codec_params
        .sample_rate
        .context("unknown sample rate")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("no decoder for this codec")?;

    let mut audio = StereoAudio {
        left: Vec::new(),
        right: Vec::new(),
        sample_rate,
    };
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(e) => return Err(e).context("error reading audio packet"),
        };
        if packet.track_id() != track.id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue, // skip a corrupt packet
            Err(e) => return Err(e).context("error decoding audio"),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        match channels {
            1 => {
                audio.left.extend_from_slice(buf.samples());
                audio.right.extend_from_slice(buf.samples());
            }
            2 => {
                for frame in buf.samples().as_chunks::<2>().0 {
                    audio.left.push(frame[0]);
                    audio.right.push(frame[1]);
                }
            }
            n => bail!("expected mono or stereo audio, got {n} channels"),
        }
    }
    if audio.is_empty() {
        bail!("no audio decoded from {}", path.display());
    }
    Ok(audio)
}

/// Write 32-bit float WAV, so summed stems above full scale never clip.
pub fn write_wav(path: &Path, audio: &StereoAudio) -> Result<()> {
    let spec = WavSpec {
        channels: 2,
        sample_rate: audio.sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)
        .with_context(|| format!("cannot create {}", path.display()))?;
    for (l, r) in audio.left.iter().zip(&audio.right) {
        writer.write_sample(*l)?;
        writer.write_sample(*r)?;
    }
    writer
        .finalize()
        .with_context(|| format!("cannot finalize {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(len: usize, sr: u32) -> StereoAudio {
        let left: Vec<f32> = (0..len).map(|i| i as f32).collect();
        let right = left.iter().map(|x| -x).collect();
        StereoAudio {
            left,
            right,
            sample_rate: sr,
        }
    }

    #[test]
    fn trim_keeps_requested_window() {
        let mut a = ramp(100, 10);
        a.trim(TimeRange {
            start: 2.0,
            end: Some(5.0),
        });
        assert_eq!(a.left, (20..50).map(|i| i as f32).collect::<Vec<_>>());
        assert_eq!(a.right[0], -20.0);

        let mut open_ended = ramp(100, 10);
        open_ended.trim(TimeRange {
            start: 9.0,
            end: None,
        });
        assert_eq!(open_ended.len(), 10);

        let mut past_end = ramp(100, 10);
        past_end.trim(TimeRange {
            start: 20.0,
            end: Some(30.0),
        });
        assert!(past_end.is_empty());
    }

    #[test]
    fn sum_tracks_adds_samples() {
        let a = ramp(4, 10);
        let b = StereoAudio {
            left: vec![1.0; 4],
            right: vec![2.0; 4],
            sample_rate: 10,
        };
        let mix = sum_tracks(&[&a, &b]).unwrap();
        assert_eq!(mix.left, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(mix.right, vec![2.0, 1.0, 0.0, -1.0]);
        assert!(sum_tracks(&[&a, &ramp(5, 10)]).is_err());
    }

    #[test]
    fn wav_roundtrip_keeps_values_above_full_scale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.wav");
        let audio = StereoAudio {
            left: vec![0.5, 1.7, -2.0],
            right: vec![0.0, -1.5, 1.2],
            sample_rate: 44100,
        };
        write_wav(&path, &audio).unwrap();
        let decoded = decode(&path).unwrap();
        assert_eq!(decoded, audio);
    }
}
