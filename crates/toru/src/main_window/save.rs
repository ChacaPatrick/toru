use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use gpui::{AsyncApp, Context, WeakEntity, Window, prelude::*};
use rfd::AsyncFileDialog;
use toru_core::{
    export::{SavedFile, save_capture},
    settings::SaveFormat,
    store::{CaptureId, Result as StoreResult, Store},
};

use super::{MainWindow, captures::check_chosen_folder};
use crate::platform;
use crate::ui::Toast;

pub(super) const SAVE_FAILED: &str = "Couldn't save. Check the Save to folder and its free space.";

const NO_SAVE_FOLDER: &str = "Couldn't find your Pictures folder to save to.";

const OPEN_FOLDER_FAILED: &str = "Couldn't open that folder. It may have moved.";

const BAD_FOLDER: &str = "Toru can't use a folder with that name";

/// Fixed when the user asks.
#[derive(Clone)]
pub(super) struct SaveTarget {
    pub(super) store: Arc<Store>,
    pub(super) folder: PathBuf,
    /// The user picked this folder, as opposed to the default, which is made when first needed.
    pub(super) chosen: bool,
    pub(super) format: SaveFormat,
}

impl MainWindow {
    pub(super) fn save_target(&mut self, cx: &mut Context<Self>) -> Option<SaveTarget> {
        let Some(folder) = self.save_folder() else {
            self.show_toast(Toast::failed(NO_SAVE_FOLDER), cx);
            return None;
        };
        Some(SaveTarget {
            store: self.store.clone(),
            folder,
            chosen: self.settings.save_folder.is_some(),
            format: self.settings.format,
        })
    }

    /// A second press before the first finishes finds it saved and does nothing.
    pub(super) fn save_card(&mut self, capture: CaptureId, cx: &mut Context<Self>) {
        let Some(target) = self.save_target(cx) else {
            return;
        };
        self.enqueue(cx, async move |this, cx| {
            let saved = save_and_show(&this, cx, target, capture).await;
            this.update(cx, |this, cx| match saved {
                Ok(Some(file)) => this.show_toast(Toast::new(saved_message(&file)), cx),
                Ok(None) => {}
                Err(_) => this.report_failure(SAVE_FAILED, cx),
            })
            .ok();
        });
    }

    pub(super) fn save_all(&mut self, cx: &mut Context<Self>) {
        let captures = self.gallery.read(cx).unsaved_captures();
        self.save_captures(captures, cx);
    }

    pub(super) fn save_selected(&mut self, cx: &mut Context<Self>) {
        let captures = self.gallery.read(cx).selected_unsaved();
        self.save_captures(captures, cx);
    }

    /// Each capture is its own job, so a capture or rename made meanwhile waits for one file
    /// rather than all of them, and one failure doesn't stop the rest.
    fn save_captures(&mut self, captures: Vec<CaptureId>, cx: &mut Context<Self>) {
        if captures.is_empty() {
            self.show_toast(Toast::new("Nothing new to save"), cx);
            return;
        }
        let Some(target) = self.save_target(cx) else {
            return;
        };
        let total = captures.len();
        let failed = Rc::new(Cell::new(0));
        for capture in captures {
            let (target, failed) = (target.clone(), failed.clone());
            self.enqueue(cx, async move |this, cx| {
                if save_and_show(&this, cx, target, capture).await.is_err() {
                    failed.set(failed.get() + 1);
                }
            });
        }
        self.enqueue(cx, async move |this, cx| {
            this.update(cx, |this, cx| {
                let message = save_all_message(total, failed.get());
                if failed.get() == 0 {
                    this.show_toast(Toast::new(message), cx);
                } else {
                    this.report_failure(message, cx);
                }
            })
            .ok();
        });
    }

    /// A pick lands in the setting whatever happens to the folder afterwards; saving to it is what
    /// finds out it has gone (`check_chosen_folder`).
    pub(super) fn change_save_folder(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.picking_folder {
            return;
        }
        self.picking_folder = true;
        let mut dialog = AsyncFileDialog::new()
            .set_title("Choose where to save")
            .set_parent(window);
        if let Some(current) = self.save_folder() {
            dialog = dialog.set_directory(current);
        }
        cx.spawn(async move |this, cx| {
            let picked = dialog.pick_folder().await;
            this.update(cx, |this, cx| {
                this.picking_folder = false;
                if let Some(folder) = picked {
                    this.set_save_folder(folder.path(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn set_save_folder(&mut self, folder: &Path, cx: &mut Context<Self>) {
        // The store keeps only Unicode paths, and one it refused would fail every later save
        // of the settings along with it.
        if folder.to_str().is_none() {
            self.report_failure(BAD_FOLDER, cx);
            return;
        }
        let folder = folder.to_path_buf();
        self.change_settings(cx, |settings| settings.save_folder = Some(folder));
    }

    pub(super) fn open_folder(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_spawn(async move { platform::reveal_file(&file) })
                .await;
            if opened.is_err() {
                this.update(cx, |this, cx| this.report_failure(OPEN_FOLDER_FAILED, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// `None` only if Windows can't say where Pictures is.
    pub(super) fn save_folder(&self) -> Option<PathBuf> {
        self.settings
            .save_folder
            .clone()
            .or_else(|| self.default_save_folder.clone())
    }
}

/// `None` means it was already saved.
async fn save_and_show(
    this: &WeakEntity<MainWindow>,
    cx: &mut AsyncApp,
    target: SaveTarget,
    capture: CaptureId,
) -> StoreResult<Option<SavedFile>> {
    let saved = cx
        .background_spawn(async move {
            check_chosen_folder(&target.folder, target.chosen)?;
            save_capture(&target.store, capture, &target.folder, target.format)
        })
        .await;
    if let Ok(Some(file)) = &saved {
        let file = file.clone();
        this.update(cx, |this, cx| {
            this.gallery
                .update(cx, |gallery, cx| gallery.mark_saved(capture, file, cx))
        })
        .ok();
    }
    saved
}

pub(super) fn saved_message(file: &SavedFile) -> String {
    let name = file
        .path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    format!("Saved as {name}")
}

fn save_all_message(total: usize, failed: usize) -> String {
    match failed {
        0 if total == 1 => "Saved 1 capture".to_string(),
        0 => format!("Saved {total} captures"),
        _ => format!(
            "Saved {} of {total}. Check the Save to folder and its free space.",
            total - failed
        ),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn save_all_says_how_it_went() {
        assert_eq!(save_all_message(1, 0), "Saved 1 capture");
        assert_eq!(save_all_message(3, 0), "Saved 3 captures");
        assert_eq!(
            save_all_message(3, 1),
            "Saved 2 of 3. Check the Save to folder and its free space."
        );
    }

    #[test]
    fn the_saved_message_names_the_file() {
        let file = SavedFile {
            path: PathBuf::from(r"C:\Pictures\shot_2.png"),
            bytes: 1,
        };
        assert_eq!(saved_message(&file), "Saved as shot_2.png");
    }
}
