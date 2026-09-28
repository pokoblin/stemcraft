//! The title bar that doubles as a toolbar (traffic lights kept), shared by
//! the main and settings windows.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, ActiveTheme as _, Sizable as _, TitleBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use stemcraft_core::chords::NO_CHORD;

use crate::app::{AppView, Stage};
use crate::i18n::t;
use crate::player::Player;
use crate::style::{TOOLBAR_H, TRAFFIC_LIGHT_GUTTER};
use crate::timeline::{chord_at, format_clock_precise};
use crate::views::processing::step_label;

/// Keep a title-bar button's press from reaching the TitleBar, which would
/// start a window drag (or zoom on double-click). `on_click` still fires.
pub fn tb(button: Button) -> Button {
    button.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// `center` is centred on the window; `left` starts after the traffic lights.
pub fn title_bar(
    center: impl Into<SharedString>,
    left: Vec<AnyElement>,
    right: Vec<AnyElement>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let fullscreen = window.is_fullscreen();
    let theme = cx.theme();
    TitleBar::new()
        .h(px(TOOLBAR_H))
        .pl_0()
        .bg(theme.title_bar)
        .child(
            div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                // A plain div has no hitbox, so window drags pass through the title.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(center.into()),
                )
                .child(
                    h_flex()
                        .h_full()
                        .gap_1()
                        .items_center()
                        .when(!fullscreen, |d| d.pl(px(TRAFFIC_LIGHT_GUTTER)))
                        .when(fullscreen, |d| d.pl_3())
                        .children(left),
                )
                .child(div().flex_1())
                .child(h_flex().h_full().gap_1().items_center().pr_2().children(right)),
        )
}

impl AppView {
    pub(crate) fn render_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = t();
        let mut left = Vec::new();
        let mut right = Vec::new();
        let center: SharedString = match &self.stage {
            Stage::Empty(_) => "Stemcraft".into(),
            Stage::Trim(st) => {
                left = self.transport(&st.player, st.duration(), None, cx);
                right.push(
                    tb(Button::new("tb-change-file")
                        .ghost()
                        .small()
                        .icon(Lucide::FolderOpen)
                        .label(s.change_file)
                        .on_click(cx.listener(|this, _, _, cx| this.back_to_empty(cx))))
                    .into_any_element(),
                );
                st.song.clone().into()
            }
            Stage::Processing(st) => format!("{} · {}", st.song, step_label(st.step)).into(),
            Stage::Mixer(st) => {
                let position = st.player.as_ref().map(|p| p.position_secs()).unwrap_or(0.0);
                let chord = chord_at(&st.chords, position)
                    .map(|i| st.chords[i].label.as_str())
                    .filter(|label| *label != NO_CHORD)
                    .unwrap_or("—");
                left = self.transport(&st.player, st.duration(), Some(chord), cx);
                right.push(
                    tb(Button::new("tb-open")
                        .ghost()
                        .small()
                        .icon(Lucide::FolderOpen)
                        .label(s.open_short)
                        .on_click(cx.listener(|this, _, window, cx| this.confirm_open_new(window, cx))))
                    .into_any_element(),
                );
                right.push(
                    tb(Button::new("tb-export")
                        .primary()
                        .small()
                        .icon(Lucide::Download)
                        .label(s.export_button)
                        .on_click(cx.listener(|this, _, window, cx| this.open_export_dialog(window, cx))))
                    .into_any_element(),
                );
                st.song.clone().into()
            }
        };
        right.push(
            tb(Button::new("tb-settings")
                .ghost()
                .small()
                .icon(Lucide::Settings)
                .tooltip(s.settings_tooltip)
                .on_click(|_, _, cx| {
                    cx.stop_propagation();
                    crate::windows::open_settings(cx);
                }))
            .into_any_element(),
        );
        title_bar(center, left, right, window, cx)
    }

    /// Back-to-start, play/pause, the clock and (in the mixer) the current chord.
    fn transport(
        &self,
        player: &Result<Player, String>,
        duration: f64,
        chord: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let s = t();
        let (position, playing) = player
            .as_ref()
            .map(|p| (p.position_secs(), p.is_playing()))
            .unwrap_or((0.0, false));
        let theme = cx.theme();
        let (muted_fg, primary, on_primary) =
            (theme.muted_foreground, theme.primary, theme.primary_foreground);
        let mono = theme.mono_font_family.clone();
        let mut items = vec![
            tb(Button::new("tb-back")
                .ghost()
                .small()
                .icon(Lucide::SkipBack)
                .tooltip(s.back_to_start)
                .on_click(cx.listener(|this, _, window, cx| this.seek(0.0, window, cx))))
            .into_any_element(),
            tb(Button::new("tb-play")
                .primary()
                .small()
                .icon(if playing { Lucide::Pause } else { Lucide::Play })
                .tooltip(if playing { s.pause } else { s.play })
                .on_click(cx.listener(|this, _, window, cx| this.toggle_play(window, cx))))
            .into_any_element(),
            h_flex()
                .ml_2()
                .gap_1()
                .items_baseline()
                .font_family(mono)
                .child(div().text_base().child(format_clock_precise(position)))
                .child(
                    div()
                        .text_xs()
                        .text_color(muted_fg)
                        .child(format!("/ {}", format_clock_precise(duration))),
                )
                .into_any_element(),
        ];
        if let Some(chord) = chord {
            items.push(
                div()
                    .ml_2()
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .bg(primary)
                    .text_color(on_primary)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(chord.to_string())
                    .into_any_element(),
            );
        }
        items
    }
}
