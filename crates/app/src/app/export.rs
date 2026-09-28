//! Export stage: the export dialog, running the export job, and its
//! progress/done notification (polled regardless of the current stage).

use std::path::PathBuf;
use std::sync::mpsc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Sizable as _, WindowExt as _};
use gpui_kit::*;
use stemcraft_core::mix;

use crate::export_job::{self, ExportJob, ExportMsg};
use crate::i18n::t;
use crate::naming;
use crate::views::export_dialog::{ExportDialog, ExportRequest};
use crate::worker;

use super::{AppView, Stage};

pub struct ExportProgress {
    pub rx: mpsc::Receiver<ExportMsg>,
    pub done: usize,
    pub total: usize,
}

impl AppView {
    pub fn open_export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Stage::Mixer(st) = &self.stage else { return };
        let audible = mix::gains(&st.controls.snapshot()).iter().map(|g| *g > 0.0).collect();
        let dest = st.path.parent().map(PathBuf::from).unwrap_or_default();
        let options = cx.new(|_| ExportDialog::new(dest, audible));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let s = t();
            let options_for_ok = options.clone();
            let view = view.clone();
            dialog
                .title(s.export_title)
                .w(px(500.))
                .child(options.clone())
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(Button::new("export-cancel").outline().label(s.cancel)))
                        .child(DialogAction::new().child(Button::new("export-confirm").primary().label(s.export_button))),
                )
                .on_ok(move |_, _, cx| {
                    let request = options_for_ok.read(cx).request();
                    view.update(cx, |this, cx| this.start_export(request, cx)).ok();
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }

    pub fn start_export(&mut self, request: ExportRequest, cx: &mut Context<Self>) {
        if self.export.is_some() {
            return;
        }
        let Stage::Mixer(st) = &mut self.stage else { return };
        let ids: Vec<&str> = request.stems.iter().map(|&i| worker::STEM_IDS[i]).collect();
        let plan = naming::plan_export(&request.dest, &st.song, request.format.extension(), &ids, request.chords);
        let job = ExportJob {
            plan,
            format: request.format,
            stems: st.stems.clone(),
            gains: mix::gains(&st.controls.snapshot()),
            stem_indices: request.stems,
            chords: st.chords.clone(),
            title: st.song.clone(),
        };
        let total = job.total();
        self.export = Some(ExportProgress {
            rx: export_job::spawn(job),
            done: 0,
            total,
        });
        cx.notify();
    }

    /// Polled every tick regardless of stage, so a stage change (e.g. "Open
    /// another song") while an export runs doesn't drop its Done/Failed
    /// notification.
    pub(super) fn poll_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(progress) = self.export.as_mut() else { return };
        let finished = loop {
            match progress.rx.try_recv() {
                Ok(ExportMsg::Progress { done, total }) => {
                    progress.done = done;
                    progress.total = total;
                    cx.notify();
                }
                Ok(ExportMsg::Done(dir)) => break Ok(dir),
                Ok(ExportMsg::Failed(e)) => break Err(e),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => break Err(t().worker_stopped.to_string()),
            }
        };
        self.export = None;
        match finished {
            Ok(dir) => notify_export_done(dir, window, cx),
            Err(e) => window.push_notification(
                Notification::error(format!("{}{}{e}", t().export_failed, t().reason_sep)),
                cx,
            ),
        }
        cx.notify();
    }
}

fn notify_export_done(dir: PathBuf, window: &mut Window, cx: &mut App) {
    let message = dir.display().to_string();
    window.push_notification(
        Notification::new()
            .title(t().export_done)
            .message(message)
            .action(move |_, _, cx| {
                let dir = dir.clone();
                Button::new("reveal-export")
                    .primary()
                    .small()
                    .label(t().reveal)
                    .on_click(cx.listener(move |note, _, window, cx| {
                        cx.reveal_path(&dir);
                        note.dismiss(window, cx);
                    }))
            }),
        cx,
    );
}
