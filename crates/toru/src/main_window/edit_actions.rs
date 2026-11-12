//! What the editor's top bar does to the capture (DESIGN.md §5): Copy, Save, Save as, and the
//! edited picture going to the card on Back. The picture is flattened by the editor as the user
//! asks; everything that touches the clipboard or a file goes on the capture queue.

use std::path::Path;

use gpui::{Context, Entity, WeakEntity, Window, prelude::*};
use rfd::AsyncFileDialog;
use toru_core::{
    export::{SaveEditedError, SavedFile, save_copy, save_edited},
    settings::SaveFormat,
    store::{CaptureId, Result as StoreResult, SessionId, Store, StoreError},
};

use super::{
    MainWindow,
    actions::{SAVE_FAILED, saved_message},
    captures::{check_chosen_folder, copy_image},
    closing::{WriteAtShutdown, store_flattened},
};
use crate::editor::{
    Editor, EditorEvent,
    document::{Flattened, to_rgba},
};
use crate::{theme::ActiveTheme, ui::Toast};

const SAVE_AS_FAILED: &str = "Couldn't save a copy there. Check the folder and its free space.";
const KEEP_EDITS_FAILED: &str = "Couldn't keep those edits. Check for free space.";
const NOTHING_TO_SAVE: &str = "Nothing new to save";
const BLANKS_KEPT: &str = "Edits kept · some characters left blank";
/// Only when an editor change made while a redacted picture was being saved takes the
/// redaction off: the store refuses it, since the original is gone.
const WOULD_REVEAL: &str = "Not kept: that would show what the saved picture hides.";

