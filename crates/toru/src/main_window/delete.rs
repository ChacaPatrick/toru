//! Deleting captures (DESIGN.md §4), from the bulk bar or a card's `⋯` menu, one rule for both. It
//! asks once, for the whole set, since an unsaved capture can't be brought back and the question
//! is also where "Also delete files" is ticked; a saved capture otherwise leaves the gallery with
//! its file still in the user's folder. The store goes first, on the capture queue, so a Save
//! already asked for finishes first, and only then do the cards go. A file that Save writes after
//! the question opened wasn't mentioned in it, so the tick box doesn't reach it. A file the tick
//! box takes goes to the Recycle Bin, so it can still be brought back.

use std::{io, path::Path};

use gpui::{Context, prelude::*};
use toru_core::store::{CaptureId, Store};

use super::{MainWindow, act, gallery::plural};
use crate::platform;
use crate::ui::{Dialog, Toast};

/// A card on its way out, as it was when the user asked.
#[derive(Clone)]
pub struct DeleteCard {
    pub card: usize,
    pub capture: CaptureId,
    /// The question counts it as saved, and only then can "Also delete files" take its file.
    pub saved: bool,
}

/// The question on the screen.
pub(super) struct DeletePrompt {
    cards: Vec<DeleteCard>,
    with_files: bool,
}

/// How a delete went.
#[derive(Default, Debug, PartialEq)]
struct Deleted {
    cards: Vec<usize>,
    failed: usize,
    /// Cards that went but whose files couldn't be removed.
    files_kept: usize,
    /// A saved file was left in the user's folder on purpose.
    files_stay: bool,
}

fn delete_title(count: usize) -> String {
    format!("Delete {}?", plural(count, "capture"))
}

/// What the question says, for this many unsaved and saved captures.
fn delete_body(unsaved: usize, saved: usize) -> String {
    let unsaved_text = if unsaved == 1 {
        "1 unsaved capture can't be brought back.".to_string()
    } else {
        format!("{unsaved} unsaved captures can't be brought back.")
    };
    let saved_text = if saved == 1 {
        "1 saved capture leaves the gallery, and its file stays in your folder unless you tick \
         the box."
            .to_string()
    } else {
        format!(
            "{saved} saved captures leave the gallery, and their files stay in your folder \
             unless you tick the box."
        )
    };
    match (unsaved, saved) {
        (_, 0) => unsaved_text,
        (0, _) => saved_text,
        _ => format!("{unsaved_text} {saved_text}"),
    }
}

/// What the toast says once `count` captures have gone. `files_stay`: saved files were left in
/// the user's folder, which the toast says so nobody goes looking for a deleted file.
fn deleted_message(count: usize, files_stay: bool) -> String {
    let captures = plural(count, "capture");
    if files_stay {
        format!("Removed {captures}. Saved files stay in your folder.")
    } else {
        format!("Deleted {captures}")
    }
}

/// Deletes the cards' captures from the store, one failure not stopping the rest. A saved file
/// that goes is handed to `remove`, which in the app sends it to the Recycle Bin.
fn delete_stored(
    store: &Store,
    cards: Vec<DeleteCard>,
    with_files: bool,
    remove: fn(&Path) -> io::Result<()>,
) -> Deleted {
    let mut deleted = Deleted::default();
    for card in cards {
        let done = if with_files && card.saved {
            store
                .delete_with_file(card.capture, remove)
                .map(|file_gone| {
                    if !file_gone {
                        deleted.files_kept += 1;
                    }
                })
        } else {
            // Read now, since a Save ahead of this may have written the file after the question.
            deleted.files_stay |= store
                .capture(card.capture)
                .is_ok_and(|capture| capture.is_saved());
            store.delete(card.capture)
        };
        match done {
            Ok(()) => deleted.cards.push(card.card),
            Err(_) => deleted.failed += 1,
        }
    }
    deleted
}

