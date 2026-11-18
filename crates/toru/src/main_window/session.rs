use gpui::{Context, prelude::*};
use toru_core::{store::SessionId, time::LocalDateTime};

use super::MainWindow;

impl MainWindow {
    /// Ends the session and starts another if the date is no longer the session's. It goes on the
    /// queue so that captures already on their way land in the session they were taken in.
    pub(super) fn check_day(&mut self, now: LocalDateTime, cx: &mut Context<Self>) {
        if now.date == self.day {
            return;
        }
        let previous = std::mem::replace(&mut self.day, now.date);
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let session = cx
                .background_spawn(async move { store.new_session() })
                .await;
            this.update(cx, |this, cx| match session {
                Ok(session) => this.start_session(session, now, cx),
                // The next check tries again.
                Err(_) => this.day = previous,
            })
            .ok();
        });
    }

    pub(super) fn start_session(
        &mut self,
        session: SessionId,
        now: LocalDateTime,
        cx: &mut Context<Self>,
    ) {
        self.session = session;
        self.session_started = now.clock().into();
        let gallery = self.gallery.clone();
        self.window
            .update(cx, |_, window, cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.end_session(session, now.date, window, cx)
                })
            })
            .ok();
        // A new day may put more captures past the retention period.
        self.clean_up_history(cx);
        cx.notify();
    }
}
