//! ④ Six tracks with mute / solo / volume, a chord lane and a shared playhead.

use gpui_kit::component::button::Button;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use stemcraft_core::chords::{Chord, NO_CHORD};
use stemcraft_core::mix::gains;

use crate::app::{AppView, MixerState, Stage};
use crate::i18n::{fill, t};
use crate::views::transport;
use crate::views::waveform_view::{waveform, WaveformProps};

/// Width of the controls column, so lanes and waveforms line up.
const LABEL_W: f32 = 300.;

impl AppView {
    pub(crate) fn render_mixer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Mixer(st) = &self.stage else { unreachable!() };
        let s = t();
        let duration = st.duration();
        let (playhead, position, playing) = match &st.player {
            Ok(p) => (p.position_frac(), p.position_secs(), p.is_playing()),
            Err(_) => (0.0, 0.0, false),
        };

        let export: AnyElement = match &st.export {
            Some(p) => div()
                .text_sm()
                .child(fill(
                    s.exporting,
                    &[("done", p.done.to_string().as_str()), ("total", p.total.to_string().as_str())],
                ))
                .into_any_element(),
            None => Button::new("export")
                .label(s.export)
                .on_click(cx.listener(|this, _, window, cx| this.open_export_dialog(window, cx)))
                .into_any_element(),
        };
        let top = transport(playing, position, duration, &st.song, cx)
            .child(div().flex_1())
            .child(export)
            .child(
                Button::new("open-new")
                    .outline()
                    .label(s.open_new)
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_open_new(window, cx))),
            );

        let lane = chord_lane(&st.chords, duration, position, cx);
        let rows = self.render_tracks(st, playhead, cx);

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(top)
            .child(lane)
            .child(rows)
    }

    fn render_tracks(&self, st: &MixerState, playhead: f32, cx: &mut Context<Self>) -> Div {
        let s = t();
        let mix = st.controls.snapshot();
        let track_gains = gains(&mix);
        let muted_fg = cx.theme().muted_foreground;
        let mut rows = v_flex().flex_1().gap_2();
        for (i, m) in mix.iter().enumerate() {
            let controls = h_flex()
                .w(px(LABEL_W))
                .gap_2()
                .items_center()
                .child(div().w(px(56.)).text_sm().child(s.stems[i]))
                .child(
                    Button::new(("mute", i))
                        .small()
                        .label("M")
                        .selected(m.mute)
                        .on_click(cx.listener(move |this, _, window, cx| this.toggle_mute(i, window, cx))),
                )
                .child(
                    Button::new(("solo", i))
                        .small()
                        .label("S")
                        .selected(m.solo)
                        .on_click(cx.listener(move |this, _, window, cx| this.toggle_solo(i, window, cx))),
                )
                .child(div().flex_1().child(Slider::new(&st.sliders[i]).horizontal()))
                .child(
                    div()
                        .w(px(40.))
                        .text_xs()
                        .text_color(muted_fg)
                        .child(format!("{}%", (m.volume * 100.0).round() as i32)),
                );
            let wave = waveform(
                ("track-waveform", i),
                WaveformProps {
                    peaks: st.peaks[i].clone(),
                    playhead: Some(playhead),
                    selection: None,
                    dimmed: track_gains[i] == 0.0,
                },
                None,
                cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
                |_: &(f32, f32), _: &mut Window, _: &mut App| {},
                cx,
            );
            rows = rows.child(
                h_flex()
                    .h(px(64.))
                    .gap_3()
                    .items_center()
                    .child(controls)
                    .child(div().flex_1().h_full().child(wave)),
            );
        }
        rows
    }
}

/// Chords laid out on the same time axis as the waveforms.
fn chord_lane(chords: &[Chord], duration: f64, position: f64, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (muted, border, primary, on_primary, muted_fg) = (
        theme.muted,
        theme.border,
        theme.primary,
        theme.primary_foreground,
        theme.muted_foreground,
    );
    let frac = |secs: f64| if duration > 0.0 { (secs / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let current = chords.iter().position(|c| position >= c.start && position < c.end);
    let segments = chords.iter().enumerate().map(|(i, c)| {
        let label = if c.label == NO_CHORD { String::new() } else { c.label.clone() };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(frac(c.start)))
            .w(relative(frac(c.end) - frac(c.start)))
            .px_1()
            .flex()
            .items_center()
            .overflow_hidden()
            .text_xs()
            .border_r_1()
            .border_color(border)
            .when(Some(i) == current, |d| d.bg(primary).text_color(on_primary))
            .child(label)
    });
    h_flex()
        .h(px(28.))
        .gap_3()
        .child(div().w(px(LABEL_W)).text_sm().text_color(muted_fg).child(t().chords_label))
        .child(
            div()
                .relative()
                .flex_1()
                .h_full()
                .rounded_md()
                .overflow_hidden()
                .bg(muted)
                .children(segments),
        )
}
