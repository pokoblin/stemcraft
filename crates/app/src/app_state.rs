//! App-wide state shared by every window: the settings, and whether a song is
//! being separated.

use std::path::PathBuf;

use gpui_kit::*;

use crate::settings::Settings;

pub struct AppSettings {
    settings: Settings,
    path: Option<PathBuf>,
    /// Set when saving failed; the main window reports it once.
    pub save_error: Option<String>,
}

impl Global for AppSettings {}

impl AppSettings {
    pub fn get(cx: &App) -> &Settings {
        &cx.global::<Self>().settings
    }

    /// Change the settings, save them, and apply language / appearance at once.
    /// Observers (`cx.observe_global::<AppSettings>`) are notified.
    pub fn update(cx: &mut App, change: impl FnOnce(&mut Settings)) {
        let this = cx.global_mut::<Self>();
        let before = this.settings.clone();
        change(&mut this.settings);
        if this.settings == before {
            return;
        }
        if let Some(path) = &this.path
            && let Err(e) = this.settings.save_to(path)
        {
            this.save_error = Some(format!("{e:#}"));
        }
        let after = this.settings.clone();
        if after.language != before.language {
            crate::apply_language(cx);
        }
        if after.appearance != before.appearance {
            crate::style::apply_appearance(cx);
        }
    }
}

/// True while a song is being separated (the GPU cache must not be cleared).
#[derive(Default)]
pub struct Busy(pub bool);

impl Global for Busy {}

pub fn init(cx: &mut App) {
    let path = Settings::default_path();
    let settings = path.as_deref().map(Settings::load_from).unwrap_or_default();
    cx.set_global(AppSettings { settings, path, save_error: None });
    cx.set_global(Busy::default());
}

pub fn set_busy(cx: &mut App, busy: bool) {
    if cx.global::<Busy>().0 != busy {
        cx.global_mut::<Busy>().0 = busy;
    }
}

pub fn is_busy(cx: &App) -> bool {
    cx.global::<Busy>().0
}
