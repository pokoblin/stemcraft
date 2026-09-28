//! The settings window. Values live in the `AppSettings` global; every change
//! is saved and applied at once.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings};
use gpui_kit::component::{v_flex, ActiveTheme as _, Icon, Root};
use gpui_kit::*;

use crate::app_state::AppSettings;
use crate::i18n::t;
use crate::settings::{Appearance, Language};
use crate::views::toolbar::title_bar;

pub struct SettingsView {
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Fields write the global; the Settings component doesn't re-render by itself.
        let subscriptions = vec![cx.observe_global::<AppSettings>(|_, cx| cx.notify())];
        Self { _subscriptions: subscriptions }
    }

    fn pages(&self, _cx: &mut Context<Self>) -> Vec<SettingPage> {
        vec![general_page()]
    }
}

fn language_key(language: Language) -> &'static str {
    match language {
        Language::System => "system",
        Language::ZhHans => "zh-hans",
        Language::English => "english",
    }
}

fn language_from(key: &str) -> Language {
    match key {
        "zh-hans" => Language::ZhHans,
        "english" => Language::English,
        _ => Language::System,
    }
}

fn appearance_key(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::System => "system",
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    }
}

fn appearance_from(key: &str) -> Appearance {
    match key {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        _ => Appearance::System,
    }
}

fn general_page() -> SettingPage {
    let s = t();
    SettingPage::new(s.page_general)
        .icon(Icon::new(Lucide::Settings))
        .default_open(true)
        .resettable(false)
        .group(SettingGroup::new().items(vec![
            SettingItem::new(
                s.language_label,
                SettingField::dropdown(
                    vec![
                        ("system".into(), s.follow_system.into()),
                        ("zh-hans".into(), "简体中文".into()),
                        ("english".into(), "English".into()),
                    ],
                    |cx: &App| language_key(AppSettings::get(cx).language).into(),
                    |value: SharedString, cx: &mut App| {
                        AppSettings::update(cx, |s| s.language = language_from(&value));
                    },
                ),
            ),
            SettingItem::new(
                s.appearance_label,
                SettingField::dropdown(
                    vec![
                        ("system".into(), s.follow_system.into()),
                        ("light".into(), s.light.into()),
                        ("dark".into(), s.dark.into()),
                    ],
                    |cx: &App| appearance_key(AppSettings::get(cx).appearance).into(),
                    |value: SharedString, cx: &mut App| {
                        AppSettings::update(cx, |s| s.appearance = appearance_from(&value));
                    },
                ),
            ),
        ]))
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let pages = self.pages(cx);
        let theme = cx.theme();
        let (background, foreground) = (theme.background, theme.foreground);
        v_flex()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(title_bar(t().settings_title, Vec::new(), Vec::new(), window, cx))
            .child(
                div().flex_1().min_h_0().child(
                    Settings::new("stemcraft-settings")
                        .sidebar_width(px(190.))
                        // The built-in search box can't be switched off; hide it.
                        .header_style(&StyleRefinement::default().hidden())
                        .pages(pages),
                ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}
