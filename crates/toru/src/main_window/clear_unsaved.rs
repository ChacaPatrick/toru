use gpui::{Context, prelude::*};

use super::{MainWindow, act, gallery::plural};
use crate::ui::{Dialog, Toast};

const CLEAR_FAILED: &str = "Couldn't clear the unsaved captures";

fn clear_title(count: usize) -> String {
    format!("Clear {}?", plural(count, "unsaved capture"))
}

fn clear_body(count: usize) -> String {
    if count == 1 {
        "It will be deleted. It isn't saved to your folder, so it can't be brought back."
            .to_string()
    } else {
        format!(
            "All {count} will be deleted. They aren't saved to your folder, so they can't be \
             brought back."
        )
    }
}

impl MainWindow {
    pub(super) fn ask_clear_unsaved(&mut self, cx: &mut Context<Self>) {
        if self.gallery.read(cx).unsaved_in_session() == 0 {
            return;
        }
        self.clear_prompt = true;
        self.focus_dialog(cx);
        cx.notify();
    }

    fn dismiss_clear_prompt(&mut self, cx: &mut Context<Self>) {
        self.clear_prompt = false;
        cx.notify();
    }

    fn confirm_clear(&mut self, cx: &mut Context<Self>) {
        self.clear_prompt = false;
        self.clear_unsaved(cx);
        cx.notify();
    }

    /// Cancel first, so it holds focus when the dialog opens and a stray Enter clears nothing.
    /// The count is read as it is drawn, so a capture landing while the question is up is counted.
    pub(super) fn clear_dialog(&self, cx: &Context<Self>) -> Dialog {
        let unsaved = self.gallery.read(cx).unsaved_in_session();
        Dialog::new(
            "clear-dialog",
            clear_title(unsaved),
            clear_body(unsaved),
            &self.dialog_focus,
            act(cx, |this, cx| this.dismiss_clear_prompt(cx)),
        )
        .action("Cancel", act(cx, |this, cx| this.dismiss_clear_prompt(cx)))
        .primary_action("Clear", act(cx, |this, cx| this.confirm_clear(cx)))
    }

    /// On the queue, so a capture still being stored is cleared with the rest, and a Save
    /// already asked for is finished first.
    fn clear_unsaved(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let Ok(session) = this.read_with(cx, |this, _| this.session) else {
                return;
            };
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
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn the_clear_question_says_how_many_go() {
        assert_eq!(clear_title(1), "Clear 1 unsaved capture?");
        assert_eq!(clear_title(3), "Clear 3 unsaved captures?");
        assert!(clear_body(1).starts_with("It will be deleted."));
        assert!(clear_body(3).starts_with("All 3 will be deleted."));
        assert!(clear_body(3).ends_with("they can't be brought back."));
    }
}
