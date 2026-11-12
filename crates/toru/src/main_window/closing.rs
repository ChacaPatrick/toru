//! Closing the window and quitting (DESIGN.md §2 and §3 with CLAUDE.md's overrides). With "Keep
//! running in the tray" on, closing the window hides it and the app lives on in the tray; off, or
//! with no tray icon to come back from, it quits. Quitting, from there or from the tray's Quit,
//! puts the On close setting to this session's unsaved captures: Keep leaves them in history
//! under the retention setting, Discard deletes them, and Ask shows a dialog, unless there is
//! nothing unsaved. Saved captures always stay in history, whatever the setting. Edits in an
//! open editor first go to the card, as Back would put them, so the setting treats that capture
//! like any other: nothing the user did is lost without being asked about.
//!
//! A capture under way or still being stored is waited for first, so a shot just taken is counted
//! and, with Discard, is in the store before the delete rather than landing after it. When Windows
//! shuts down or logs off there is nobody to ask, so Ask counts as Keep and only Discard acts, in
//! the moment GPUI gives quit handlers. The queue doesn't run then, so captures and Back writes
//! still waiting on it, and then an open editor's edits, are written there and then, before the
//! setting is applied, as a quit from Toru would order them.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{BackgroundExecutor, Context, FocusHandle, SHUTDOWN_TIMEOUT, Window};
use toru_core::{
    settings::OnClose,
    store::{CaptureId, Result as StoreResult, SessionId, Store},
};

use super::{MainWindow, act};
use crate::editor::document::{Flattened, to_rgba};
use crate::platform;
use crate::ui::{Dialog, Toast};

/// How long quitting waits for a capture under way and the queue. A full-screen shot is stored
/// well within this; past it the app quits anyway.
const SETTLE_LIMIT: Duration = Duration::from_secs(3);
/// How often the wait looks again.
const SETTLE_POLL: Duration = Duration::from_millis(20);
/// How long the shutdown handler waits: under GPUI's limit, with time left for the delete.
const SHUTDOWN_SETTLE_LIMIT: Duration =
    Duration::from_millis(SHUTDOWN_TIMEOUT.as_millis() as u64 * 3 / 4);
/// How long after hiding in the tray the app hands its unused memory back: once the hide and
/// the last frame are done with.
const TRIM_AFTER: Duration = Duration::from_secs(2);
const NO_TRAY: &str = "Couldn't add the tray icon. Closing the window quits Toru.";
const NO_TRAY_TOAST_FOR: Duration = Duration::from_secs(8);
/// What Windows shows for Toru if it ends the session while the editor's edits are being written.
const WRITING_EDITS: &str = "Keeping your edits";
/// The same, while captures and edits still waiting on the queue are being written.
const WRITING_QUEUED: &str = "Keeping your captures";

/// What a queued write needs, held where the quit handler can reach it until the job starts
/// (`hold_until_started`): at Windows' shutdown GPUI exits without running the queue, and the
/// handler writes on the main thread whatever no job has taken yet.
pub(super) trait WriteAtShutdown {
    fn write(self, store: &Store, session: SessionId);
}

/// A queued write's data, until its job or the quit handler takes it.
pub(super) type Held<T> = Rc<RefCell<Option<T>>>;

/// A `Held` write of any kind, as the quit handler sees it.
pub(super) trait Unstarted {
    fn is_unstarted(&self) -> bool;
    fn write_now(&self, store: &Store, session: SessionId);
}

impl<T: WriteAtShutdown> Unstarted for RefCell<Option<T>> {
    fn is_unstarted(&self) -> bool {
        self.borrow().is_some()
    }

    fn write_now(&self, store: &Store, session: SessionId) {
        if let Some(write) = self.take() {
            write.write(store, session);
        }
    }
}

/// What quitting does with this session's unsaved captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Quit,
    DiscardThenQuit,
    Ask,
}

/// The On close setting applied to `unsaved` captures. With none there is nothing to ask about
/// or to delete, so the app just quits.
pub fn decide(on_close: OnClose, unsaved: usize) -> Exit {
    match on_close {
        _ if unsaved == 0 => Exit::Quit,
        OnClose::Keep => Exit::Quit,
        OnClose::Discard => Exit::DiscardThenQuit,
        OnClose::Ask => Exit::Ask,
    }
}

