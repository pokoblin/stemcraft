//! htdemucs_6s weights: shipped inside the macOS app bundle, otherwise cached in
//! `~/Library/Caches/demucs-rs/` (shared with the demucs-rs CLI) and downloaded
//! from Hugging Face on first use.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use demucs_core::model::metadata::{download_url, ModelInfo, HTDEMUCS_6S};

pub const MODEL: &ModelInfo = &HTDEMUCS_6S;

pub fn cache_path() -> Result<PathBuf> {
    let base = dirs::cache_dir().context("cannot determine the user cache directory")?;
    Ok(base.join("demucs-rs").join(MODEL.filename))
}

/// `<App>.app/Contents/Resources/<model>` for an executable in `Contents/MacOS/`.
fn bundled_path_for(exe: &Path) -> Option<PathBuf> {
    Some(exe.parent()?.parent()?.join("Resources").join(MODEL.filename))
}

/// Weights already on disk: the app bundle's copy first, then the download
/// cache. `cache` is `None` when the cache directory couldn't be resolved at
/// all — that must not stop the bundled copy from being found.
fn find_local(exe: Option<&Path>, cache: Option<&Path>) -> Option<PathBuf> {
    exe.and_then(bundled_path_for)
        .filter(|p| p.is_file())
        .or_else(|| cache.filter(|c| c.is_file()).map(Path::to_path_buf))
}

fn local_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    let cache = cache_path().ok();
    find_local(exe.as_deref(), cache.as_deref())
}

/// True when no download is needed (bundled or cached).
pub fn is_cached() -> bool {
    local_path().is_some()
}

/// Load the weights, downloading them into the cache first if needed.
/// `on_download(received_bytes, total_bytes)` reports download progress.
pub fn load(mut on_download: impl FnMut(u64, Option<u64>)) -> Result<Vec<u8>> {
    if let Some(path) = local_path() {
        return std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()));
    }
    let path = cache_path()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn bundled_path_is_in_app_resources() {
        let exe = Path::new("/Applications/Stemcraft.app/Contents/MacOS/stemcraft-app");
        assert_eq!(
            bundled_path_for(exe).unwrap(),
            Path::new("/Applications/Stemcraft.app/Contents/Resources").join(MODEL.filename)
        );
    }

    #[test]
    fn find_local_prefers_bundle_then_cache() {
        let dir = tempfile::tempdir().unwrap();
        let macos = dir.path().join("X.app/Contents/MacOS");
        let resources = dir.path().join("X.app/Contents/Resources");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&resources).unwrap();
        let exe = macos.join("stemcraft-app");
        let cache = dir.path().join("cache.safetensors");

        assert_eq!(find_local(Some(&exe), Some(&cache)), None);

        std::fs::write(&cache, b"cached").unwrap();
        assert_eq!(find_local(Some(&exe), Some(&cache)), Some(cache.clone()));

        let bundled = resources.join(MODEL.filename);
        std::fs::write(&bundled, b"bundled").unwrap();
        assert_eq!(find_local(Some(&exe), Some(&cache)), Some(bundled.clone()));

        assert_eq!(find_local(None, Some(&cache)), Some(cache));

        // No cache directory could be resolved at all (`cache_dir()` returned
        // None): the bundled copy must still be found rather than bailing out.
        assert_eq!(find_local(Some(&exe), None), Some(bundled));
        assert_eq!(find_local(None, None), None);
    }
}
