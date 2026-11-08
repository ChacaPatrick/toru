//! What the cards' buttons and the bottom bar do to real captures: Save, Save all, Copy all,
//! Clear unsaved and Open folder. Anything that writes goes on the capture queue (`enqueue`), so
//! it happens in the order the user asked for it and nothing else can touch a capture meanwhile.

use std::{cell::Cell, fs, path::PathBuf, rc::Rc, sync::Arc};

use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext, ContentFormat};
use gpui::{AsyncApp, Context, WeakEntity, prelude::*};
use toru_core::{
    export::{CopySet, SavedFile, prepare_copy_set, save_capture},
    settings::SaveFormat,
    store::{CaptureId, Result as StoreResult, SessionId, Store},
};

use super::{MainWindow, captures::COPY_FAILED};
use crate::platform;
use crate::ui::Toast;

const SAVE_FAILED: &str = "Couldn't save. Check the Save to folder and its free space.";
const NO_SAVE_FOLDER: &str = "Couldn't find your Pictures folder to save to.";
const COPY_ALL_FAILED: &str = "Couldn't copy those files. Check for free space.";
const CLEAR_FAILED: &str = "Couldn't clear the unsaved captures";
const OPEN_FOLDER_FAILED: &str = "Couldn't open that folder. It may have moved.";

/// Where and how captures are saved, fixed when the user asks.
#[derive(Clone)]
struct SaveTarget {
    store: Arc<Store>,
    folder: PathBuf,
    format: SaveFormat,
}

impl MainWindow {
    fn save_target(&mut self, cx: &mut Context<Self>) -> Option<SaveTarget> {
        let Some(folder) = self.save_folder() else {
            self.show_toast(Toast::failed(NO_SAVE_FOLDER), cx);
            return None;
        };
        Some(SaveTarget {
            store: self.store.clone(),
            folder,
            format: self.settings.format,
        })
    }

    /// A card's Save. A second press before the first finishes finds it saved and does nothing.
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

