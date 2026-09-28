//! ④ Six coloured tracks (mute / solo / volume / waveform) under a time ruler
//! and a chord lane, all sharing one playhead.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon};
use gpui_kit::*;
use stemcraft_core::chords::{Chord, NO_CHORD};
use stemcraft_core::mix::gains;

use crate::app::{AppView, MixerState, Stage};
use crate::i18n::t;
use crate::style::{
    mute_on, playhead, solo_on, solo_on_text, stem_color, CHORD_LANE_H, RULER_H, STEM_ICONS,
    TRACK_HEADER_W,
};
use crate::timeline::chord_at;
use crate::views::ruler::ruler;
use crate::views::waveform_view::{waveform, WaveformProps};

impl AppView {
    pub(crate) fn render_mixer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Stage::Mixer(st) = &self.stage else { unreachable!() };
        let s = t();
        let duration = st.duration();
        let (playhead_frac, position) = st
            .player
            .as_ref()
            .map(|p| (p.position_frac(), p.position_secs()))
            .unwrap_or((0.0, 0.0));
        let theme = cx.theme();
        let (panel, border, muted_fg) = (theme.title_bar, theme.border, theme.muted_foreground);
        let lane_width = (f32::from(window.viewport_size().width) - TRACK_HEADER_W) as f64;
        // Left column cell, aligned with the track headers below.
        let header = move |content: AnyElement| {
            h_flex()
                .w(px(TRACK_HEADER_W))
                .flex_none()
                .h_full()
                .px_3()
                .gap_2()
                .items_center()
                .bg(panel)
                .border_r_1()
                .border_color(border)
                .text_xs()
                .text_color(muted_fg)
                .child(content)
        };

        let ruler_row = h_flex()
            .h(px(RULER_H))
            .flex_none()
            .child(header(div().child(s.tracks_label).into_any_element()).border_b_1())
            .child(div().flex_1().min_w_0().child(ruler(
                "mixer-ruler",
                duration,
                lane_width,
                playhead_frac,
                cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
                cx,
            )));
        let chord_row = h_flex()
            .h(px(CHORD_LANE_H))
            .flex_none()
            .border_b_1()
            .border_color(border)
            .child(header(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(Lucide::Music).size(px(13.)))
                    .child(s.chords_label)
                    .into_any_element(),
            ))
            .child(chord_lane(&st.chords, duration, position, playhead_frac, cx));

        v_flex()
            .size_full()
            .child(ruler_row)
            .child(chord_row)
            .child(self.render_tracks(st, playhead_frac, cx))
    }

    fn render_tracks(&self, st: &MixerState, playhead_frac: f32, cx: &mut Context<Self>) -> Div {
        let s = t();
        let mix = st.controls.snapshot();
        let track_gains = gains(&mix);
        let theme = cx.theme();
        let (panel, border, muted, muted_fg) =
            (theme.title_bar, theme.border, theme.muted, theme.muted_foreground);
        let mono = theme.mono_font_family.clone();
        let (mute_bg, solo_bg, solo_fg) = (mute_on(cx), solo_on(), solo_on_text());
        // M / S toggles are custom so a pressed M is dark red and a pressed S yellow.
        let toggle = move |id: (&'static str, usize), label: &'static str, on: bool, on_bg: Hsla, on_fg: Hsla| {
            div()
                .id(id)
                .w(px(22.))
                .h(px(18.))
                .flex_none()
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .bg(if on { on_bg } else { muted })
                .text_color(if on { on_fg } else { muted_fg })
                .child(label)
        };

        let mut rows = v_flex().flex_1().min_h_0();
        for (i, m) in mix.iter().enumerate() {
            let color = stem_color(i);
            let dimmed = track_gains[i] == 0.0;
            let head = h_flex()
                .w(px(TRACK_HEADER_W))
                .flex_none()
                .h_full()
                .gap_2()
                .pr_3()
                .items_center()
                .bg(panel)
                .border_r_1()
                .border_color(border)
                .child(div().w(px(4.)).h_full().bg(color))
                .child(Icon::new(STEM_ICONS[i]).size(px(15.)).text_color(color))
                .child(div().w(px(36.)).text_sm().truncate().child(s.stems[i]))
                .child(
                    toggle(("mute", i), "M", m.mute, mute_bg, white())
                        .on_click(cx.listener(move |this, _, window, cx| this.toggle_mute(i, window, cx))),
                )
                .child(
                    toggle(("solo", i), "S", m.solo, solo_bg, solo_fg)
                        .on_click(cx.listener(move |this, _, window, cx| this.toggle_solo(i, window, cx))),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        // Slider paints its bar with the element's background colour.
                        Slider::new(&st.sliders[i])
                            .horizontal()
                            .bg(color.opacity(if dimmed { 0.35 } else { 1.0 })),
                    ),
                )
                .child(
                    div()
                        .w(px(34.))
                        .flex()
                        .justify_end()
                        .text_xs()
                        .font_family(mono.clone())
                        .text_color(muted_fg)
                        .child(format!("{}%", (m.volume * 100.0).round() as i32)),
                );
            let wave = waveform(
                ("track-waveform", i),
                WaveformProps {
                    peaks: st.peaks[i].clone(),
                    color,
                    playhead: Some(playhead_frac),
                    selection: None,
                    dimmed,
                },
                None,
                cx.listener(|this, frac: &f32, window, cx| this.seek(*frac, window, cx)),
                |_: &(f32, f32), _: &mut Window, _: &mut App| {},
                cx,
            );
            rows = rows.child(
                h_flex()
                    .flex_1()
                    .min_h(px(44.))
                    .border_b_1()
                    .border_color(border)
                    .child(head)
                    .child(div().flex_1().min_w_0().h_full().py_1().child(wave)),
            );
        }
        rows
    }
}

/// Chords on the same time axis as the waveforms; the sounding one is highlighted.
fn chord_lane(
    chords: &[Chord],
    duration: f64,
    position: f64,
    playhead_frac: f32,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (muted, foreground, primary, on_primary) =
        (theme.muted, theme.foreground, theme.primary, theme.primary_foreground);
    let line = playhead(cx);
    let frac = |secs: f64| if duration > 0.0 { (secs / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let current = chord_at(chords, position);
    let segments: Vec<_> = chords
        .iter()
        .enumerate()
        .filter(|(_, c)| c.label != NO_CHORD)
        .map(|(i, c)| {
            let on = Some(i) == current;
            div()
                .absolute()
                .top(px(3.))
                .bottom(px(3.))
                .left(relative(frac(c.start)))
                .w(relative((frac(c.end) - frac(c.start)).max(0.0)))
                .px(px(1.))
                .child(
                    div()
                        .size_full()
                        .px_1()
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .text_xs()
                        .bg(if on { primary } else { muted })
                        .text_color(if on { on_primary } else { foreground })
                        .child(c.label.clone()),
                )
        })
        .collect();
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .children(segments)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(playhead_frac.clamp(0.0, 1.0)))
                .w(px(1.5))
                .bg(line),
        )
}
