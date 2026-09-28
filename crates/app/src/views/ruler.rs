//! Time ruler: labels every few seconds and the playhead. Clicking seeks.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

use crate::style::{playhead, RULER_H};
use crate::timeline::{format_clock, ruler_step};
use crate::views::waveform_view::fraction;

/// `width_px` is the ruler's approximate width, used to space the labels.
pub fn ruler(
    id: impl Into<ElementId>,
    duration: f64,
    width_px: f64,
    position: f32,
    on_seek: impl Fn(&f32, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let (border, muted_fg, panel) = (theme.border, theme.muted_foreground, theme.title_bar);
    let line = playhead(cx);
    let step = ruler_step(duration, width_px);
    let count = if duration > 0.0 { (duration / step).floor() as usize + 1 } else { 0 };
    let ticks = (0..count).map(move |i| {
        let secs = i as f64 * step;
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative((secs / duration) as f32))
            .pl_1()
            .border_l_1()
            .border_color(border)
            .flex()
            .items_center()
            .child(format_clock(secs))
    });
    // Mouse handlers get no bounds; the canvas stores them during prepaint.
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let bounds_for_click = bounds.clone();
    div()
        .id(id)
        .relative()
        .h(px(RULER_H))
        .w_full()
        .flex_none()
        .bg(panel)
        .border_b_1()
        .border_color(border)
        .text_xs()
        .text_color(muted_fg)
        .overflow_hidden()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
            on_seek(&fraction(event.position, bounds_for_click.get()), window, cx);
        })
        .children(ticks)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(position.clamp(0.0, 1.0)))
                .w(px(1.5))
                .bg(line),
        )
        .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().size_full())
}
