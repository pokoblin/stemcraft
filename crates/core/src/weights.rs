//! htdemucs_6s weights: cached in `~/Library/Caches/demucs-rs/` (shared with the
//! demucs-rs CLI), downloaded from Hugging Face on first use.

use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use demucs_core::model::metadata::{download_url, ModelInfo, HTDEMUCS_6S};

pub const MODEL: &ModelInfo = &HTDEMUCS_6S;

pub fn cache_path() -> Result<PathBuf> {
    let base = dirs::cache_dir().context("cannot determine the user cache directory")?;
    Ok(base.join("demucs-rs").join(MODEL.filename))
}

pub fn is_cached() -> bool {
    cache_path().is_ok_and(|p| p.is_file())
}

/// Load the cached weights, downloading them first if needed.
/// `on_download(received_bytes, total_bytes)` reports download progress.
pub fn load(mut on_download: impl FnMut(u64, Option<u64>)) -> Result<Vec<u8>> {
    let path = cache_path()?;
    if path.is_file() {
        return std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()));
    }

    let url = download_url(MODEL);
    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("download failed: {url}"))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse().ok());
    let mut reader = response.into_body().into_reader();
    let mut data = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut chunk = vec![0u8; 1 << 16];
    loop {
        let n = reader.read(&mut chunk).context("download interrupted")?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..n]);
        on_download(data.len() as u64, total);
    }
    if total.is_some_and(|t| t != data.len() as u64) {
        bail!(
            "incomplete download: got {} of {} bytes",
            data.len(),
            total.unwrap()
        );
    }

    // Write to a temporary file first so an interrupted run never leaves a truncated cache.
    std::fs::create_dir_all(path.parent().unwrap())?;
    let partial = path.with_extension("part");
    std::fs::File::create(&partial)
        .and_then(|mut f| f.write_all(&data))
        .with_context(|| format!("cannot write {}", partial.display()))?;
    std::fs::rename(&partial, &path)?;
    Ok(data)
}