/// Whether the setting deletes unsaved captures when Windows ends the session: only Discard
/// does, since Ask can't be answered then and so counts as Keep.
pub fn discards_at_shutdown(on_close: OnClose) -> bool {
    on_close == OnClose::Discard
}

/// The dialog's explanation, for `unsaved` captures.
pub fn quit_prompt(unsaved: usize) -> String {
    if unsaved == 1 {
        "1 capture from this session isn't saved to your folder. Keep it in history, or delete \
         it?"
        .to_string()
    } else {
        format!(
            "{unsaved} captures from this session aren't saved to your folder. Keep them in \
             history, or delete them?"
        )
    }
}

/// Writes the edited picture to its capture, for Back and for quitting, on this thread. `None`
/// for the original tells the store the edits were all taken off again.
pub(super) fn store_flattened(
    store: &Store,
    capture: CaptureId,
    flattened: &Flattened,
    thumbnail_width: u32,
) -> StoreResult<u64> {
    let original = flattened.is_edited.then(|| to_rgba(&flattened.original));
    store.store_edit(
        capture,
        &to_rgba(&flattened.picture),
        original.as_ref(),
        flattened.redacted,
        thumbnail_width,
    )
}

/// Resolves once `pending` captures and store jobs have all finished, or after `limit`.
async fn settle(pending: &Cell<usize>, limit: Duration, executor: &BackgroundExecutor) {
    let deadline = Instant::now() + limit;
    while pending.get() > 0 && Instant::now() < deadline {
        executor.timer(SETTLE_POLL).await;
    }
}

