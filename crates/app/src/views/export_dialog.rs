//! ⑤ Export options, grouped: format, contents, destination. Lives in its own
//! entity because the dialog builder closure re-runs every frame.

use std::path::PathBuf;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use stemcraft_core::export::ExportFormat;

use crate::i18n::t;

pub struct ExportRequest {
    pub format: ExportFormat,
    /// Stems to write individually (indices into `worker::STEM_IDS`).
    pub stems: Vec<usize>,
    pub chords: bool,
    pub dest: PathBuf,
}

pub struct ExportDialog {
    format: ExportFormat,
    include_stems: bool,
    stems: Vec<bool>,
    chords: bool,
    dest: PathBuf,
}

impl ExportDialog {
    /// `audible[i]`: whether stem `i` is heard right now (pre-ticks it). The
    /// other values come from the settings.
    pub fn new(dest: PathBuf, audible: Vec<bool>, format: ExportFormat, include_stems: bool, chords: bool) -> Self {
        Self { format, include_stems, stems: audible, chords, dest }
    }

    pub fn request(&self) -> ExportRequest {
        let stems = if self.include_stems {
            (0..self.stems.len()).filter(|&i| self.stems[i]).collect()
        } else {
            Vec::new()
        };
        ExportRequest {
            format: self.format,
            stems,
            chords: self.chords,
            dest: self.dest.clone(),
        }
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t().save_to.into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = rx.await else { return };
            let Some(dir) = paths.pop() else { return };
            this.update(cx, |this, cx| {
                this.dest = dir;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ExportDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = t();
        let theme = cx.theme();
        let (muted_fg, border, muted, success) =
            (theme.muted_foreground, theme.border, theme.muted, theme.success);
        let heading = move |text: &'static str| {
            div().text_xs().font_weight(FontWeight::MEDIUM).text_color(muted_fg).child(text)
        };

        let formats = h_flex().gap_1().children(ExportFormat::ALL.iter().map(|&format| {
            Button::new(format.extension())
                .small()
                .label(format.label())
                .selected(self.format == format)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.format = format;
                    cx.notify();
                }))
        }));

        let stem_boxes = h_flex().flex_wrap().gap_3().pl(px(24.)).children((0..self.stems.len()).map(|i| {
            Checkbox::new(("export-stem", i))
                .label(s.stems[i])
                .checked(self.stems[i])
                .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                    this.stems[i] = *checked;
                    cx.notify();
                }))
        }));

        let contents = v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(Lucide::Check).size(px(14.)).text_color(success))
                    .child(s.mix_label),
            )
            .child(
                Checkbox::new("export-also-stems")
                    .label(s.also_stems)
                    .checked(self.include_stems)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.include_stems = *checked;
                        cx.notify();
                    })),
            )
            .when(self.include_stems, |el| el.child(stem_boxes))
            .child(
                Checkbox::new("export-chords")
                    .label(s.chord_chart)
                    .checked(self.chords)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.chords = *checked;
                        cx.notify();
                    })),
            );

        let destination = h_flex()
            .gap_2()
            .items_center()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(muted)
            .child(Icon::new(Lucide::FolderOpen).size(px(14.)).text_color(muted_fg))
            .child(div().flex_1().min_w_0().text_sm().truncate().child(self.dest.display().to_string()))
            .child(
                Button::new("export-change-folder")
                    .small()
                    .outline()
                    .label(s.change_folder)
                    .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))),
            );

        v_flex()
            .gap_5()
            .child(v_flex().gap_2().child(heading(s.format_label)).child(formats))
            .child(v_flex().gap_2().child(heading(s.export_contents)).child(contents))
            .child(v_flex().gap_2().child(heading(s.save_to)).child(destination))
    }
}

#[cfg(test)]
mod tests {
    // Only the names needed: a glob import of gpui here hits the recursion limit.
    use super::{ExportDialog, ExportFormat, PathBuf};

    fn dialog(include_stems: bool) -> ExportDialog {
        ExportDialog::new(
            PathBuf::from("/music"),
            vec![true, false, true, false, false, true],
            ExportFormat::Mp3,
            include_stems,
            true,
        )
    }

    #[test]
    fn request_carries_settings_defaults() {
        let r = dialog(false).request();
        assert_eq!(r.format, ExportFormat::Mp3);
        assert!(r.chords);
        assert_eq!(r.dest, PathBuf::from("/music"));
    }

    #[test]
    fn stems_only_when_enabled_and_pre_ticked_by_audibility() {
        assert!(dialog(false).request().stems.is_empty());
        assert_eq!(dialog(true).request().stems, vec![0, 2, 5]);
    }
}
