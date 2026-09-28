//! The settings window. Values live in the `AppSettings` global; every change
//! is saved and applied at once.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::accordion::Accordion;
use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::label::Label;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, Root, Sizable as _, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::export::ExportFormat;
use stemcraft_core::{separation, weights};

use crate::app_state::{self, AppSettings};
use crate::i18n::{fill, t};
use crate::licenses::LICENSES;
use crate::player::{choose_device, output_devices, DeviceChoice};
use crate::settings::{format_bytes, Appearance, ExportLocation, Language, OutputDevice};
use crate::views::toolbar::title_bar;

pub struct SettingsView {
    /// Indices of the expanded license entries (the accordion is controlled).
    open_licenses: Vec<usize>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Fields write the globals; the Settings component doesn't re-render by itself.
        let subscriptions = vec![
            cx.observe_global::<AppSettings>(|_, cx| cx.notify()),
            cx.observe_global::<app_state::Busy>(|_, cx| cx.notify()),
        ];
        Self { open_licenses: Vec::new(), _subscriptions: subscriptions }
    }

    fn pages(&self, cx: &mut Context<Self>) -> Vec<SettingPage> {
        vec![general_page(), export_page(cx), audio_page(cx), storage_page(cx), self.about_page(cx)]
    }

    fn about_page(&self, cx: &mut Context<Self>) -> SettingPage {
        let s = t();
        let view = cx.entity().downgrade();
        let open = self.open_licenses.clone();
        SettingPage::new(s.page_about)
            .icon(Icon::new(Lucide::Info))
            .resettable(false)
            .group(
                SettingGroup::new()
                    .item(SettingItem::render(|_, _, cx| {
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("Stemcraft {}", env!("CARGO_PKG_VERSION"))),
                            )
                            .child(Label::new(t().tagline).text_sm().text_color(cx.theme().muted_foreground))
                    }))
                    .item(SettingItem::new(
                        s.model_source,
                        SettingField::render(|_, _, cx| {
                            Label::new(t().model_source_value)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                        }),
                    )),
            )
            .group(SettingGroup::new().title(s.licenses).item(SettingItem::render(move |_, _, cx| {
                let mono = cx.theme().mono_font_family.clone();
                let view = view.clone();
                let mut accordion = Accordion::new("licenses");
                for (i, license) in LICENSES.iter().enumerate() {
                    let is_open = open.contains(&i);
                    let mono = mono.clone();
                    accordion = accordion.item(move |item| {
                        item.title(format!("{} — {}", license.name, license.license))
                            .open(is_open)
                            .child(div().text_xs().font_family(mono).child(license.text))
                    });
                }
                accordion.on_toggle_click(move |open: &[usize], _, cx| {
                    let open = open.to_vec();
                    view.update(cx, |this, cx| {
                        this.open_licenses = open;
                        cx.notify();
                    })
                    .ok();
                })
            })))
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

fn export_page(cx: &App) -> SettingPage {
    let s = t();
    let custom = matches!(AppSettings::get(cx).export_location, ExportLocation::Folder { .. });
    let mut items = vec![
        SettingItem::new(
            s.export_location,
            SettingField::dropdown(
                vec![("same".into(), s.location_same.into()), ("folder".into(), s.location_folder.into())],
                |cx: &App| {
                    let key = match AppSettings::get(cx).export_location {
                        ExportLocation::Folder { .. } => "folder",
                        ExportLocation::SameAsSong => "same",
                    };
                    key.into()
                },
                |value: SharedString, cx: &mut App| {
                    if value.as_ref() == "folder" {
                        // Stays "same" until a folder is actually chosen.
                        choose_export_folder(cx);
                    } else {
                        AppSettings::update(cx, |s| s.export_location = ExportLocation::SameAsSong);
                    }
                },
            ),
        )
        .description(s.location_desc),
    ];
    if custom {
        items.push(SettingItem::new(
            s.folder_label,
            SettingField::render(|options, _, cx| {
                let path = match &AppSettings::get(cx).export_location {
                    ExportLocation::Folder { path } => path.display().to_string(),
                    ExportLocation::SameAsSong => String::new(),
                };
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .max_w(px(240.))
                            .truncate()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(path),
                    )
                    .child(
                        Button::new("choose-export-folder")
                            .outline()
                            .with_size(options.size())
                            .icon(Lucide::FolderOpen)
                            .label(t().choose)
                            .on_click(|_, _, cx| choose_export_folder(cx)),
                    )
            }),
        ));
    }
    items.push(
        SettingItem::new(
            s.default_format,
            SettingField::dropdown(
                ExportFormat::ALL
                    .iter()
                    .map(|f| (f.extension().into(), f.label().into()))
                    .collect(),
                |cx: &App| AppSettings::get(cx).format().extension().into(),
                |value: SharedString, cx: &mut App| {
                    AppSettings::update(cx, |s| s.export_format = value.to_string());
                },
            ),
        )
        .description(s.defaults_desc),
    );
    items.push(SettingItem::new(
        s.default_stems,
        SettingField::switch(
            |cx: &App| AppSettings::get(cx).export_stems,
            |on: bool, cx: &mut App| AppSettings::update(cx, |s| s.export_stems = on),
        ),
    ));
    items.push(SettingItem::new(
        s.default_chords,
        SettingField::switch(
            |cx: &App| AppSettings::get(cx).export_chords,
            |on: bool, cx: &mut App| AppSettings::update(cx, |s| s.export_chords = on),
        ),
    ));
    SettingPage::new(s.page_export)
        .icon(Icon::new(Lucide::Download))
        .resettable(false)
        .group(SettingGroup::new().items(items))
}