    /// Saves each unsaved capture as its own job, so a capture or rename made meanwhile waits for
    /// one file rather than all of them, and one failure doesn't stop the rest.
    pub(super) fn save_all(&mut self, cx: &mut Context<Self>) {
        let captures = self.gallery.read(cx).unsaved_captures();
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

    /// Puts this session's captures on the clipboard as files.
    pub(super) fn copy_all(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let session = self.session;
        let format = self.settings.format;
        self.enqueue(cx, async move |this, cx| {
            let copied = cx
                .background_spawn(async move { copy_session(&store, session, format, copy_files) })
                .await;
            this.update(cx, |this, cx| this.finish_copy_all(copied, cx))
                .ok();
        });
    }

    fn finish_copy_all(&mut self, copied: Result<CopyAll, &'static str>, cx: &mut Context<Self>) {
        match copied {
            Err(message) => self.report_failure(message, cx),
            Ok(CopyAll::Nothing) => self.show_toast(Toast::new("Nothing to copy"), cx),
            Ok(CopyAll::AllMissing(count)) => {
                let message = format!("Nothing copied. {}", missing_files(count));
                self.report_failure(message, cx);
            }
            Ok(CopyAll::Copied { missing: 0 }) => self.report_copy(Ok(()), cx),
            Ok(CopyAll::Copied { missing }) => {
                let message = format!("Copied, but {}", missing_files(missing));
                self.report_failure(message, cx);
            }
        }
    }

    /// Bottom bar's Clear unsaved. Runs on the queue so a capture still being stored is cleared
    /// with the rest, and a Save already asked for is finished first.
    pub(super) fn clear_unsaved(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let session = self.session;
        self.enqueue(cx, async move |this, cx| {
            let cleared = cx
                .background_spawn(async move { store.discard_unsaved(session) })
                .await;
            this.update(cx, |this, cx| {
                if cleared.is_err() {
                    this.report_failure(CLEAR_FAILED, cx);
                    return;
                }
                let removed = this
                    .gallery
                    .update(cx, |gallery, cx| gallery.remove_unsaved(cx));
                let message = match removed {
                    0 => "Nothing unsaved to clear".to_string(),
                    1 => "Cleared 1 unsaved capture".to_string(),
                    count => format!("Cleared {count} unsaved captures"),
                };
                this.show_toast(Toast::new(message), cx);
            })
            .ok();
        });
    }

    /// A saved card's Open folder.
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

    /// Where captures are saved: the setting, else Pictures\Screenshots. `None` only if Windows
    /// can't say where Pictures is.
    pub(super) fn save_folder(&self) -> Option<PathBuf> {
        self.settings
            .save_folder
            .clone()
            .or_else(|| self.default_save_folder.clone())
    }
}

/// Saves `capture` and turns its card to Saved. `None` means it was already saved.
async fn save_and_show(
    this: &WeakEntity<MainWindow>,
    cx: &mut AsyncApp,
    target: SaveTarget,
    capture: CaptureId,
) -> StoreResult<Option<SavedFile>> {
    let saved = cx
        .background_spawn(async move {
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

/// How a Copy all went, short of the failures that are told to the user.
#[derive(Debug, PartialEq)]
enum CopyAll {
    Nothing,
    /// Every capture is saved and every one of their files has gone: how many.
    AllMissing(usize),
    Copied {
        missing: usize,
    },
}

/// How putting a file list on the clipboard went.
#[derive(Clone, Copy, PartialEq)]
enum FileCopy {
    /// The clipboard wasn't touched, so it still lists whatever it did.
    Refused,
    /// The clipboard was emptied but didn't take the list.
    Lost,
    Listed,
}

/// The error is what to tell the user. `copy` puts the list on the clipboard. Only a clipboard
/// that wasn't touched loses the new folder: anything else may be listing it, and the set before
/// is off the clipboard either way, so it goes instead.
fn copy_session(
    store: &Store,
    session: SessionId,
    format: SaveFormat,
    copy: impl FnOnce(Vec<String>) -> FileCopy,
) -> Result<CopyAll, &'static str> {
    let CopySet {
        folder,
        files,
        missing,
    } = prepare_copy_set(store, session, format).map_err(|_| COPY_ALL_FAILED)?;
    if files.is_empty() {
        let _ = fs::remove_dir_all(&folder);
        return Ok(match missing {
            0 => CopyAll::Nothing,
            count => CopyAll::AllMissing(count),
        });
    }
    let copied = copy(files);
    if copied == FileCopy::Refused {
        let _ = fs::remove_dir_all(&folder);
        return Err(COPY_FAILED);
    }
    store.remove_other_copy_folders(&folder);
    if copied == FileCopy::Lost {
        return Err(COPY_FAILED);
    }
    Ok(CopyAll::Copied { missing })
}

/// `set` clears the clipboard first and fails if it can't, which `set_files` doesn't, but it
/// ignores a failed write, so the list is read back (`docs/spikes.md` §1.4). A clipboard manager
/// can hold the clipboard through the read, and a failed read proves nothing while the clipboard
/// still offers a file list.
fn copy_files(files: Vec<String>) -> FileCopy {
    let Ok(clipboard) = ClipboardContext::new() else {
        return FileCopy::Refused;
    };
    if clipboard
        .set(vec![ClipboardContent::Files(files.clone())])
        .is_err()
    {
        return FileCopy::Refused;
    }
    let listed = match clipboard.get_files() {
        Ok(listed) => listed == files,
        Err(_) => clipboard.has(ContentFormat::Files),
    };
    if listed {
        FileCopy::Listed
    } else {
        FileCopy::Lost
    }
}

fn saved_message(file: &SavedFile) -> String {
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

/// What a copy left out because saved files have gone.
fn missing_files(count: usize) -> String {
    if count == 1 {
        "1 saved file has moved or been deleted.".to_string()
    } else {
        format!("{count} saved files have moved or been deleted.")
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use toru_core::naming::DEFAULT_PATTERN;

    use super::super::captures::tests::{TestDir, add};
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
    fn missing_files_are_counted() {
        assert_eq!(missing_files(1), "1 saved file has moved or been deleted.");
        assert_eq!(
            missing_files(2),
            "2 saved files have moved or been deleted."
        );
    }

    fn copy_folders(dir: &TestDir) -> Vec<PathBuf> {
        fs::read_dir(dir.0.join("copies"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }

    /// A store with one unsaved capture in a session.
    fn session_with_a_capture(dir: &TestDir) -> (Store, SessionId) {
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, DEFAULT_PATTERN).unwrap();
        (store, session)
    }

    #[test]
    fn a_refused_copy_deletes_the_new_folder_and_keeps_the_one_before() {
        let dir = TestDir::new("refused");
        let (store, session) = session_with_a_capture(&dir);
        let copied = copy_session(&store, session, SaveFormat::Png, |_| FileCopy::Listed);
        assert_eq!(copied, Ok(CopyAll::Copied { missing: 0 }));
        let before = copy_folders(&dir);

        let copied = copy_session(&store, session, SaveFormat::Png, |_| FileCopy::Refused);

        assert_eq!(copied, Err(COPY_FAILED));
        assert_eq!(copy_folders(&dir), before);
    }

    #[test]
    fn a_copy_replaces_the_folder_before() {
        let dir = TestDir::new("replaces");
        let (store, session) = session_with_a_capture(&dir);
        copy_session(&store, session, SaveFormat::Png, |_| FileCopy::Listed).unwrap();
        let before = copy_folders(&dir);

        copy_session(&store, session, SaveFormat::Png, |_| FileCopy::Listed).unwrap();

        let after = copy_folders(&dir);
        assert_eq!(after.len(), 1);
        assert_ne!(after, before);
    }

    #[test]
    fn a_list_the_clipboard_lost_is_reported_but_its_folder_stays() {
        let dir = TestDir::new("lost");
        let (store, session) = session_with_a_capture(&dir);
        let mut listed = Vec::new();

        let copied = copy_session(&store, session, SaveFormat::Png, |files| {
            listed = files;
            FileCopy::Lost
        });

        assert_eq!(copied, Err(COPY_FAILED));
        assert!(Path::new(&listed[0]).is_file());
    }

    #[test]
    fn a_session_whose_saved_files_have_all_gone_reports_them() {
        let dir = TestDir::new("all-missing");
        let (store, session) = session_with_a_capture(&dir);
        let id = store.session_captures(session).unwrap()[0].id;
        let target = dir.0.join("Screenshots");
        let saved = save_capture(&store, id, &target, SaveFormat::Png)
            .unwrap()
            .unwrap();
        fs::remove_file(saved.path).unwrap();

        let copied = copy_session(&store, session, SaveFormat::Png, |_| {
            panic!("there was nothing to put on the clipboard")
        });

        assert_eq!(copied, Ok(CopyAll::AllMissing(1)));
        assert!(copy_folders(&dir).is_empty());
    }

    #[test]
    fn an_empty_session_has_nothing_to_copy() {
        let dir = TestDir::new("empty-copy");
        let store = dir.store();
        let session = store.new_session().unwrap();

        let copied = copy_session(&store, session, SaveFormat::Png, |_| {
            panic!("there was nothing to put on the clipboard")
        });

        assert_eq!(copied, Ok(CopyAll::Nothing));
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