impl MainWindow {
    /// The bulk bar's Delete.
    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let cards = self.gallery.read(cx).selected_for_delete();
        self.ask_delete(cards, cx);
    }

    /// A card's `⋯ → Delete`.
    pub(super) fn delete_card(&mut self, card: usize, cx: &mut Context<Self>) {
        let cards = self.gallery.read(cx).card_for_delete(card);
        self.ask_delete(cards.into_iter().collect(), cx);
    }

    fn ask_delete(&mut self, cards: Vec<DeleteCard>, cx: &mut Context<Self>) {
        if cards.is_empty() {
            return;
        }
        self.delete_prompt = Some(DeletePrompt {
            cards,
            with_files: false,
        });
        self.focus_dialog(cx);
        cx.notify();
    }

    fn dismiss_delete_prompt(&mut self, cx: &mut Context<Self>) {
        self.delete_prompt = None;
        cx.notify();
    }

    fn toggle_delete_files(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = &mut self.delete_prompt {
            prompt.with_files = !prompt.with_files;
            cx.notify();
        }
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(DeletePrompt { cards, with_files }) = self.delete_prompt.take() {
            self.delete_now(cards, with_files, cx);
            cx.notify();
        }
    }

    fn delete_now(&mut self, cards: Vec<DeleteCard>, with_files: bool, cx: &mut Context<Self>) {
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let deleted = cx
                .background_spawn(async move {
                    delete_stored(&store, cards, with_files, platform::recycle_file)
                })
                .await;
            this.update(cx, |this, cx| this.finish_delete(deleted, cx))
                .ok();
        });
    }

    fn finish_delete(&mut self, deleted: Deleted, cx: &mut Context<Self>) {
        let count = deleted.cards.len();
        self.gallery
            .update(cx, |gallery, cx| gallery.remove_cards(&deleted.cards, cx));
        if deleted.failed > 0 {
            let total = count + deleted.failed;
            self.report_failure(format!("Couldn't delete {} of {total}", deleted.failed), cx);
        } else if deleted.files_kept > 0 {
            let message = format!(
                "Deleted, but {} couldn't be removed. It may be open elsewhere.",
                plural(deleted.files_kept, "file")
            );
            self.report_failure(message, cx);
        } else {
            self.show_toast(Toast::new(deleted_message(count, deleted.files_stay)), cx);
        }
    }

    /// Cancel comes first, so it holds focus when the dialog opens and a stray Enter deletes
    /// nothing. The tick box only shows when there is a file to delete.
    pub(super) fn delete_dialog(&self, prompt: &DeletePrompt, cx: &Context<Self>) -> Dialog {
        let saved = prompt.cards.iter().filter(|card| card.saved).count();
        let unsaved = prompt.cards.len() - saved;
        let dialog = Dialog::new(
            "delete-dialog",
            delete_title(prompt.cards.len()),
            delete_body(unsaved, saved),
            &self.dialog_focus,
            act(cx, |this, cx| this.dismiss_delete_prompt(cx)),
        )
        .action("Cancel", act(cx, |this, cx| this.dismiss_delete_prompt(cx)))
        .primary_action("Delete", act(cx, |this, cx| this.confirm_delete(cx)));
        if saved > 0 {
            dialog.tick(
                "Also delete files",
                prompt.with_files,
                act(cx, |this, cx| this.toggle_delete_files(cx)),
            )
        } else {
            dialog
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use toru_core::{
        export::save_capture, naming::DEFAULT_PATTERN, settings::SaveFormat, store::SessionId,
    };

    use super::super::captures::tests::{TestDir, add};
    use super::*;

    /// How the tests remove a file: for good, so they leave nothing in the Recycle Bin.
    fn remove(path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    /// A store with one unsaved capture in a session.
    fn stored(dir: &TestDir) -> (Store, SessionId, CaptureId) {
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, DEFAULT_PATTERN).unwrap().files.id;
        (store, session, id)
    }

    #[test]
    fn the_question_says_what_goes_and_what_stays() {
        assert_eq!(
            delete_body(1, 0),
            "1 unsaved capture can't be brought back."
        );
        assert_eq!(
            delete_body(3, 0),
            "3 unsaved captures can't be brought back."
        );
        assert!(delete_body(0, 1).starts_with("1 saved capture leaves the gallery, and its file"));
        assert!(delete_body(0, 2).starts_with("2 saved captures leave the gallery, and their"));
        let both = delete_body(2, 1);
        assert!(both.starts_with("2 unsaved captures can't be brought back. 1 saved capture"));
        assert_eq!(delete_title(1), "Delete 1 capture?");
        assert_eq!(delete_title(4), "Delete 4 captures?");
    }

    #[test]
    fn the_toast_says_when_files_stayed() {
        assert_eq!(deleted_message(1, false), "Deleted 1 capture");
        assert_eq!(
            deleted_message(2, true),
            "Removed 2 captures. Saved files stay in your folder."
        );
    }

    fn saved_card(dir: &TestDir, store: &Store, session: SessionId) -> (DeleteCard, String) {
        let id = add(store, session, "saved-{yyyy}").unwrap().files.id;
        let file = save_capture(store, id, &dir.0.join("Screenshots"), SaveFormat::Png)
            .unwrap()
            .unwrap()
            .path;
        let card = DeleteCard {
            card: 2,
            capture: id,
            saved: true,
        };
        (card, file.display().to_string())
    }

    #[test]
    fn without_the_box_a_saved_file_stays() {
        let dir = TestDir::new("keeps-file");
        let (store, session, _) = stored(&dir);
        let (card, file) = saved_card(&dir, &store, session);

        let deleted = delete_stored(&store, vec![card], false, remove);

        assert_eq!(deleted.cards, [2]);
        assert_eq!(deleted.failed, 0);
        assert!(deleted.files_stay);
        assert_eq!(store.session_captures(session).unwrap().len(), 1);
        assert!(fs::metadata(file).is_ok());
    }

    #[test]
    fn with_the_box_the_file_goes_too() {
        let dir = TestDir::new("deletes-file");
        let (store, session, _) = stored(&dir);
        let (card, file) = saved_card(&dir, &store, session);

        let deleted = delete_stored(&store, vec![card], true, remove);

        assert_eq!(deleted.cards, [2]);
        assert_eq!(deleted.files_kept, 0);
        assert!(!deleted.files_stay);
        assert!(fs::metadata(file).is_err());
    }

    #[test]
    fn a_file_saved_after_the_question_opened_stays_even_with_the_box() {
        let dir = TestDir::new("saved-late");
        let (store, session, _) = stored(&dir);
        let (mut card, file) = saved_card(&dir, &store, session);
        card.saved = false;

        let deleted = delete_stored(&store, vec![card], true, remove);

        assert_eq!(deleted.cards, [2]);
        assert_eq!(deleted.files_kept, 0);
        assert!(deleted.files_stay);
        assert_eq!(store.session_captures(session).unwrap().len(), 1);
        assert!(fs::metadata(file).is_ok());
    }

    #[test]
    fn a_file_that_stays_is_reported_but_its_card_still_goes() {
        let dir = TestDir::new("file-stuck");
        let (store, session, _) = stored(&dir);
        let (card, file) = saved_card(&dir, &store, session);
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();

        let deleted = delete_stored(&store, vec![card], true, remove);

        assert_eq!(deleted.cards, [2]);
        assert_eq!(deleted.files_kept, 1);
    }

    #[test]
    fn an_unsaved_capture_goes_without_touching_anything_else() {
        let dir = TestDir::new("unsaved");
        let (store, session, id) = stored(&dir);
        let (_saved, file) = saved_card(&dir, &store, session);
        let unsaved = DeleteCard {
            card: 1,
            capture: id,
            saved: false,
        };

        let deleted = delete_stored(&store, vec![unsaved], true, remove);

        assert_eq!(deleted.cards, [1]);
        assert!(!deleted.files_stay);
        assert_eq!(store.session_captures(session).unwrap().len(), 1);
        assert!(fs::metadata(file).is_ok());
        assert!(!store.image_path(id).exists());
    }
}
