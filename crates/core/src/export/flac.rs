//! 24-bit FLAC (flacenc, pure Rust).

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::audio::StereoAudio;

const MAX_24: f32 = 8_388_607.0; // 2^23 - 1

pub(super) fn write(path: &Path, audio: &StereoAudio) -> Result<()> {
    let mut interleaved = Vec::with_capacity(audio.len() * 2);
    for (l, r) in audio.left.iter().zip(&audio.right) {
        interleaved.push((l.clamp(-1.0, 1.0) * MAX_24).round() as i32);
        interleaved.push((r.clamp(-1.0, 1.0) * MAX_24).round() as i32);
    }
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| anyhow!("flac config: {e:?}"))?;
    let source =
        flacenc::source::MemSource::from_samples(&interleaved, 2, 24, audio.sample_rate as usize);
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| anyhow!("flac encode: {e:?}"))?;
    // flacenc 0.5.1 lets the short final frame lower STREAMINFO's min block size,
    // which the FLAC spec forbids and Symphonia rejects. Restore the fixed size.
    if stream.frame_count() > 1 {
        stream
            .stream_info_mut()
            .set_block_sizes(config.block_size, config.block_size)
            .map_err(|e| anyhow!("flac block sizes: {e:?}"))?;
    }
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| anyhow!("flac write: {e:?}"))?;
    std::fs::write(path, sink.as_slice()).with_context(|| format!("cannot write {}", path.display()))
}
