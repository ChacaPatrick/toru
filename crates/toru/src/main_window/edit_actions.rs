//! What the editor's top bar does to the capture (DESIGN.md §5): Copy, Save, Save as, and the
//! edited picture going to the card on Back. The picture is flattened by the editor as the user
//! asks; everything that touches the clipboard or a file goes on the capture queue.

use std::path::Path;

use gpui::{Context, Entity, WeakEntity, Window, prelude::*};
use rfd::AsyncFileDialog;
use toru_core::{
    export::{SavedFile, save_copy, save_edited},
    settings::SaveFormat,
    store::{CaptureId, Result as StoreResult},
};

use super::{
    MainWindow,
    actions::{SAVE_FAILED, saved_message},
    captures::{check_chosen_folder, copy_image},
};
use crate::editor::{
    Editor, EditorEvent,
    document::{Flattened, to_rgba},
};
use crate::ui::Toast;

const SAVE_AS_FAILED: &str = "Couldn't save a copy there. Check the folder and its free space.";
const KEEP_EDITS_FAILED: &str = "Couldn't keep those edits. Check for free space.";
const NOTHING_TO_SAVE: &str = "Nothing new to save";

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
            EditorEvent::SaveAs(flattened, format) => {
                self.save_edited_as(capture, flattened.clone(), *format, window, cx)
            }
        }
    }

    fn copy_edited(
        &mut self,
        editor: WeakEntity<Editor>,
        flattened: Flattened,
        cx: &mut Context<Self>,
    ) {
        self.enqueue(cx, async move |this, cx| {
            let picture = flattened.picture;
            let copied = cx
                .background_spawn(async move { copy_image(to_rgba(&picture)) })
                .await;
            this.update(cx, |this, cx| {
                if copied.is_ok() {
                    editor.update(cx, |editor, cx| editor.show_copied(cx)).ok();
                }
                this.report_copy(copied, cx);
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
                edits,
            } = flattened;
            let size = (picture.width(), picture.height());
            let saved = cx
                .background_spawn(async move {
                    check_chosen_folder(&target.folder, target.chosen)?;
                    let original = is_edited.then(|| to_rgba(&original));
                    save_edited(
                        &target.store,
                        capture,
                        &to_rgba(&picture),
                        original.as_ref(),
                        &target.folder,
                        target.format,
                        thumbnail_width,
                    )
                })
                .await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| {
                    this.saving_edit = false;
                    let file = match saved {
                        Ok(file) => file,
                        Err(_) => return this.report_failure(SAVE_FAILED, cx),
                    };
                    this.show_toast(Toast::new(saved_message(&file)), cx);
                    this.gallery.update(cx, |gallery, cx| {
                        gallery.picture_changed(capture, size, file.bytes, window, cx);
                        gallery.mark_saved(capture, file, cx);
                    });
                    editor.update(cx, |editor, cx| editor.saved(edits, cx)).ok();
                })
            })
            .ok();
        });
    }

    /// Save as: the file picker runs on a thread of its own, so the window keeps drawing, and
    /// off the queue, so a capture needn't wait for the user to choose. Only the write queues.
    fn save_edited_as(
        &mut self,
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
        cx.spawn(async move |this, cx| {
            let Some(file) = dialog.save_file().await else {
                this.update(cx, |this, _| this.saving_edit = false).ok();
                return;
            };
            this.update(cx, |this, cx| {
                this.enqueue(cx, async move |this, cx| {
                    let picture = flattened.picture;
                    let saved = cx
                        .background_spawn(async move {
                            write_copy(&to_rgba(&picture), file.path(), format)
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        this.saving_edit = false;
                        match saved {
                            Ok(file) => this.show_toast(Toast::new(saved_message(&file)), cx),
                            Err(_) => this.report_failure(SAVE_AS_FAILED, cx),
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
    /// `None` for the original tells the store the edits were all taken off again.
    fn keep_edits(&mut self, capture: CaptureId, flattened: Flattened, cx: &mut Context<Self>) {
        let store = self.store.clone();
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
                ..
            } = flattened;
            let size = (picture.width(), picture.height());
            let stored = cx
                .background_spawn(async move {
                    let original = is_edited.then(|| to_rgba(&original));
                    store.store_edit(
                        capture,
                        &to_rgba(&picture),
                        original.as_ref(),
                        thumbnail_width,
                    )
                })
                .await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| match stored {
                    Ok(bytes) => this.gallery.update(cx, |gallery, cx| {
                        gallery.picture_changed(capture, size, bytes, window, cx)
                    }),
                    Err(_) => this.report_failure(KEEP_EDITS_FAILED, cx),
                })
            })
            .ok();
        });
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
