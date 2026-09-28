//! Window bookkeeping: the main window and the single settings window.

use gpui_kit::component::{Root, TitleBar};
use gpui_kit::*;

use crate::i18n::t;
use crate::views::settings_window::SettingsView;

#[derive(Default)]
struct Windows {
    main: Option<AnyWindowHandle>,
    settings: Option<WindowHandle<Root>>,
}

impl Global for Windows {}

/// Options for a window whose toolbar is drawn in the title bar.
pub fn window_options(title: &str, initial: Size<Pixels>, min: Size<Pixels>, cx: &App) -> WindowOptions {
    let mut options = TitleBar::window_options();
    options.titlebar = Some(TitlebarOptions {
        // Hidden by the transparent title bar, but used by the Window menu.
        title: Some(title.to_string().into()),
        appears_transparent: true,
        // Centred on a 40 px bar (the default suits gpui-kit's 34 px).
        traffic_light_position: Some(point(px(12.), px(12.))),
    });
    options.window_bounds = Some(WindowBounds::centered(initial, cx));
    options.window_min_size = Some(min);
    options
}

pub fn open_main(cx: &mut App) {
    cx.set_global(Windows::default());
    let options = window_options("Stemcraft", size(px(1100.), px(720.)), size(px(900.), px(600.)), cx);
    let handle = cx
        .open_window(options, |window, cx| {
            let view = cx.new(|cx| crate::app::AppView::new(window, cx));
            // Dialogs and notifications need a Root as the window's first view.
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("cannot open the main window");
    cx.global_mut::<Windows>().main = Some(handle.into());
    // Closing the settings window keeps the app running; closing the main one quits.
    cx.on_window_closed(|cx, id| {
        let windows = cx.global_mut::<Windows>();
        if windows.settings.is_some_and(|h| h.window_id() == id) {
            windows.settings = None;
        }
        let closed_main = windows.main.is_some_and(|h| h.window_id() == id);
        if closed_main {
            cx.quit();
        }
    })
    .detach();
}

/// Open the settings window, or bring it to the front if it's already open.
pub fn open_settings(cx: &mut App) {
    if let Some(handle) = cx.global::<Windows>().settings {
        let id = handle.window_id();
        if cx.windows().iter().any(|w| w.window_id() == id) {
            // Fails when called from inside that window (⌘, while it has
            // focus) — it's already frontmost then, so ignoring is right.
            let _ = handle.update(cx, |_, window, _| window.activate_window());
            return;
        }
    }
    let options = window_options(t().settings_title, size(px(720.), px(520.)), size(px(600.), px(420.)), cx);
    let handle = cx
        .open_window(options, |window, cx| {
            let view = cx.new(|cx| SettingsView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("cannot open the settings window");
    cx.global_mut::<Windows>().settings = Some(handle);
}
