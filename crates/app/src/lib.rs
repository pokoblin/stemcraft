//! Stemcraft desktop app: split a song into six stems, audition a mix, export it.

pub mod app;
pub mod controls;
pub mod export_job;
pub mod i18n;
pub mod naming;
pub mod player;
pub mod timeline;
pub mod views;
pub mod waveform;
pub mod worker;

use gpui_kit::component::{Root, Theme};
use gpui_kit::*;

actions!(stemcraft, [Quit]);

pub fn run() {
    i18n::init(sys_locale::get_locale().as_deref());
    gpui_kit::application()
        // Icons (checkbox tick, notification close, …) need the asset bundle.
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            app::bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.set_menus(vec![Menu {
                name: "Stemcraft".into(),
                items: vec![MenuItem::action(i18n::t().quit, Quit)],
                disabled: false,
            }]);

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Stemcraft".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::centered(size(px(1100.), px(720.)), cx)),
                window_min_size: Some(size(px(900.), px(600.))),
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                // gpui-kit defaults to the light theme; follow the system instead.
                Theme::sync_system_appearance(Some(window), cx);
                let view = cx.new(|cx| app::AppView::new(window, cx));
                // Dialogs and notifications require a Root as the window's first view.
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("cannot open the main window");
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);
        });
}
