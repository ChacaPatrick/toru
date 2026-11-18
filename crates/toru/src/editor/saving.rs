use std::{sync::Arc, time::Duration};

use gpui::Context;
use tiny_skia::Pixmap;
use toru_core::settings::SaveFormat;

use super::document::{Edits, Flattened};
use super::tools::Tool;
use super::{Editor, EditorEvent};

const COPIED_FOR: Duration = Duration::from_millis(1500);

impl Editor {
    /// Text being typed, and a crop rectangle drawn but not yet applied, go on the document, so
    /// what leaves the editor is what the user sees. The crop is applied as Apply would, as one
    /// undo step, and the editor returns to Select.
    fn commit_pending(&mut self, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.tool == Tool::Crop {
            self.apply_crop(cx);
        }
    }

    /// What Copy, Save and Save as take, once `commit_pending` has run; a drag waits, as the
    /// mark under the pointer isn't on the document yet.
    fn flattened(&mut self, cx: &mut Context<Self>) -> Option<Flattened> {
        self.commit_pending(cx);
        self.drag.is_none().then(|| self.document.flatten())
    }

    /// The edited picture for the card, if it shows something else: what Back gives it, and
    /// what quitting with the editor open gives it so the edits aren't lost.
    pub fn take_unwritten(&mut self, cx: &mut Context<Self>) -> Option<Flattened> {
        self.commit_pending(cx);
        self.document.take_unwritten()
    }

    pub(super) fn back(&mut self, cx: &mut Context<Self>) {
        let changed = self.take_unwritten(cx);
        cx.emit(EditorEvent::Back(changed));
    }

    pub(super) fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(flattened) = self.flattened(cx) {
            cx.emit(EditorEvent::Copy(flattened));
        }
    }

    /// The Copy button's check mark, once the clipboard has taken the picture.
    pub fn show_copied(&mut self, cx: &mut Context<Self>) {
        self.copied
            .show((), COPIED_FOR, cx, |this| &mut this.copied);
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(flattened) = self.flattened(cx) else {
            return;
        };
        cx.emit(if self.document.is_saved() {
            EditorEvent::NothingToSave
        } else {
            EditorEvent::Save(flattened)
        });
    }

    /// The file now shows `edits`, which the user may have changed since.
    pub fn saved(&mut self, edits: Edits, cx: &mut Context<Self>) {
        self.document.mark_saved(edits);
        cx.notify();
    }

    /// The card shows `edits`, which no file does: a redacted Save stored them on the card
    /// first, then couldn't write the file.
    pub fn written(&mut self, edits: Edits) {
        self.document.mark_written(edits);
    }

    /// Writing `edits` to the card failed, so Back tries again.
    pub fn write_failed(&mut self, edits: &Edits) {
        self.document.write_failed(edits);
    }

    /// A save of `picture`, flattened from `saved`, deleted the capture's original, as the
    /// picture hid some of it. The saved picture becomes the one edited, with no undo back past
    /// it, so neither Reset edits nor Undo can bring back what was hidden. `to_card` says the
    /// card shows it too (Save, not Save as). Typing under way is finished first, and a drag
    /// under way is dropped, as the marks it holds by index change.
    pub fn original_removed(
        &mut self,
        picture: Arc<Pixmap>,
        saved: &Edits,
        to_card: bool,
        cx: &mut Context<Self>,
    ) {
        self.finish_typing(cx);
        self.cancel_drag(cx);
        let shown = self.shown();
        self.document.forget_original(picture, saved, to_card);
        self.select(None, cx);
        if self.tool == Tool::Crop {
            self.crop_draft = self.document_crop_draft();
        }
        if self.shown() == shown {
            self.document_changed(cx);
        } else {
            self.shown_changed(cx);
        }
    }

    pub(super) fn save_as(&mut self, format: SaveFormat, cx: &mut Context<Self>) {
        if let Some(flattened) = self.flattened(cx) {
            cx.emit(EditorEvent::SaveAs(flattened, format));
        }
    }
}
