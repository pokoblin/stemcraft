//! ① Drop a song or choose a file.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{v_flex, ActiveTheme as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Stage};
use crate::i18n::t;

impl AppView {
    pub(crate) fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Empty(state) = &self.stage else { unreachable!() };
        let loading = state.loading.is_some();
        let s = t();
        let theme = cx.theme();
        let (border, accent, primary, muted_fg) =
            (theme.border, theme.accent, theme.primary, theme.muted_foreground);

        div().size_full().p_8().child(
            v_flex()
                .id("drop-zone")
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .border_2()
                .border_dashed()
                .border_color(border)
                .rounded_lg()
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.bg(accent).border_color(primary)
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    if let Some(path) = paths.paths().first() {
                        this.load_file(path.clone(), window, cx);
                    }
                }))
                .when(loading, |el| el.child(Spinner::new()).child(s.reading))
                .when(!loading, |el| {
                    el.child(div().text_lg().child(s.drop_hint))
                        .child(
                            Button::new("choose-file")
                                .primary()
                                .label(s.choose_file)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_file_dialog(window, cx)
                                })),
                        )
                        .child(div().text_sm().text_color(muted_fg).child(s.supported_formats))
                }),
        )
    }
}
