//! ② Pick the part of the song to separate.

use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Stage};
use crate::i18n::{fill, t};
use crate::timeline::{format_clock, frac_to_secs};
use crate::views::transport;
use crate::views::waveform_view::{waveform, WaveformProps};

impl AppView {
    pub(crate) fn render_trim(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Trim(st) = &self.stage else { unreachable!() };
        let s = t();
        let duration = st.duration();
        let (playhead, position, playing) = match &st.player {
            Ok(p) => (p.position_frac(), p.position_secs(), p.is_playing()),
            Err(_) => (0.0, 0.0, false),
        };
        let length = frac_to_secs(st.selection.1 - st.selection.0, duration);
        let theme = cx.theme();
        let (muted_fg, danger) = (theme.muted_foreground, theme.danger);

        let top = transport(playing, position, duration, &st.song, cx)
            .child(div().flex_1())
            .child(
                Button::new("change-file")
                    .outline()
                    .label(s.change_file)
                    .on_click(cx.listener(|this, _, _, cx| this.back_to_empty(cx))),
            );

        let wave = waveform(
            "trim-waveform",
            WaveformProps {
                peaks: st.peaks.clone(),
                playhead: Some(playhead),
                selection: Some(st.selection),
                dimmed: false,
            },
            Some(st.anchor.clone()),
            cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
            cx.listener(|this, sel: &(f32, f32), window, cx| this.set_selection(*sel, window, cx)),
            cx,
        );

        let times = h_flex()
            .gap_2()
            .items_center()
            .child(s.start_label)
            .child(div().w(px(96.)).child(Input::new(&st.start_input)))
            .child(s.end_label)
            .child(div().w(px(96.)).child(Input::new(&st.end_input)))
            .child(
                div()
                    .text_sm()
                    .text_color(muted_fg)
                    .child(fill(s.selected_length, &[("len", format_clock(length).as_str())])),
            )
            .when_some(st.input_error.clone(), |el, error| {
                el.child(div().text_sm().text_color(danger).child(error))
            });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(top)
            .child(div().text_sm().text_color(muted_fg).child(s.trim_hint))
            .child(div().h(px(200.)).w_full().child(wave))
            .child(times)
    }
}
