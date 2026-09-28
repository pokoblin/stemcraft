//! ③ A card with the steps and separation progress; errors offer a retry.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Stage};
use crate::i18n::{fill, t};
use crate::timeline::format_clock;
use crate::worker::{steps, Step};

pub(crate) fn step_label(step: Step) -> &'static str {
    let s = t();
    match step {
        Step::LoadModel => s.step_load_model,
        Step::Warmup => s.step_warmup,
        Step::Separate => s.step_separate,
        Step::Chords => s.step_chords,
    }
}

impl AppView {
    pub(crate) fn render_processing(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Processing(st) = &self.stage else { unreachable!() };
        let s = t();
        let theme = cx.theme();
        let (panel, border, muted_fg, danger, success) =
            (theme.title_bar, theme.border, theme.muted_foreground, theme.danger, theme.success);
        let mono = theme.mono_font_family.clone();
        let all = steps(st.warmup);
        let current = all.iter().position(|x| *x == st.step).unwrap_or(0);
        let failed = st.error.is_some();

        let rows = all.iter().enumerate().map(|(i, &step)| {
            let marker = if i < current {
                Icon::new(Lucide::Check).size(px(16.)).text_color(success).into_any_element()
            } else if i == current && failed {
                Icon::new(Lucide::X).size(px(16.)).text_color(danger).into_any_element()
            } else if i == current {
                Spinner::new().small().into_any_element()
            } else {
                div().size(px(6.)).rounded_full().bg(muted_fg.opacity(0.5)).into_any_element()
            };
            let label_color = if i > current {
                Some(muted_fg)
            } else if i == current && failed {
                Some(danger)
            } else {
                None
            };
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().w(px(20.)).flex().justify_center().child(marker))
                        .child(div().when_some(label_color, |d, c| d.text_color(c)).child(step_label(step))),
                )
                .when(step == Step::Separate && i == current && !failed, |row| {
                    row.child(
                        h_flex()
                            .gap_3()
                            .pl(px(32.))
                            .items_center()
                            .child(div().flex_1().child(Progress::new("separate-progress").value(st.progress * 100.0)))
                            .child(
                                div()
                                    .w(px(44.))
                                    .text_sm()
                                    .font_family(mono.clone())
                                    .child(format!("{:.0}%", st.progress * 100.0)),
                            ),
                    )
                })
        });

        let (start, end) = st.range;
        let mut card = v_flex()
            .w(px(460.))
            .p_6()
            .gap_4()
            .rounded_lg()
            .bg(panel)
            .border_1()
            .border_color(border)
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_lg().font_weight(FontWeight::MEDIUM).child(st.song.clone()))
                    .child(div().text_sm().text_color(muted_fg).child(fill(
                        s.range,
                        &[("start", format_clock(start).as_str()), ("end", format_clock(end).as_str())],
                    ))),
            )
            .children(rows);
        if let Some(error) = &st.error {
            card = card
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_color(danger).font_weight(FontWeight::MEDIUM).child(s.failed_title))
                        .child(div().text_sm().child(error.clone())),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("retry")
                                .primary()
                                .label(s.retry)
                                .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                        )
                        .child(
                            Button::new("change-file")
                                .outline()
                                .label(s.change_file)
                                .on_click(cx.listener(|this, _, _, cx| this.back_to_empty(cx))),
                        ),
                );
        }
        div().size_full().flex().items_center().justify_center().child(card)
    }
}
