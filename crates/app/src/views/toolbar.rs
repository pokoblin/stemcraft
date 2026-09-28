//! The title bar that doubles as a toolbar (traffic lights kept), shared by
//! the main and settings windows.

use gpui_kit::component::button::Button;
use gpui_kit::component::{h_flex, ActiveTheme as _, TitleBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::{TOOLBAR_H, TRAFFIC_LIGHT_GUTTER};

/// Keep a title-bar button's press from reaching the TitleBar, which would
/// start a window drag (or zoom on double-click). `on_click` still fires.
pub fn tb(button: Button) -> Button {
    button.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// `center` is centred on the window; `left` starts after the traffic lights.
pub fn title_bar(
    center: impl Into<SharedString>,
    left: Vec<AnyElement>,
    right: Vec<AnyElement>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let fullscreen = window.is_fullscreen();
    let theme = cx.theme();
    TitleBar::new()
        .h(px(TOOLBAR_H))
        .pl_0()
        .bg(theme.title_bar)
        .child(
            div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                // A plain div has no hitbox, so window drags pass through the title.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(center.into()),
                )
                .child(
                    h_flex()
                        .h_full()
                        .gap_1()
                        .items_center()
                        .when(!fullscreen, |d| d.pl(px(TRAFFIC_LIGHT_GUTTER)))
                        .when(fullscreen, |d| d.pl_3())
                        .children(left),
                )
                .child(div().flex_1())
                .child(h_flex().h_full().gap_1().items_center().pr_2().children(right)),
        )
}
