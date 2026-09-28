//! Clickable waveform: bars, optional selection band and playhead. A click
//! seeks; with a drag anchor, dragging selects a range.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

use crate::style::playhead;
use crate::waveform::columns;

/// Movement (fraction of the width) before a press counts as a drag.
const DRAG_THRESHOLD: f32 = 0.003;
const BAR_PITCH: f32 = 2.0;
const BAR_WIDTH: f32 = 1.5;
/// Grab distance (px) around a selection edge that drags that edge.
const HANDLE_GRAB_PX: f32 = 6.0;
const HANDLE_W: f32 = 3.0;

pub struct WaveformProps {
    pub peaks: Arc<[(f32, f32)]>,
    pub color: Hsla,
    pub playhead: Option<f32>,
    pub selection: Option<(f32, f32)>,
    pub dimmed: bool,
}

pub(crate) fn fraction(position: Point<Pixels>, bounds: Bounds<Pixels>) -> f32 {
    if bounds.size.width <= px(0.) {
        return 0.0;
    }
    ((position.x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0)
}

/// `drag_anchor` must live in the owning view's state: it has to survive the
/// re-render between mouse-down and the following move events.
pub fn waveform(
    id: impl Into<ElementId>,
    props: WaveformProps,
    drag_anchor: Option<Rc<Cell<Option<f32>>>>,
    on_seek: impl Fn(&f32, &mut Window, &mut App) + 'static,
    on_select: impl Fn(&(f32, f32), &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let bar_color = props.color.opacity(if props.dimmed { 0.28 } else { 0.9 });
    // Everything outside the selection is dimmed with the window background.
    let shade = theme.background.opacity(0.6);
    let handle_color = theme.primary;
    let line = playhead(cx);
    let selection = props.selection;

    // Event handlers get no bounds; the canvas stores them during prepaint.
    let bounds_cell = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let on_select = Rc::new(on_select);

    div()
        .id(id)
        .size_full()
        .overflow_hidden()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, {
            let bounds_cell = bounds_cell.clone();
            let anchor = drag_anchor.clone();
            move |event: &MouseDownEvent, window, cx| {
                let bounds = bounds_cell.get();
                let frac = fraction(event.position, bounds);
                if let Some(anchor) = &anchor {
                    // Grabbing a selection edge drags that edge: anchor the other one.
                    if let Some((start, end)) = selection {
                        let width = f32::from(bounds.size.width);
                        let near = |edge: f32| (frac - edge).abs() * width <= HANDLE_GRAB_PX;
                        if near(end) {
                            anchor.set(Some(start));
                            return;
                        }
                        if near(start) {
                            anchor.set(Some(end));
                            return;
                        }
                    }
                    anchor.set(Some(frac));
                }
                on_seek(&frac, window, cx);
            }
        })
        .child(
            canvas(
                move |bounds, _, _| bounds_cell.set(bounds),
                move |bounds, (), window, _| {
                    let width = f32::from(bounds.size.width);
                    let height = f32::from(bounds.size.height);
                    let mid = f32::from(bounds.origin.y) + height / 2.0;
                    let cols = columns(&props.peaks, (width / BAR_PITCH).max(1.0) as usize);
                    for (i, (lo, hi)) in cols.iter().enumerate() {
                        let top = mid - hi * height / 2.0;
                        let bar_height = ((hi - lo) * height / 2.0).max(1.0);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.origin.x + px(i as f32 * BAR_PITCH), px(top)),
                                size(px(BAR_WIDTH), px(bar_height)),
                            ),
                            bar_color,
                        ));
                    }
                    if let Some((start, end)) = props.selection {
                        let x0 = bounds.origin.x;
                        window.paint_quad(fill(
                            Bounds::new(point(x0, bounds.origin.y), size(px(start * width), bounds.size.height)),
                            shade,
                        ));
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x0 + px(end * width), bounds.origin.y),
                                size(px((1.0 - end) * width), bounds.size.height),
                            ),
                            shade,
                        ));
                        for edge in [start, end] {
                            let x = (edge * width - HANDLE_W / 2.0).clamp(0.0, (width - HANDLE_W).max(0.0));
                            window.paint_quad(fill(
                                Bounds::new(point(x0 + px(x), bounds.origin.y), size(px(HANDLE_W), bounds.size.height)),
                                handle_color,
                            ));
                        }
                    }
                    if let Some(frac) = props.playhead {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.origin.x + px(frac * width), bounds.origin.y),
                                size(px(1.5), bounds.size.height),
                            ),
                            line,
                        ));
                    }
                    // Window-level listeners keep tracking a drag outside the element.
                    if let Some(anchor) = drag_anchor {
                        let move_anchor = anchor.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || event.pressed_button != Some(MouseButton::Left)
                            {
                                return;
                            }
                            let Some(start) = move_anchor.get() else { return };
                            let frac = fraction(event.position, bounds);
                            if (frac - start).abs() >= DRAG_THRESHOLD {
                                on_select(&(start.min(frac), start.max(frac)), window, cx);
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, _| {
                            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                                anchor.set(None);
                            }
                        });
                    }
                },
            )
            .size_full(),
        )
}
