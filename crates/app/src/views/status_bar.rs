//! Bottom status bar: format, selection, export progress and output device.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{h_flex, ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Stage};
use crate::i18n::{fill, t};
use crate::style::STATUS_H;
use crate::timeline::{format_clock, frac_to_secs, khz};

impl AppView {
    pub(crate) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = t();
        let theme = cx.theme();
        let (panel, border, muted_fg, warning) =
            (theme.title_bar, theme.border, theme.muted_foreground, theme.warning);
        let (rate, range, player) = match &self.stage {
            Stage::Empty(_) => (None, None, None),
            Stage::Trim(st) => {
                let d = st.duration();
                let range = (frac_to_secs(st.selection.0, d), frac_to_secs(st.selection.1, d));
                (Some(st.source[0].sample_rate), Some(range), Some(&st.player))
            }
            Stage::Processing(st) => (Some(st.audio.sample_rate), Some(st.range), None),
            Stage::Mixer(st) => (Some(st.stems[0].sample_rate), Some(st.range), Some(&st.player)),
        };

        let mut left = h_flex().gap_4();
        if let Some(rate) = rate {
            left = left.child(fill(s.stereo_rate, &[("rate", khz(rate).as_str())]));
        }
        if let Some((start, end)) = range {
            left = left.child(fill(
                s.range,
                &[("start", format_clock(start).as_str()), ("end", format_clock(end).as_str())],
            ));
        }

        let mut right = h_flex().gap_4().items_center();
        if let Some(p) = &self.export {
            let label = fill(
                s.exporting,
                &[("done", p.done.to_string().as_str()), ("total", p.total.to_string().as_str())],
            );
            let percent = if p.total == 0 { 0.0 } else { p.done as f32 / p.total as f32 * 100.0 };
            right = right.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(label)
                    .child(div().w(px(100.)).child(Progress::new("export-progress").value(percent))),
            );
        }
        if let Some(Ok(player)) = player {
            right = right.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .when(player.fell_back(), |d| d.text_color(warning))
                    .child(Icon::new(Lucide::Speaker).size(px(12.)))
                    .child(player.device_name().to_string()),
            );
        }

        h_flex()
            .h(px(STATUS_H))
            .flex_none()
            .px_3()
            .items_center()
            .bg(panel)
            .border_t_1()
            .border_color(border)
            .text_xs()
            .text_color(muted_fg)
            .child(left)
            .child(div().flex_1())
            .child(right)
    }
}
