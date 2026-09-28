//! ① Drop a song or choose a file.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{v_flex, ActiveTheme as _, Icon};
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
        let (panel, border, primary, muted_fg) =
            (theme.title_bar, theme.border, theme.primary, theme.muted_foreground);

        div().size_full().flex().items_center().justify_center().child(
            v_flex()
                .id("drop-zone")
                .w(px(480.))
                .h(px(280.))
                .items_center()
                .justify_center()
                .gap_3()
                .rounded_lg()
                .border_2()
                .border_dashed()
                .border_color(border)
                .bg(panel)
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.border_color(primary).bg(primary.opacity(0.08))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    if let Some(path) = paths.paths().first() {
                        this.load_file(path.clone(), window, cx);
                    }
                }))
                .when(loading, |el| {
                    el.child(Spinner::new())
                        .child(div().text_sm().text_color(muted_fg).child(s.reading))
                })
                .when(!loading, |el| {
                    el.child(Icon::new(Lucide::FileMusic).size(px(48.)).text_color(muted_fg))
                        .child(div().text_lg().font_weight(FontWeight::MEDIUM).child(s.drop_hint))
                        .child(
                            Button::new("choose-file")
                                .primary()
                                .icon(Lucide::FolderOpen)
                                .label(s.choose_file)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_file_dialog(window, cx)
                                })),
                        )
                        .child(div().text_xs().text_color(muted_fg).child(s.supported_formats))
                }),
        )
    }
}
