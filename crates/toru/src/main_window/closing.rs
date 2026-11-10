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
//! the moment GPUI gives quit handlers.

use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{BackgroundExecutor, Context, SHUTDOWN_TIMEOUT};
use toru_core::settings::OnClose;

use super::{MainWindow, act};
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
const NO_TRAY: &str = "Couldn't add the tray icon. Closing the window quits Toru.";
const NO_TRAY_TOAST_FOR: Duration = Duration::from_secs(8);

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
        } else {
            self.request_quit(cx);
        }
        false
    }

    /// The tray icon couldn't be added, so closing quits (`on_close_requested`), and the user is
    /// told so rather than left to find out.
    pub fn tray_unavailable(&mut self, cx: &mut Context<Self>) {
        self.has_tray = false;
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
            platform::show_main_window();
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
                platform::show_main_window();
                self.focus_dialog(cx);
                cx.notify();
            }
        }
    }

    /// Focuses a dialog that has just opened, on its first action. Deferred, because a close
    /// request or a press arrives inside the window's own update, which the window can't be
    /// updated again from.
    pub(super) fn focus_dialog(&self, cx: &mut Context<Self>) {
        let (window, focus) = (self.window, self.dialog_focus.clone());
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| focus.focus(window, cx))
                .ok();
        });
    }

    /// Deletes the session's unsaved captures and quits. Done here and now rather than on the
    /// queue, since the app is leaving; the files are few. If the delete fails, the captures
    /// stay in history, where the retention setting reaches them.
    fn discard_and_quit(&mut self, cx: &mut Context<Self>) {
        self.store.discard_unsaved(self.session).ok();
        cx.quit();
    }

    /// Runs as the app quits, for whatever reason: its own Quit, or Windows shutting down or
    /// logging off, when GPUI runs the quit handlers and exits without asking the window. The
    /// setting is applied as `discards_at_shutdown` says. After Toru's own Quit the delete has
    /// been done already and finds nothing. The main thread's tasks don't run while GPUI waits
    /// on this, so the wait only helps a store write already on a background thread; the first
    /// delete makes sure the captures already there go even if the time runs out.
    pub fn on_app_quit(&mut self, cx: &mut Context<Self>) -> impl Future<Output = ()> + use<> {
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
    use super::*;

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
