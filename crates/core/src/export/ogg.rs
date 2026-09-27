//! OGG Vorbis at quality 0.8 (~256 kbps), libvorbis built from source.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::{NonZeroU32, NonZeroU8};
use std::path::Path;

use anyhow::{Context, Result};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

use crate::audio::StereoAudio;

pub(super) fn write(path: &Path, audio: &StereoAudio) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    let rate = NonZeroU32::new(audio.sample_rate).context("sample rate is 0")?;
    let channels = NonZeroU8::new(2).unwrap();
    let mut encoder = VorbisEncoderBuilder::new(rate, channels, BufWriter::new(file))?
        .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
            target_quality: 0.8,
        })
        .build()?;
    // Moderate blocks: libvorbis is very slow with huge ones.
    const BLOCK: usize = 4096;
    for (l, r) in audio.left.chunks(BLOCK).zip(audio.right.chunks(BLOCK)) {
        encoder.encode_audio_block([l, r])?;
    }
    encoder.finish()?.flush()?;
    Ok(())
}