impl MainWindow {
    /// The window's close request, from the caption's X, Alt+F4 or the title bar's Close. The
    /// window is never closed by GPUI itself: it is hidden, or the app quits. Without a tray icon
    /// there would be no way back from hidden, so then it quits whatever the setting.
    pub(super) fn on_close_requested(&mut self, cx: &mut Context<Self>) -> bool {
        if self.settings.keep_in_tray && self.has_tray {
            // A hidden window records nothing: the next key press is some other app's.
            self.stop_recording(cx);
            platform::hide_main_window();
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(TRIM_AFTER).await;
                // Not if the window is back already.
                if platform::main_window_hidden() {
                    platform::trim_working_set();
                }
            })
            .detach();
        } else {
            self.request_quit(cx);
        }
        false
    }

    /// Shows the window from the tray, restored if minimized, and brings it forward. A window
    /// opened hidden (`--tray`) has never been placed, so the first time GPUI places it where
    /// it would have opened, and the foreground follows once it has.
    pub fn show_window(&mut self, cx: &mut Context<Self>) {
        if self.started_hidden
            && self
                .window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            self.started_hidden = false;
            // GPUI places the window in a task of its own, which runs before this one.
            cx.spawn(async |_, _| platform::show_main_window()).detach();
            return;
        }
        platform::show_main_window();
    }

    /// The tray icon couldn't be added, so closing quits (`on_close_requested`), and the user is
    /// told so rather than left to find out.
    pub fn tray_unavailable(&mut self, cx: &mut Context<Self>) {
        self.has_tray = false;
        // Started in the tray, the window would have nothing to come back from.
        self.show_window(cx);
        self.toast
            .show(Toast::failed(NO_TRAY), NO_TRAY_TOAST_FOR, cx, |this| {
                &mut this.toast
            });
    }

    /// Quits by the On close setting, once a capture under way and the queue have had their
    /// chance to finish. A second request while that wait or the dialog is up changes nothing,
    /// beyond bringing the window back for the dialog.
    pub fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.quit_prompt.is_some() {
            self.show_window(cx);
            return;
        }
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.keep_open_edits(cx);
        let pending = self.pending.clone();
        cx.spawn(async move |this, cx| {
            settle(&pending, SETTLE_LIMIT, cx.background_executor()).await;
            this.update(cx, |this, cx| {
                this.quitting = false;
                this.finish_quit(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Ask brings the window back from the tray if need be, since the question is asked in it.
    fn finish_quit(&mut self, cx: &mut Context<Self>) {
        let unsaved = self.gallery.read(cx).unsaved_in_session();
        match decide(self.settings.on_close, unsaved) {
            Exit::Quit => cx.quit(),
            Exit::DiscardThenQuit => self.discard_and_quit(cx),
            Exit::Ask => {
                // One dialog at a time: the question about quitting replaces this one.
                self.delete_prompt = None;
                self.clear_prompt = false;
                self.quit_prompt = Some(unsaved);
                self.show_window(cx);
                self.focus_dialog(cx);
                cx.notify();
            }
        }
    }

    /// Focuses a dialog that has just opened, on its first action. Deferred, because a close
    /// request or a press arrives inside the window's own update, which the window can't be
    /// updated again from.
    pub(super) fn focus_dialog(&self, cx: &mut Context<Self>) {
        self.focus_in_dialog(self.dialog_focus.clone(), cx);
    }

    /// Focuses `focus`, a control of a dialog that has just opened, and notes what had focus
    /// until then, the control the dialog was opened from, so focus can go back there when the
    /// dialog closes (`return_focus`). A dialog opened over another keeps the first one's note.
    pub(super) fn focus_in_dialog(&self, focus: FocusHandle, cx: &mut Context<Self>) {
        let window = self.window;
        let this = cx.weak_entity();
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    let before = window.focused(cx);
                    this.update(cx, |this, _| this.note_opener(before)).ok();
                    focus.focus(window, cx)
                })
                .ok();
        });
    }

    /// Keeps `focused` as what to go back to when the dialogs close, unless one is noted.
    pub(super) fn note_opener(&mut self, focused: Option<FocusHandle>) {
        if self.opener.is_none() {
            self.opener = focused;
        }
    }

    /// Once no dialog is open, gives focus back to the control the dialog was opened from. If
    /// that control is gone, as a deleted card's menu is, the frame drawn next finds nothing
    /// focused and `on_focus_lost` picks a place instead. Called as the window renders.
    pub(super) fn return_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dialog_open()
            && let Some(opener) = self.opener.take()
        {
            opener.focus(window, cx);
        }
    }

    /// Deletes the session's unsaved captures and quits. Done here and now rather than on the
    /// queue, since the app is leaving; the files are few. If the delete fails, the captures
    /// stay in history, where the retention setting reaches them. Captures the wait for the
    /// queue gave up on are stored first, as the quit handler would after the delete, so the
    /// delete reaches them; one auto-saved stays, as saved captures do.
    fn discard_and_quit(&mut self, cx: &mut Context<Self>) {
        self.write_unstarted_now();
        self.store.discard_unsaved(self.session).ok();
        cx.quit();
    }

    /// Runs as the app quits, for whatever reason: its own Quit, or Windows shutting down or
    /// logging off, when GPUI runs the quit handlers and exits without asking the window.
    /// Captures and Back writes still waiting on the queue, then an open editor's edits, are
    /// written first, before this returns, since GPUI's time limit only holds the future; after
    /// Toru's own Quit the queue has run and there is nothing left. The setting is then applied as
    /// `discards_at_shutdown` says. After Toru's own Quit the delete has been done already and
    /// finds nothing. The main thread's tasks don't run while GPUI waits on this, so the wait
    /// only helps a store write already on a background thread; the first delete makes sure the
    /// captures already there go even if the time runs out.
    pub fn on_app_quit(&mut self, cx: &mut Context<Self>) -> impl Future<Output = ()> + use<> {
        self.write_unstarted_now();
        self.write_open_edits_now(cx);
        let discard = discards_at_shutdown(self.settings.on_close);
        let (store, session, pending) = (self.store.clone(), self.session, self.pending.clone());
        let executor = cx.background_executor().clone();
        async move {
            if !discard {
                return;
            }
            store.discard_unsaved(session).ok();
            if pending.get() > 0 {
                settle(&pending, SHUTDOWN_SETTLE_LIMIT, &executor).await;
                store.discard_unsaved(session).ok();
            }
        }
    }

    /// Keeps `write` where the quit handler can reach it until the job queued with the returned
    /// handle takes it as it starts. A job that finds it gone returns: it was written already.
    pub(super) fn hold_until_started<T: WriteAtShutdown + 'static>(&mut self, write: T) -> Held<T> {
        let held = Rc::new(RefCell::new(Some(write)));
        self.unstarted.retain(|held| held.is_unstarted());
        self.unstarted.push(held.clone());
        held
    }

    /// Writes, in queue order and before this returns, whatever queued job hasn't started. A job
    /// already running on a background thread is left to finish if it can.
    fn write_unstarted_now(&mut self) {
        let unstarted = std::mem::take(&mut self.unstarted);
        if !unstarted.iter().any(|held| held.is_unstarted()) {
            return;
        }
        platform::while_holding_up_shutdown(WRITING_QUEUED, || {
            for held in unstarted {
                held.write_now(&self.store, self.session);
            }
        });
    }

    /// The open editor's edits not yet given to the card, written before this returns: at
    /// Windows' shutdown GPUI exits once the quit handlers return, without running the queue
    /// `keep_open_edits` uses. After Toru's own Quit that has taken them already, and this finds
    /// nothing. A failure goes unreported, as there is no window left to say so in.
    fn write_open_edits_now(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let (capture, flattened) = editor.update(cx, |editor, cx| {
            (editor.stored(), editor.take_unwritten(cx))
        });
        let Some(flattened) = flattened else {
            return;
        };
        let thumbnail_width = self
            .gallery
            .read(cx)
            .thumbnail_width((flattened.picture.width(), flattened.picture.height()));
        platform::while_holding_up_shutdown(WRITING_EDITS, || {
            store_flattened(&self.store, capture, &flattened, thumbnail_width)
        })
        .ok();
    }

    fn dismiss_quit_prompt(&mut self, cx: &mut Context<Self>) {
        self.quit_prompt = None;
        cx.notify();
    }

    /// The Ask dialog. Keep in history is the primary action and holds focus when it opens;
    /// Cancel and Esc leave the app running.
    pub(super) fn quit_dialog(&self, unsaved: usize, cx: &Context<Self>) -> Dialog {
        Dialog::new(
            "quit-dialog",
            "Quit Toru?",
            quit_prompt(unsaved),
            &self.dialog_focus,
            act(cx, |this, cx| this.dismiss_quit_prompt(cx)),
        )
        .primary_action(
            "Keep in history",
            act(cx, |this, cx| {
                this.quit_prompt = None;
                cx.quit();
            }),
        )
        .action("Discard", act(cx, |this, cx| this.discard_and_quit(cx)))
        .action("Cancel", act(cx, |this, cx| this.dismiss_quit_prompt(cx)))
    }
}

