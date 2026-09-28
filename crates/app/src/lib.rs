//! Stemcraft desktop app: split a song into six stems, audition a mix, export it.

pub mod app;
pub mod app_state;
pub mod controls;
pub mod export_job;
pub mod i18n;
pub mod licenses;
pub mod naming;
pub mod player;
pub mod settings;
pub mod style;
pub mod timeline;
pub mod views;
pub mod waveform;
pub mod windows;
pub mod worker;

use gpui_kit::*;

use crate::app_state::AppSettings;

actions!(stemcraft, [Quit, OpenSettings]);

pub fn run() {
    gpui_kit::application()
        .with_assets(style::AppAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            app_state::init(cx);
            style::install_themes(cx);
            style::apply_appearance(cx);
            app::bind_keys(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-,", OpenSettings, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &OpenSettings, cx| windows::open_settings(cx));
            // After bind_keys, so the menu shows the shortcuts.
            apply_language(cx);
            windows::open_main(cx);
            cx.activate(true);
        });
}

/// Apply the language setting everywhere: our strings, gpui-kit's built-in
/// strings and the menu bar.
pub fn apply_language(cx: &mut App) {
    let language = AppSettings::get(cx).language;
    let locale = i18n::set_language(language, sys_locale::get_locale().as_deref());
    gpui_kit::component::set_locale(locale);
    cx.set_menus(menus());
    cx.refresh_windows();
}

fn menus() -> Vec<Menu> {
    let s = i18n::t();
    vec![Menu {
        name: "Stemcraft".into(),
        items: vec![
            MenuItem::action(s.settings_menu, OpenSettings),
            MenuItem::separator(),
            MenuItem::action(s.quit, Quit),
        ],
        disabled: false,
    }]
}
