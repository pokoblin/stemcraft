//! ⑤ Export options. Lives in its own entity because the dialog builder
//! closure re-runs every frame and must not own state.

use std::path::PathBuf;

use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Selectable as _, Sizable as _};
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
    /// `audible[i]`: whether stem `i` is heard right now (pre-ticks it).
    pub fn new(dest: PathBuf, audible: Vec<bool>) -> Self {
        Self {
            format: ExportFormat::Flac,
            include_stems: false,
            stems: audible,
            chords: false,
            dest,
        }
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
        let muted_fg = cx.theme().muted_foreground;
        let label = |text: &'static str| div().text_sm().text_color(muted_fg).child(text);

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

        v_flex()
            .gap_4()
            .child(v_flex().gap_2().child(label(s.format_label)).child(formats))
            .child(h_flex().gap_2().child("✓").child(s.mix_label))
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
            )
            .child(
                v_flex().gap_2().child(label(s.save_to)).child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().text_sm().truncate().child(self.dest.display().to_string()))
                        .child(
                            Button::new("export-change-folder")
                                .small()
                                .outline()
                                .label(s.change_folder)
                                .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))),
                        ),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportDialog, ExportFormat, PathBuf};

    #[test]
    fn pre_ticks_stems_to_what_is_audible() {
        let dialog = ExportDialog::new(PathBuf::from("/tmp"), vec![true, false, true]);
        assert_eq!(dialog.stems, vec![true, false, true]);
    }

    #[test]
    fn request_omits_stems_unless_also_stems_is_checked() {
        let mut dialog = ExportDialog::new(PathBuf::from("/tmp"), vec![true, false, true]);
        assert_eq!(dialog.request().stems, Vec::<usize>::new());
        dialog.include_stems = true;
        assert_eq!(dialog.request().stems, vec![0, 2]);
    }

    #[test]
    fn request_carries_format_chords_and_dest() {
        let mut dialog = ExportDialog::new(PathBuf::from("/music"), vec![true]);
        dialog.format = ExportFormat::Mp3;
        dialog.chords = true;
        let request = dialog.request();
        assert_eq!(request.format, ExportFormat::Mp3);
        assert!(request.chords);
        assert_eq!(request.dest, PathBuf::from("/music"));
    }
}