/// A count of work under way, shared with the tasks that finish it.
pub(super) fn pending_counter() -> Rc<Cell<usize>> {
    Rc::new(Cell::new(0))
}

/// One more piece of work under way; the returned guard takes it off again when dropped.
pub(super) fn begin(pending: &Rc<Cell<usize>>) -> Pending {
    pending.set(pending.get() + 1);
    Pending(pending.clone())
}

pub(super) struct Pending(Rc<Cell<usize>>);

impl Drop for Pending {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tiny_skia::{Color, Pixmap};
    use toru_core::{
        capture::{Capture, CaptureMode},
        time::{LocalDate, LocalDateTime},
    };
    use xcap::image::{self, RgbaImage};

    use super::super::captures::tests::TestDir;
    use super::*;
    use crate::editor::document::Document;

    fn one_capture(store: &Store) -> CaptureId {
        let session = store.new_session().unwrap();
        let capture = Capture {
            name: "shot".to_string(),
            mode: CaptureMode::Region,
            width: 4,
            height: 2,
            taken_at: LocalDateTime {
                date: LocalDate {
                    year: 2026,
                    month: 11,
                    day: 10,
                },
                hour: 9,
                minute: 0,
            },
            saved_path: None,
        };
        store
            .add_capture(session, &RgbaImage::new(4, 2), &capture, 2)
            .unwrap()
    }

    /// What the editor gives the card for a 4 × 2 capture: red all over if `edited`, the
    /// capture as it was otherwise.
    fn flattened(edited: bool) -> Flattened {
        let mut flattened = Document::new(Pixmap::new(4, 2).unwrap()).flatten();
        if edited {
            let mut picture = Pixmap::new(4, 2).unwrap();
            picture.fill(Color::from_rgba8(255, 0, 0, 255));
            flattened.picture = Arc::new(picture);
            flattened.is_edited = true;
        }
        flattened
    }