/// What to say when a write failed: `otherwise`, unless the store refused it for showing what
/// a saved redaction hid.
fn failure_message(error: &StoreError, otherwise: &'static str) -> &'static str {
    match error {
        StoreError::WouldReveal => WOULD_REVEAL,
        _ => otherwise,
    }
}

/// `message`, saying so when some text in the picture has characters no font could draw.
fn noting_blanks(message: impl Into<String>, blanks: bool) -> String {
    let message = message.into();
    if blanks {
        format!("{message} · some characters left blank")
    } else {
        message
    }
}

impl MainWindow {
    pub(super) fn editor_event(
        &mut self,
        editor: &Entity<Editor>,
        event: &EditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capture = editor.read(cx).stored();
        match event {
            EditorEvent::Back(changed) => {
                if let Some(flattened) = changed {
                    self.keep_edits(capture, flattened.clone(), cx);
                }
                self.close_editor(window, cx);
            }
            EditorEvent::Copy(flattened) => {
                self.copy_edited(editor.downgrade(), flattened.clone(), cx)
            }
            EditorEvent::Save(flattened) => {
                self.save_edited(editor.downgrade(), capture, flattened.clone(), cx)
            }
            EditorEvent::NothingToSave => self.show_toast(Toast::new(NOTHING_TO_SAVE), cx),
            EditorEvent::SaveAs(flattened, format) => self.save_edited_as(
                editor.downgrade(),
                capture,
                flattened.clone(),
                *format,
                window,
                cx,
            ),
        }
    }

    fn copy_edited(
        &mut self,
        editor: WeakEntity<Editor>,
        flattened: Flattened,
        cx: &mut Context<Self>,
    ) {
        let blanks = flattened.edits.has_blanks();
        self.enqueue(cx, async move |this, cx| {
            let picture = flattened.picture;
            let copied = cx
                .background_spawn(async move { copy_image(to_rgba(&picture)) })
                .await;
            this.update(cx, |this, cx| {
                if copied.is_ok() {
                    editor.update(cx, |editor, cx| editor.show_copied(cx)).ok();
                }
                if copied.is_ok() && blanks {
                    let message = noting_blanks(cx.theme().strings.copied, true);
                    this.show_toast(Toast::new(message), cx);
                } else {
                    this.report_copy(copied, cx);
                }
            })
            .ok();
        });
    }

    /// Save: the card's capture becomes the flattened picture, in the user's folder. A second
    /// press before the first finishes does nothing, or it would write a second file.
    fn save_edited(
        &mut self,
        editor: WeakEntity<Editor>,
        capture: CaptureId,
        flattened: Flattened,
        cx: &mut Context<Self>,
    ) {
        if self.saving_edit {
            return;
        }
        let Some(target) = self.save_target(cx) else {
            return;
        };
        self.saving_edit = true;
        let thumbnail_width = self
            .gallery
            .read(cx)
            .thumbnail_width((flattened.picture.width(), flattened.picture.height()));
        let window = self.window;
        self.enqueue(cx, async move |this, cx| {
            let Flattened {
                picture,
                original,
                is_edited,
                redacted,
                edits,
            } = flattened;
            let size = (picture.width(), picture.height());
            let blanks = edits.has_blanks();
            let saved_picture = picture.clone();
            let saved = cx
                .background_spawn(async move {
                    check_chosen_folder(&target.folder, target.chosen)?;
                    let original = is_edited.then(|| to_rgba(&original));
                    let file = save_edited(
                        &target.store,
                        capture,
                        &to_rgba(&picture),
                        original.as_ref(),
                        redacted,
                        &target.folder,
                        target.format,
                        thumbnail_width,
                    )?;
                    let removed = target
                        .store
                        .redaction(capture)
                        .is_ok_and(|redaction| redaction.original_removed);
                    Ok::<_, SaveEditedError>((file, removed))
                })
                .await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| {
                    this.saving_edit = false;
                    let (file, removed) = match saved {
                        Ok(saved) => saved,
                        Err(SaveEditedError { error, stored }) => {
                            // A redacted picture goes on the card before the file is written,
                            // so the card and the editor learn it shows it even so.
                            if let Some(bytes) = stored {
                                this.gallery.update(cx, |gallery, cx| {
                                    gallery.picture_changed(capture, size, bytes, window, cx)
                                });
                                editor.update(cx, |editor, _| editor.written(edits)).ok();
                            }
                            return this.report_failure(failure_message(&error, SAVE_FAILED), cx);
                        }
                    };
                    let message = noting_blanks(saved_message(&file), blanks);
                    this.show_toast(Toast::new(message), cx);
                    this.gallery.update(cx, |gallery, cx| {
                        gallery.picture_changed(capture, size, file.bytes, window, cx);
                        gallery.mark_saved(capture, file, cx);
                    });
                    editor
                        .update(cx, |editor, cx| {
                            editor.saved(edits.clone(), cx);
                            if removed {
                                editor.original_removed(saved_picture, &edits, true, cx);
                            }
                        })
                        .ok();
                })
            })
            .ok();
        });
    }

    /// Save as: the file picker runs on a thread of its own, so the window keeps drawing, and
    /// off the queue, so a capture needn't wait for the user to choose. Only the write queues.
    /// A copy of a picture that hides some of the capture deletes the original, as Save does.
    fn save_edited_as(
        &mut self,
        editor: WeakEntity<Editor>,
        capture: CaptureId,
        flattened: Flattened,
        format: SaveFormat,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving_edit {
            return;
        }
        self.saving_edit = true;
        let blanks = flattened.edits.has_blanks();
        let extension = format.extension();
        let name = self
            .store
            .capture(capture)
            .map_or_else(|_| "screenshot".to_string(), |capture| capture.name);
        let mut dialog = AsyncFileDialog::new()
            .set_title("Save a copy as")
            .set_file_name(format!("{name}.{extension}"))
            .add_filter(extension.to_uppercase(), &[extension])
            .set_parent(window);
        if let Some(folder) = self.save_folder() {
            dialog = dialog.set_directory(folder);
        }
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let Some(file) = dialog.save_file().await else {
                this.update(cx, |this, _| this.saving_edit = false).ok();
                return;
            };
            this.update(cx, |this, cx| {
                this.enqueue(cx, async move |this, cx| {
                    let Flattened {
                        picture,
                        redacted,
                        edits,
                        ..
                    } = flattened;
                    let copied = picture.clone();
                    let saved = cx
                        .background_spawn(async move {
                            store.check_not_revealing(capture, redacted)?;
                            let saved = write_copy(&to_rgba(&picture), file.path(), format)?;
                            // The copy is what was asked for; an original left is only kept.
                            let removed = redacted
                                && store.remove_redacted_original(capture).unwrap_or(false);
                            StoreResult::Ok((saved, removed))
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        this.saving_edit = false;
                        match saved {
                            Ok((file, removed)) => {
                                let message = noting_blanks(saved_message(&file), blanks);
                                this.show_toast(Toast::new(message), cx);
                                if removed {
                                    editor
                                        .update(cx, |editor, cx| {
                                            editor.original_removed(copied, &edits, false, cx)
                                        })
                                        .ok();
                                }
                            }
                            Err(error) => {
                                this.report_failure(failure_message(&error, SAVE_AS_FAILED), cx)
                            }
                        }
                    })
                    .ok();
                });
            })
            .ok();
        })
        .detach();
    }

    /// Quitting with the editor open: its edits go to the card as Back would put them, on the
    /// queue, which quitting waits for. The editor stays open, in case quitting is cancelled.
    pub(super) fn keep_open_edits(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let (capture, flattened) = editor.update(cx, |editor, cx| {
            (editor.stored(), editor.take_unwritten(cx))
        });
        if let Some(flattened) = flattened {
            self.keep_edits(capture, flattened, cx);
        }
    }

    /// Back: the card takes the edited picture, leaving any file the user saved as it is.
    /// `None` for the original tells the store the edits were all taken off again. Until the
    /// write starts, Windows' shutdown can write it instead (`closing`).
    fn keep_edits(&mut self, capture: CaptureId, flattened: Flattened, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let thumbnail_width = self
            .gallery
            .read(cx)
            .thumbnail_width((flattened.picture.width(), flattened.picture.height()));
        let window = self.window;
        let blanks = flattened.edits.has_blanks();
        let size = (flattened.picture.width(), flattened.picture.height());
        let edits = flattened.edits.clone();
        let held = self.hold_until_started(KeptEdits {
            capture,
            flattened,
            thumbnail_width,
        });
        self.enqueue(cx, async move |this, cx| {
            let Some(kept) = held.take() else {
                return;
            };
            let stored = cx.background_spawn(async move { kept.store(&store) }).await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| match stored {
                    Ok(bytes) => {
                        this.gallery.update(cx, |gallery, cx| {
                            gallery.picture_changed(capture, size, bytes, window, cx)
                        });
                        if blanks {
                            this.show_toast(Toast::new(BLANKS_KEPT), cx);
                        }
                    }
                    Err(error) => {
                        // Quitting keeps the editor open, and may be cancelled: Back tries again.
                        if let Some(editor) = this.editor.clone() {
                            editor.update(cx, |editor, _| {
                                if editor.stored() == capture {
                                    editor.write_failed(&edits);
                                }
                            });
                        }
                        this.report_failure(failure_message(&error, KEEP_EDITS_FAILED), cx)
                    }
                })
            })
            .ok();
        });
    }
}

/// Back's write, held for the quit handler until the queue starts it.
struct KeptEdits {
    capture: CaptureId,
    flattened: Flattened,
    thumbnail_width: u32,
}

impl KeptEdits {
    fn store(&self, store: &Store) -> StoreResult<u64> {
        store_flattened(store, self.capture, &self.flattened, self.thumbnail_width)
    }
}

impl WriteAtShutdown for KeptEdits {
    /// A failure goes unreported, as there is no window left to say so in.
    fn write(self, store: &Store, _session: SessionId) {
        self.store(store).ok();
    }
}

/// Save as's write: the name picked, with the extension of the format chosen in the menu. Toru
/// never overwrites, so a name already taken gets a number even if the picker asked to replace.
fn write_copy(
    picture: &xcap::image::RgbaImage,
    path: &Path,
    format: SaveFormat,
) -> StoreResult<SavedFile> {
    let folder = path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty());
    let name = path.file_stem().and_then(|name| name.to_str());
    let (Some(folder), Some(name)) = (folder, name) else {
        return Err(std::io::Error::other("no such place to save").into());
    };
    save_copy(picture, folder, name, format)
}
