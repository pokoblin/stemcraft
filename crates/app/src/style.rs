//! Look and feel: themes, stem colours and icons, layout sizes, and the extra
//! Lucide icons bundled with the app.

use std::borrow::Cow;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use crate::app_state::AppSettings;
use crate::settings::Appearance;

pub const TOOLBAR_H: f32 = 40.;
pub const STATUS_H: f32 = 24.;
pub const RULER_H: f32 = 22.;
pub const CHORD_LANE_H: f32 = 26.;
pub const TRACK_HEADER_W: f32 = 210.;
/// Room for the macOS traffic lights at the left of the title bar.
pub const TRAFFIC_LIGHT_GUTTER: f32 = 80.;

/// Icons per stem, in `worker::STEM_IDS` order.
pub const STEM_ICONS: [Lucide; 6] = [
    Lucide::Drum,
    Lucide::AudioWaveform,
    Lucide::MicVocal,
    Lucide::Piano,
    Lucide::Guitar,
    Lucide::Shapes,
];
const STEM_COLORS: [u32; 6] = [0xE8684A, 0xE0A030, 0x9B7CF0, 0x4AA3E8, 0x3CC49A, 0xD8609A];

pub fn stem_color(i: usize) -> Hsla {
    rgb(STEM_COLORS[i]).into()
}

/// Background of a pressed M (mute) button: a dark red.
pub fn mute_on(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() { 0x8E2A2A } else { 0xB83232 }).into()
}

/// Background and text of a pressed S (solo) button.
pub fn solo_on() -> Hsla {
    rgb(0xE5D040).into()
}

pub fn solo_on_text() -> Hsla {
    rgb(0x2A2400).into()
}

pub fn playhead(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() { 0xFF4D4D } else { 0xE5383B }).into()
}

// gpui-kit's default asset bundle only has 101 icons; add the ones we use.
gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        Drum, AudioWaveform, MicVocal, Piano, Guitar, Shapes, FileMusic, SkipBack, Download,
        Speaker, X, HardDrive, Info, Music
    ]
);

/// gpui-kit's assets plus `ExtraIcons`.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

const THEMES: &str = include_str!("../themes/stemcraft.json");

/// Register our light and dark themes as gpui-kit's. Call after `gpui_kit::init`.
pub fn install_themes(cx: &mut App) {
    ThemeRegistry::global_mut(cx)
        .load_themes_from_str(THEMES)
        .expect("bundled theme JSON is valid");
    let registry = ThemeRegistry::global(cx);
    let light = registry.themes().get("Stemcraft Light").cloned().expect("light theme");
    let dark = registry.themes().get("Stemcraft Dark").cloned().expect("dark theme");
    let theme = Theme::global_mut(cx);
    theme.light_theme = light;
    theme.dark_theme = dark;
}

/// Apply the appearance setting to every window.
pub fn apply_appearance(cx: &mut App) {
    let mode = match AppSettings::get(cx).appearance {
        Appearance::Light => ThemeMode::Light,
        Appearance::Dark => ThemeMode::Dark,
        Appearance::System => cx.window_appearance().into(),
    };
    Theme::change(mode, None, cx);
    cx.refresh_windows();
}
