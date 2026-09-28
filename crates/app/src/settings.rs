//! User preferences, stored as JSON in `~/Library/Application Support/Stemcraft/`.
//! Plain data and file I/O; `app_state` wraps it in a gpui Global.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use stemcraft_core::export::ExportFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    System,
    ZhHans,
    /// Traditional Chinese, Taiwan wording.
    ZhHant,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ExportLocation {
    #[default]
    SameAsSong,
    Folder { path: PathBuf },
}

/// A saved output device: the stable cpal id to match on, and a name to show
/// when it's unplugged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Language,
    pub appearance: Appearance,
    pub export_location: ExportLocation,
    /// Extension of the default export format (`ExportFormat::extension`).
    pub export_format: String,
    pub export_stems: bool,
    pub export_chords: bool,
    /// `None` follows the system default output.
    pub output_device: Option<OutputDevice>,
    /// Set when a cache clear was requested while the GPU was already
    /// initialised this session; the clear happens at the next launch
    /// instead, before any GPU use.
    pub clear_gpu_cache_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            appearance: Appearance::default(),
            export_location: ExportLocation::default(),
            export_format: ExportFormat::Flac.extension().into(),
            export_stems: false,
            export_chords: false,
            output_device: None,
            clear_gpu_cache_on_launch: false,
        }
    }
}

/// Where an export goes, and whether the configured folder was missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDir {
    pub dir: PathBuf,
    pub fell_back: bool,
}

impl Settings {
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Stemcraft").join("settings.json"))
    }

    /// A missing, unreadable or invalid file gives the defaults; missing
    /// fields take their defaults and unknown fields are ignored.
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write via a temporary file, so a crash never leaves a truncated file.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn format(&self) -> ExportFormat {
        ExportFormat::ALL
            .into_iter()
            .find(|f| f.extension() == self.export_format)
            .unwrap_or(ExportFormat::Flac)
    }

    /// The export folder for `song`; a custom folder that no longer exists
    /// falls back to the song's own folder.
    pub fn export_dir(&self, song: &Path) -> ExportDir {
        let song_dir = song.parent().map(Path::to_path_buf).unwrap_or_default();
        match &self.export_location {
            ExportLocation::SameAsSong => ExportDir { dir: song_dir, fell_back: false },
            ExportLocation::Folder { path } if path.is_dir() => {
                ExportDir { dir: path.clone(), fell_back: false }
            }
            ExportLocation::Folder { .. } => ExportDir { dir: song_dir, fell_back: true },
        }
    }

    pub fn output_device_id(&self) -> Option<&str> {
        self.output_device.as_ref().map(|d| d.id.as_str())
    }
}

/// "436 KB", "1.2 GB" — for cache sizes.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{bytes} B"),
        _ if value < 10.0 => format!("{value:.1} {}", UNITS[unit]),
        _ => format!("{value:.0} {}", UNITS[unit]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert_eq!(s.language, Language::System);
        assert_eq!(s.appearance, Appearance::System);
        assert_eq!(s.export_location, ExportLocation::SameAsSong);
        assert_eq!(s.format(), ExportFormat::Flac);
        assert!(!s.export_stems && !s.export_chords);
        assert_eq!(s.output_device_id(), None);
        assert!(!s.clear_gpu_cache_on_launch);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let s = Settings {
            language: Language::ZhHans,
            appearance: Appearance::Dark,
            export_location: ExportLocation::Folder { path: "/tmp/out".into() },
            export_format: "mp3".into(),
            export_stems: true,
            export_chords: true,
            output_device: Some(OutputDevice { id: "coreaudio:X".into(), name: "Speakers".into() }),
            clear_gpu_cache_on_launch: true,
        };
        s.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), s);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_or_corrupt_files_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load_from(&dir.path().join("none.json")), Settings::default());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{ not json").unwrap();
        assert_eq!(Settings::load_from(&bad), Settings::default());
    }

    #[test]
    fn missing_fields_default_and_unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(&path, r#"{"appearance":"dark","from_the_future":1}"#).unwrap();
        let s = Settings::load_from(&path);
        assert_eq!(s.appearance, Appearance::Dark);
        assert_eq!(s.language, Language::System);
        assert_eq!(s.export_format, "flac");
    }

    #[test]
    fn export_location_json_shape() {
        let json = serde_json::to_string(&ExportLocation::Folder { path: "/x".into() }).unwrap();
        assert_eq!(json, r#"{"kind":"folder","path":"/x"}"#);
        let json = serde_json::to_string(&ExportLocation::SameAsSong).unwrap();
        assert_eq!(json, r#"{"kind":"same-as-song"}"#);
    }

    #[test]
    fn language_json_names() {
        assert_eq!(serde_json::to_string(&Language::ZhHans).unwrap(), r#""zh-hans""#);
        assert_eq!(serde_json::to_string(&Language::ZhHant).unwrap(), r#""zh-hant""#);
    }

    #[test]
    fn format_maps_extensions_and_falls_back_to_flac() {
        let s_mp3 = Settings {
            export_format: "mp3".into(),
            ..Settings::default()
        };
        assert_eq!(s_mp3.format(), ExportFormat::Mp3);
        let s_bogus = Settings {
            export_format: "bogus".into(),
            ..Settings::default()
        };
        assert_eq!(s_bogus.format(), ExportFormat::Flac);
    }

    #[test]
    fn export_dir_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("music/song.mp3");

        let s_same = Settings::default();
        assert_eq!(
            s_same.export_dir(&song),
            ExportDir { dir: dir.path().join("music"), fell_back: false }
        );

        let s_folder = Settings {
            export_location: ExportLocation::Folder { path: dir.path().to_path_buf() },
            ..Settings::default()
        };
        assert_eq!(s_folder.export_dir(&song), ExportDir { dir: dir.path().to_path_buf(), fell_back: false });

        let s_gone = Settings {
            export_location: ExportLocation::Folder { path: dir.path().join("gone") },
            ..Settings::default()
        };
        assert_eq!(
            s_gone.export_dir(&song),
            ExportDir { dir: dir.path().join("music"), fell_back: true }
        );
    }

    #[test]
    fn formats_byte_counts() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(446_464), "436 KB");
        assert_eq!(format_bytes(1_288_490_189), "1.2 GB");
    }
}
