use gpui::{Context, Entity, Focusable, SharedString, Window};
use gpui_kit::component::input::InputState;
use toru_core::store::CaptureId;

use super::Gallery;

impl Gallery {
    /// The store took the name. Returns whether the capture is saved, as its file keeps its name.
    pub fn accept_name(
        &mut self,
        capture: CaptureId,
        name: String,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(card) = self.stored_card(capture) else {
            return false;
        };
        card.capture.name = name;
        card.name_error = None;
        let saved = card.capture.is_saved();
        cx.notify();
        saved
    }

    /// Says on the card why the store turned the name down. If the user has already left the
    /// field there is nothing more to type, so the name goes back at once.
    pub fn refuse_name(
        &mut self,
        capture: CaptureId,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        card.name_error = Some(message.into());
        let focused = card.name.read(cx).focus_handle(cx).is_focused(window);
        if !focused {
            self.restore_refused_name(capture, window, cx);
        }
        cx.notify();
    }

    pub(super) fn restore_refused_name(
        &mut self,
        capture: CaptureId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        if card.name_error.take().is_some() {
            let name = card.capture.name.clone();
            card.name
                .update(cx, |field, cx| field.set_value(name, window, cx));
            cx.notify();
        }
    }

    /// Why the card with the name field `name` has its name refused, if it does.
    pub fn name_error_of(&self, name: &Entity<InputState>) -> Option<SharedString> {
        self.groups
            .iter()
            .flat_map(|group| &group.cards)
            .find(|card| card.name == *name)
            .and_then(|card| card.name_error.clone())
    }
}
