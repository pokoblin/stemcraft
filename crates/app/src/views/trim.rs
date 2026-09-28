//! ② Pick the part of the song to separate: ruler, a large waveform with a
//! draggable selection, and the time fields.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Sizable as _};
use gpui_kit::*;

use crate::app::{AppView, Stage};
use crate::i18n::{fill, t};
use crate::timeline::{format_clock, frac_to_secs};
use crate::views::ruler::ruler;
use crate::views::waveform_view::{waveform, WaveformProps};

impl AppView {
    pub(crate) fn render_trim(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Trim(st) = &self.stage else { unreachable!() };
        let s = t();
        let duration = st.duration();
        let playhead = st.player.as_ref().map(|p| p.position_frac()).unwrap_or(0.0);
        let length = frac_to_secs(st.selection.1 - st.selection.0, duration);
        let theme = cx.theme();
        let (panel, border, muted_fg, danger, primary) =
            (theme.title_bar, theme.border, theme.muted_foreground, theme.danger, theme.primary);
        let width = f32::from(window.viewport_size().width) as f64;

        let ruler = ruler(
            "trim-ruler",
            duration,
            width,
            playhead,
            cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
            cx,
        );
        let wave = waveform(
            "trim-waveform",
            WaveformProps {
                peaks: st.peaks.clone(),
                color: primary,
                playhead: Some(playhead),
                selection: Some(st.selection),
                dimmed: false,
            },
            Some(st.anchor.clone()),
            cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
            cx.listener(|this, sel: &(f32, f32), window, cx| this.set_selection(*sel, window, cx)),
            cx,
        );
        // The hint gives way to an input error.
        let message = match &st.input_error {
            Some(error) => div().text_color(danger).child(error.clone()),
            None => div().text_color(muted_fg).child(s.trim_hint),
        };
        let bar = h_flex()
            .h(px(56.))
            .flex_none()
            .px_4()
            .gap_3()
            .items_center()
            .bg(panel)
            .border_t_1()
            .border_color(border)
            .text_sm()
            .child(s.start_label)
            .child(div().w(px(84.)).child(Input::new(&st.start_input).small()))
            .child(s.end_label)
            .child(div().w(px(84.)).child(Input::new(&st.end_input).small()))
            .child(
                div()
                    .text_color(muted_fg)
                    .child(fill(s.selected_length, &[("len", format_clock(length).as_str())])),
            )
            .child(div().flex_1().min_w_0().truncate().child(message))
            .child(
                Button::new("start-separation")
                    .primary()
                    .icon(Lucide::AudioWaveform)
                    .label(s.start_separation)
                    .on_click(cx.listener(|this, _, _, cx| this.start_separation(cx))),
            );

        v_flex()
            .size_full()
            .child(ruler)
            .child(div().flex_1().min_h(px(160.)).p_2().child(wave))
            .child(bar)
    }
}
