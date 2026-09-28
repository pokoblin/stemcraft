//! Mixer stage: per-track mute/solo, and confirming a discard before
//! opening another song.

use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;

use crate::i18n::t;

use super::{AppView, Stage};

impl AppView {
    pub fn toggle_mute(&mut self, track: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Mixer(st) = &self.stage {
            st.controls.set_mute(track, !st.controls.get(track).mute);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn toggle_solo(&mut self, track: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Mixer(st) = &self.stage {
            st.controls.set_solo(track, !st.controls.get(track).solo);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Opening another song discards the stems, so confirm first.
    pub fn confirm_open_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let s = t();
            alert
                .confirm()
                .title(s.confirm_discard_title)
                .description(s.confirm_discard_body)
                // button_props replaces all props including show_cancel, so set it before the handlers
                // and add show_cancel(true) to restore the Cancel button.
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(s.confirm_open)
                        .cancel_text(s.cancel)
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    // Don't discard the stems until a file is actually chosen —
                    // `load_file` replaces the stage once one is (or reports an
                    // unsupported type and leaves the mixer); cancelling the
                    // picker must leave the current stems in place.
                    view.update(cx, |this, cx| this.open_file_dialog(window, cx)).ok();
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }
}
