use gpui::{App, Context, Entity, Focusable, Window, prelude::*};
use gpui_kit::component::input::InputState;
use toru_core::store::CaptureId;

use super::{MainWindow, captures};
use crate::editor::skia;
use crate::editor::{Editor, EditorEvent, document::Document};

/// How the `editor` start-up argument opens the editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorStart {
    Plain,
    /// With the sample marks of the Editor mockup placed on the picture.
    Demo,
}

impl MainWindow {
    /// Opens the card with `id` in the editor. Its picture is read back from where the store
    /// keeps it, on the capture queue, since a Save queued ahead moves it; if it can't be read
    /// the editor doesn't open and a toast says so, as Copy does. "Open editor after capture"
    /// calls this with the new card's id.
    pub fn open_editor(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some((name, stored)) = self
            .gallery
            .read(cx)
            .card(id)
            .map(|card| (card.name.clone(), card.files.id))
        else {
            return;
        };
        // A capture opened by "Open editor after capture" would be counted, and cleared, by the
        // Clear unsaved question, which the editor leaves no way back to anyway. Closed now, not
        // when the editor appears, so a Clear can't be queued while the picture loads.
        if self.clear_prompt {
            self.clear_prompt = false;
            cx.notify();
        }
        let store = self.store.clone();
        let window = self.window;
        self.enqueue(cx, async move |this, cx| {
            let picture = cx
                .background_spawn(async move {
                    let base = skia::load_picture(&store.full_image_path(stored).ok()?)?;
                    let redaction = store.redaction(stored).unwrap_or_default();
                    // One whose delete failed waits for the next start, unused.
                    let original = (!redaction.original_removed)
                        .then(|| skia::load_picture(&store.original_path(stored)))
                        .flatten();
                    let in_file = !store.image_path(stored).is_file();
                    Some((base, original, in_file, redaction))
                })
                .await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| match picture {
                    Some((base, original, in_file, redaction)) => {
                        let document =
                            Document::opened(base, original, in_file).with_redaction(redaction);
                        this.show_editor(stored, name, document, window, cx)
                    }
                    None => this.report_failure(captures::FILE_GONE, cx),
                })
            })
            .ok();
        });
    }

    fn show_editor(
        &mut self,
        stored: CaptureId,
        name: Entity<InputState>,
        document: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_editor(window, cx);
        // And again here, for a question asked while the editor's picture was loading.
        self.clear_prompt = false;
        let demo = std::mem::take(&mut self.demo_marks);
        let editor = cx.new(|cx| {
            let mut editor = Editor::new(stored, name, document, cx);
            if demo {
                editor.add_demo_marks(cx);
            }
            editor
        });
        cx.subscribe_in(
            &editor,
            window,
            |this, editor, event: &EditorEvent, window, cx| {
                this.editor_event(editor, event, window, cx)
            },
        )
        .detach();
        // A dialog open over the window keeps the keyboard, so Esc and Tab still reach it; the
        // editor gets focus when the dialog closes, through `on_focus_lost`.
        if !self.dialog_open() {
            editor.focus_handle(cx).focus(window, cx);
        }
        self.editor = Some(editor);
        cx.notify();
    }

    /// The `editor` start-up argument: straight into the editor on the newest card. History
    /// loads after the window opens, so with no card yet the first to arrive is opened.
    pub fn open_newest_in_editor(&mut self, start: EditorStart, cx: &mut Context<Self>) {
        let newest = self
            .gallery
            .read(cx)
            .groups()
            .flat_map(|group| group.cards.first())
            .next()
            .map(|card| card.id);
        self.editor_awaits_history = newest.is_none().then_some(start);
        if let Some(id) = newest {
            self.demo_marks = start == EditorStart::Demo;
            self.open_editor(id, cx);
        }
    }

    /// Whether the editor is open with annotations that closing it would lose.
    pub(super) fn editor_has_edits(&self, cx: &App) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_edited())
    }

    /// Back to session. Focus returns to the card that was in the editor, so the arrow keys
    /// carry on from it, and the picture leaves the GPU. If that card is no longer listed,
    /// `on_focus_lost` finds focus a place. A dialog open over the window keeps the focus.
    pub(super) fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.take() {
            let stored = editor.update(cx, |editor, _| {
                editor.release_images(window);
                editor.stored()
            });
            if !self.dialog_open() {
                self.gallery
                    .update(cx, |gallery, cx| gallery.focus_capture(stored, window, cx));
            }
        }
        cx.notify();
    }
}
