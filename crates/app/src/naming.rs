//! File names: which inputs we accept, and where an export goes.

use std::path::{Path, PathBuf};

pub const SUPPORTED_EXTENSIONS: [&str; 7] = ["wav", "aiff", "aif", "flac", "mp3", "ogg", "m4a"];

pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Keep Unicode titles (macOS filenames are Unicode); drop only path separators.
pub fn safe_name(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| if c == '/' || c == ':' || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        "output".into()
    } else {
        trimmed.into()
    }
}

pub fn song_name(path: &Path) -> String {
    safe_name(&path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default())
}

/// `parent/base`, or `parent/base (1)`, `(2)`… if that already exists.
pub fn unique_dir(parent: &Path, base: &str) -> PathBuf {
    let first = parent.join(base);
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| parent.join(format!("{base} ({n})")))
        .find(|p| !p.exists())
        .expect("an unused directory name")
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan {
    pub dir: PathBuf,
    pub mix: PathBuf,
    /// One path per requested stem, in the order the ids were given.
    pub stems: Vec<PathBuf>,
    /// `(.lrc, .txt)` when a chord chart was requested.
    pub chords: Option<(PathBuf, PathBuf)>,
}

/// Files go into a fresh `<song> - Stemcraft` folder, so they can't collide.
pub fn plan_export(parent: &Path, song: &str, ext: &str, stem_ids: &[&str], chords: bool) -> ExportPlan {
    let dir = unique_dir(parent, &format!("{song} - Stemcraft"));
    ExportPlan {
        mix: dir.join(format!("{song}_mix.{ext}")),
        stems: stem_ids.iter().map(|id| dir.join(format!("{song}_{id}.{ext}"))).collect(),
        chords: chords.then(|| {
            (
                dir.join(format!("{song}_chords.lrc")),
                dir.join(format!("{song}_chords.txt")),
            )
        }),
        dir,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_unicode_and_drops_separators() {
        assert_eq!(safe_name("加州旅馆"), "加州旅馆");
        assert_eq!(safe_name("AC/DC: Live"), "AC_DC_ Live");
        assert_eq!(safe_name(" . "), "output");
    }

    #[test]
    fn song_name_uses_the_file_stem() {
        assert_eq!(song_name(Path::new("/music/Hotel California.mp3")), "Hotel California");
    }

    #[test]
    fn recognises_supported_audio() {
        assert!(is_supported_audio(Path::new("a.MP3")));
        assert!(is_supported_audio(Path::new("a.aif")));
        assert!(!is_supported_audio(Path::new("a.txt")));
        assert!(!is_supported_audio(Path::new("noext")));
    }

    #[test]
    fn unique_dir_appends_a_counter() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_dir(dir.path(), "x"), dir.path().join("x"));
        std::fs::create_dir(dir.path().join("x")).unwrap();
        std::fs::create_dir(dir.path().join("x (1)")).unwrap();
        assert_eq!(unique_dir(dir.path(), "x"), dir.path().join("x (2)"));
    }

    #[test]
    fn plans_export_paths() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_export(dir.path(), "Song", "flac", &["drums", "guitar"], true);
        let out = dir.path().join("Song - Stemcraft");
        assert_eq!(plan.dir, out);
        assert_eq!(plan.mix, out.join("Song_mix.flac"));
        assert_eq!(plan.stems, vec![out.join("Song_drums.flac"), out.join("Song_guitar.flac")]);
        assert_eq!(
            plan.chords,
            Some((out.join("Song_chords.lrc"), out.join("Song_chords.txt")))
        );
        assert_eq!(plan_export(dir.path(), "Song", "wav", &[], false).chords, None);
    }
}