    #[test]
    fn edits_written_at_shutdown_reach_the_store_before_it_returns() {
        let dir = TestDir::new("shutdown-edits");
        let store = dir.store();
        let capture = one_capture(&store);

        let bytes = store_flattened(&store, capture, &flattened(true), 2).unwrap();
        let stored = image::open(store.image_path(capture)).unwrap().to_rgba8();
        assert_eq!(stored.get_pixel(3, 1).0, [255, 0, 0, 255]);
        assert_eq!(fs_len(&store.image_path(capture)), bytes);
        assert!(store.original_path(capture).is_file(), "kept for Reset");

        // Edits all taken off again: the capture as taken, and nothing kept for Reset.
        store_flattened(&store, capture, &flattened(false), 2).unwrap();
        let stored = image::open(store.image_path(capture)).unwrap().to_rgba8();
        assert_eq!(stored.get_pixel(3, 1).0, [0, 0, 0, 0]);
        assert!(!store.original_path(capture).exists());
    }

    fn fs_len(path: &std::path::Path) -> u64 {
        std::fs::metadata(path).unwrap().len()
    }

    /// Counts how often it is written.
    struct Counted(Rc<Cell<usize>>);

    impl WriteAtShutdown for Counted {
        fn write(self, _store: &Store, _session: SessionId) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn a_held_write_is_written_once_by_whichever_takes_it_first() {
        let dir = TestDir::new("held-write");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let written = Rc::new(Cell::new(0));

        // Never started: the quit handler writes it, and a job starting later finds nothing.
        let held = RefCell::new(Some(Counted(written.clone())));
        assert!(held.is_unstarted());
        held.write_now(&store, session);
        assert_eq!(written.get(), 1);
        assert!(!held.is_unstarted());
        held.write_now(&store, session);
        assert!(held.take().is_none());
        assert_eq!(written.get(), 1);

        // Started: the job has it, and the quit handler leaves it alone.
        let held = RefCell::new(Some(Counted(written.clone())));
        let _taken = held.take();
        held.write_now(&store, session);
        assert_eq!(written.get(), 1);
    }

    #[test]
    fn the_setting_decides_what_quitting_does() {
        assert_eq!(decide(OnClose::Ask, 2), Exit::Ask);
        assert_eq!(decide(OnClose::Keep, 2), Exit::Quit);
        assert_eq!(decide(OnClose::Discard, 2), Exit::DiscardThenQuit);
    }

    #[test]
    fn nothing_unsaved_means_nothing_to_ask_or_delete() {
        for on_close in [OnClose::Ask, OnClose::Keep, OnClose::Discard] {
            assert_eq!(decide(on_close, 0), Exit::Quit);
        }
    }

    #[test]
    fn at_shutdown_only_discard_deletes_and_ask_counts_as_keep() {
        assert!(discards_at_shutdown(OnClose::Discard));
        assert!(!discards_at_shutdown(OnClose::Keep));
        assert!(!discards_at_shutdown(OnClose::Ask));
    }

    #[test]
    fn the_shutdown_wait_leaves_time_for_the_delete() {
        assert!(SHUTDOWN_SETTLE_LIMIT < SHUTDOWN_TIMEOUT);
        assert!(SHUTDOWN_SETTLE_LIMIT >= SETTLE_POLL * 2);
    }

    #[test]
    fn pending_work_is_counted_while_its_guard_lives() {
        let pending = pending_counter();
        let first = begin(&pending);
        let second = begin(&pending);
        assert_eq!(pending.get(), 2);
        drop(first);
        assert_eq!(pending.get(), 1);
        drop(second);
        assert_eq!(pending.get(), 0);
    }

    #[test]
    fn the_prompt_counts_the_captures() {
        assert!(quit_prompt(1).starts_with("1 capture from this session isn't saved"));
        assert!(quit_prompt(1).ends_with("Keep it in history, or delete it?"));
        assert!(quit_prompt(3).starts_with("3 captures from this session aren't saved"));
        assert!(quit_prompt(3).ends_with("Keep them in history, or delete them?"));
    }
}
