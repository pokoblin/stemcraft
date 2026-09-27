//! Page renderers (as `impl AppView` blocks) and shared pieces.

pub mod empty;
pub mod mixer;
pub mod processing;
pub mod trim;
pub mod waveform_view;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::AppView;
use crate::i18n::t;
use crate::timeline::format_clock;

/// Play / pause, time, and song title. Callers append their own buttons.
pub(crate) fn transport(
    playing: bool,
    position: f64,
    duration: f64,
    song: &str,
    cx: &mut Context<AppView>,
) -> Div {
    let s = t();
    h_flex()
        .gap_3()
        .items_center()
        .child(
            Button::new("play")
                .primary()
                .label(if playing { s.pause } else { s.play })
                .on_click(cx.listener(|this, _, window, cx| this.toggle_play(window, cx))),
        )
        .child(
            div()
                .text_sm()
                .child(format!("{} / {}", format_clock(position), format_clock(duration))),
        )
        .child(div().font_weight(FontWeight::MEDIUM).child(song.to_string()))
}
