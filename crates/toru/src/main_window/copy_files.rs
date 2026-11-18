use std::{fs, path::PathBuf};

use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext, ContentFormat};
use gpui::{Context, prelude::*};
use toru_core::{
    export::{CopySet, prepare_copy_set},
    settings::SaveFormat,
    store::{CaptureId, SessionId, Store},
};

use super::{
    MainWindow,
    captures::{COPY_FAILED, CopyResult},
};
use crate::platform;
use crate::ui::Toast;

const COPY_ALL_FAILED: &str = "Couldn't copy those files. Check for free space.";

const NO_PATHS: &str = "None of those are saved yet, so they have no paths";

impl MainWindow {
    /// Puts this session's captures on the clipboard as files.
    pub(super) fn copy_all(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let format = self.settings.format;
        self.enqueue(cx, async move |this, cx| {
            let Ok(session) = this.read_with(cx, |this, _| this.session) else {
                return;
            };
            let copied = cx
                .background_spawn(async move { copy_session(&store, session, format, copy_files) })
                .await;
            this.update(cx, |this, cx| this.finish_copy_all(copied, cx))
                .ok();
        });
    }

    /// The bulk bar's Copy as files.
    pub(super) fn copy_selected(&mut self, cx: &mut Context<Self>) {
        let captures = self.gallery.read(cx).selected_captures();
        let store = self.store.clone();
        let format = self.settings.format;
        self.enqueue(cx, async move |this, cx| {
            let copied = cx
                .background_spawn(
                    async move { copy_captures(&store, &captures, format, copy_files) },
                )
                .await;
            this.update(cx, |this, cx| this.finish_copy_all(copied, cx))
                .ok();
        });
    }

    /// The bulk bar's Copy paths: the saved ones, one per line. The captures are the ones selected
    /// at the press; their paths are read on the queue, so a capture a Save ahead of it is still
    /// writing has its path by now.
    pub(super) fn copy_selected_paths(&mut self, cx: &mut Context<Self>) {
        let captures = self.gallery.read(cx).selected_captures();
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let copied = cx
                .background_spawn(async move {
                    let paths = saved_paths(&store, &captures);
                    (!paths.is_empty()).then(|| copy_text(paths_text(&paths)))
                })
                .await;
            this.update(cx, |this, cx| match copied {
                Some(copied) => this.report_copy(copied, cx),
                None => this.show_toast(Toast::new(NO_PATHS), cx),
            })
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

fn copy_session(
    store: &Store,
    session: SessionId,
    format: SaveFormat,
    copy: impl FnOnce(Vec<String>) -> FileCopy,
) -> Result<CopyAll, &'static str> {
    let captures: Vec<CaptureId> = store
        .session_captures(session)
        .map_err(|_| COPY_ALL_FAILED)?
        .iter()
        .map(|stored| stored.id)
        .collect();
    copy_captures(store, &captures, format, copy)
}

/// The error is what to tell the user. `copy` puts the list on the clipboard. Only a clipboard
/// that wasn't touched loses the new folder: anything else may be listing it, and the set before
/// is off the clipboard either way, so it goes instead.
fn copy_captures(
    store: &Store,
    captures: &[CaptureId],
    format: SaveFormat,
    copy: impl FnOnce(Vec<String>) -> FileCopy,
) -> Result<CopyAll, &'static str> {
    let CopySet {
        folder,
        files,
        missing,
    } = prepare_copy_set(store, captures, format).map_err(|_| COPY_ALL_FAILED)?;
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

/// The files these captures are saved to, in their order. One deleted meanwhile has none.
fn saved_paths(store: &Store, captures: &[CaptureId]) -> Vec<PathBuf> {
    captures
        .iter()
        .filter_map(|&capture| store.capture(capture).ok()?.saved_path)
        .collect()
}

/// Copy paths' text: one path per line.
fn paths_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(platform::LINE_BREAK)
}

/// `set` clears the clipboard first and fails if it can't, as `copy_files` needs.
fn copy_text(text: String) -> CopyResult {
    ClipboardContext::new()?.set(vec![ClipboardContent::Text(text)])
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

    use toru_core::export::save_capture;
    use toru_core::naming::DEFAULT_PATTERN;

    use super::*;
    use crate::main_window::captures::tests::{TestDir, add};

    #[test]
    fn paths_are_one_per_line() {
        let paths = [
            PathBuf::from(r"C:\Pictures\a shot.png"),
            PathBuf::from(r"C:\Pictures\b.png"),
        ];
        assert_eq!(
            paths_text(&paths),
            format!(
                r"C:\Pictures\a shot.png{}C:\Pictures\b.png",
                platform::LINE_BREAK
            )
        );
        assert_eq!(paths_text(&paths[..1]), r"C:\Pictures\a shot.png");
    }

    #[test]
    fn copy_paths_lists_the_chosen_captures_saved_files_as_they_are_now() {
        let dir = TestDir::new("paths");
        let (store, session) = session_with_a_capture(&dir);
        let unsaved = store.session_captures(session).unwrap()[0].id;
        let saved = add(&store, session, DEFAULT_PATTERN).unwrap().files.id;
        let deleted = add(&store, session, DEFAULT_PATTERN).unwrap().files.id;
        let not_chosen = add(&store, session, DEFAULT_PATTERN).unwrap().files.id;
        let target = dir.0.join("Screenshots");
        let file = save_capture(&store, saved, &target, SaveFormat::Png)
            .unwrap()
            .unwrap()
            .path;
        save_capture(&store, not_chosen, &target, SaveFormat::Png).unwrap();
        store.delete(deleted).unwrap();

        assert_eq!(saved_paths(&store, &[unsaved, saved, deleted]), [file]);
    }

    #[test]
    fn copying_a_selection_lists_only_those_captures() {
        let dir = TestDir::new("selection");
        let (store, session) = session_with_a_capture(&dir);
        let chosen = store.session_captures(session).unwrap()[0].id;
        add(&store, session, DEFAULT_PATTERN).unwrap();
        let mut listed = Vec::new();

        let copied = copy_captures(&store, &[chosen], SaveFormat::Png, |files| {
            listed = files;
            FileCopy::Listed
        });

        assert_eq!(copied, Ok(CopyAll::Copied { missing: 0 }));
        assert_eq!(listed.len(), 1);
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
}