fn choose_export_folder(cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(t().choose.into()),
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(Ok(Some(mut paths))) = rx.await
            && let Some(path) = paths.pop()
        {
            cx.update(|cx| AppSettings::update(cx, |s| s.export_location = ExportLocation::Folder { path }));
        }
    })
    .detach();
}

fn audio_page(cx: &App) -> SettingPage {
    let s = t();
    let devices = output_devices();
    let saved = AppSettings::get(cx).output_device.clone();
    let mut options: Vec<(SharedString, SharedString)> = vec![("".into(), s.system_default_device.into())];
    options.extend(devices.iter().map(|d| (d.id.clone().into(), d.name.clone().into())));
    // Keep an unplugged saved device selectable, marked as unavailable.
    if let Some(saved) = &saved
        && choose_device(Some(&saved.id), &devices) == DeviceChoice::Missing
    {
        options.push((saved.id.clone().into(), fill(s.unavailable, &[("name", saved.name.as_str())]).into()));
    }
    SettingPage::new(s.page_audio)
        .icon(Icon::new(Lucide::Speaker))
        .resettable(false)
        .group(SettingGroup::new().item(
            SettingItem::new(
                s.output_device,
                SettingField::dropdown(
                    options,
                    |cx: &App| AppSettings::get(cx).output_device_id().unwrap_or("").to_string().into(),
                    move |value: SharedString, cx: &mut App| {
                        let device = if value.is_empty() {
                            None
                        } else {
                            devices
                                .iter()
                                .find(|d| d.id == value.as_ref())
                                .map(|d| OutputDevice { id: d.id.clone(), name: d.name.clone() })
                                .or_else(|| {
                                    AppSettings::get(cx).output_device.clone().filter(|d| d.id == value.as_ref())
                                })
                        };
                        AppSettings::update(cx, |s| s.output_device = device);
                    },
                ),
            )
            .description(s.output_device_desc),
        ))
}

fn storage_page(cx: &App) -> SettingPage {
    let s = t();
    let size = format_bytes(separation::gpu_cache_size());
    let busy = app_state::is_busy(cx);
    let model = if weights::is_bundled() { s.model_bundled } else { s.model_cached };
    SettingPage::new(s.page_storage)
        .icon(Icon::new(Lucide::HardDrive))
        .resettable(false)
        .group(SettingGroup::new().items(vec![
            SettingItem::new(
                s.gpu_cache,
                SettingField::render(move |options, _, cx| {
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(size.clone()))
                        .child(
                            Button::new("clear-gpu-cache")
                                .outline()
                                .with_size(options.size())
                                .label(t().clear)
                                .disabled(busy)
                                .on_click(|_, window, cx| confirm_clear_cache(window, cx)),
                        )
                }),
            )
            .description(if busy { s.clear_busy } else { s.gpu_cache_desc }),
            SettingItem::new(
                s.model_label,
                SettingField::render(move |_, _, cx| {
                    Label::new(model).text_sm().text_color(cx.theme().muted_foreground)
                }),
            ),
        ]))
}

fn confirm_clear_cache(window: &mut Window, cx: &mut App) {
    window.open_alert_dialog(cx, |alert, _, _| {
        let s = t();
        alert
            .confirm()
            .title(s.clear_confirm_title)
            .description(s.clear_confirm_body)
            // button_props replaces all props (show_cancel too), so set it first.
            .button_props(
                DialogButtonProps::default()
                    .ok_text(s.clear)
                    .cancel_text(s.cancel)
                    .show_cancel(true),
            )
            .on_ok(|_, window, cx| {
                // Check again: a split may have started since the dialog opened.
                let note = if app_state::is_busy(cx) {
                    Notification::warning(t().clear_busy)
                } else {
                    match separation::clear_gpu_cache() {
                        Ok(()) => Notification::success(t().cache_cleared),
                        Err(e) => Notification::error(format!("{}{}{e:#}", t().clear_failed, t().reason_sep)),
                    }
                };
                window.push_notification(note, cx);
                cx.refresh_windows();
                true
            })
            .on_cancel(|_, _, _| true)
    });
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
