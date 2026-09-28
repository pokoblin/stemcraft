//! 320 kbps CBR MP3 (LAME, built from source and linked statically).

use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use mp3lame_encoder::{Bitrate, Builder, DualPcm, FlushGap, Quality};

use crate::audio::StereoAudio;

/// Expects samples in ±1.0 at 32, 44.1 or 48 kHz (see `super::mp3_rate`).
pub(super) fn write(path: &Path, audio: &StereoAudio) -> Result<()> {
    let mut builder = Builder::new().context("cannot allocate the MP3 encoder")?;
    builder.set_num_channels(2).map_err(|e| anyhow!("mp3 channels: {e}"))?;
    builder
        .set_sample_rate(audio.sample_rate)
        .map_err(|e| anyhow!("mp3 sample rate: {e}"))?;
    // Keep the output rate equal to the input; LAME may otherwise pick its own.
    builder
        .set_output_sample_rate(NonZeroU32::new(audio.sample_rate))
        .map_err(|e| anyhow!("mp3 output sample rate: {e}"))?;
    builder.set_brate(Bitrate::Kbps320).map_err(|e| anyhow!("mp3 bitrate: {e}"))?;
    builder.set_quality(Quality::Best).map_err(|e| anyhow!("mp3 quality: {e}"))?;
    let mut encoder = builder.build().map_err(|e| anyhow!("mp3 init: {e}"))?;

    let mut out = Vec::new();
    const CHUNK: usize = 1152 * 16;
    for (l, r) in audio.left.chunks(CHUNK).zip(audio.right.chunks(CHUNK)) {
        // encode_to_vec only writes into spare capacity.
        out.reserve(mp3lame_encoder::max_required_buffer_size(l.len()));
        encoder
            .encode_to_vec(DualPcm { left: l, right: r }, &mut out)
            .map_err(|e| anyhow!("mp3 encode: {e}"))?;
    }
    out.reserve(7200);
    // FlushGap keeps the tail; FlushNoGap drops ~40 ms.
    encoder
        .flush_to_vec::<FlushGap>(&mut out)
        .map_err(|e| anyhow!("mp3 flush: {e}"))?;

    // LAME left a zero-filled placeholder frame for the Xing/LAME tag; overwrite it
    // so players can trim encoder delay and padding.
    let tag_size = encoder.lame_tag_size();
    if tag_size > 0 {
        let mut tag = Vec::with_capacity(tag_size);
        if encoder.lame_tag_encode_to_vec(&mut tag).is_some() {
            let offset = encoder.id3v2_tag_size();
            if offset + tag.len() <= out.len() {
                out[offset..offset + tag.len()].copy_from_slice(&tag);
            }
        }
    }
    std::fs::write(path, &out).with_context(|| format!("cannot write {}", path.display()))
}
