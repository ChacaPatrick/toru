use gpui::{Context, Modifiers, Window};
use toru_core::store::CaptureId;

use super::{Card, Gallery};
use crate::main_window::{
    delete::DeleteCard,
    selection::{Counts, enters_select_mode},
};

impl Gallery {
    pub fn in_select_mode(&self) -> bool {
        self.select_mode
    }

    /// How many cards are selected and how many of those are saved, for the bulk bar.
    pub fn selection_counts(&self) -> Counts {
        let saved = self
            .groups
            .iter()
            .flat_map(|group| &group.cards)
            .filter(|card| self.selection.contains(card.id) && card.capture.is_saved())
            .count();
        Counts {
            selected: self.selection.len(),
            saved,
        }
    }

    /// The selected cards in reading order, so what bulk actions produce reads like the gallery.
    fn selected_cards(&self) -> Vec<&Card> {
        self.listed_ids()
            .into_iter()
            .filter(|&id| self.selection.contains(id))
            .filter_map(|id| self.card(id))
            .collect()
    }

    /// The selected captures: what Copy as files and Copy paths copy.
    pub fn selected_captures(&self) -> Vec<CaptureId> {
        self.selected_cards()
            .iter()
            .map(|card| card.files.id)
            .collect()
    }

    /// The selected captures not saved yet: what the bulk bar's Save writes.
    pub fn selected_unsaved(&self) -> Vec<CaptureId> {
        self.selected_cards()
            .iter()
            .filter(|card| !card.capture.is_saved())
            .map(|card| card.files.id)
            .collect()
    }

    fn delete_card_for(card: &Card) -> DeleteCard {
        DeleteCard {
            card: card.id,
            capture: card.files.id,
            saved: card.capture.is_saved(),
        }
    }

    pub fn selected_for_delete(&self) -> Vec<DeleteCard> {
        self.selected_cards()
            .into_iter()
            .map(Self::delete_card_for)
            .collect()
    }

    pub fn card_for_delete(&self, id: usize) -> Option<DeleteCard> {
        self.card(id).map(Self::delete_card_for)
    }

    pub(super) fn enter_select_mode(&mut self, cx: &mut Context<Self>) {
        self.select_mode = true;
        cx.notify();
    }

    /// Esc or Done: out of select mode with nothing selected.
    pub fn leave_select_mode(&mut self, cx: &mut Context<Self>) {
        self.select_mode = false;
        self.selection.clear();
        cx.notify();
    }

    /// Ctrl+A or Select all: every listed card, entering select mode if need be.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.select_mode = true;
        self.selection.set_all(&self.listed_ids());
        cx.notify();
    }

    /// The bulk bar's ✕: the selection goes, select mode stays. Focus moves into the gallery,
    /// where Tab reaches the current card, since the bar it was on goes with the selection.
    pub fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.focus_handle().focus(window, cx);
        cx.notify();
    }

    /// A press on a card's checkbox or thumbnail, or Space on the card, with the modifiers held.
    /// Outside select mode only a Ctrl-press, without Alt, counts, and enters it; a Shift-press
    /// ranges from the last card pressed on its own.
    pub(in crate::main_window) fn press_card(
        &mut self,
        id: usize,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if !self.select_mode {
            if !enters_select_mode(&modifiers) {
                return;
            }
            self.select_mode = true;
        }
        if modifiers.shift {
            self.selection.extend_to(id, &self.listed_ids());
        } else {
            self.selection.toggle(id);
        }
        self.current = Some(id);
        cx.notify();
    }

    /// The listed cards' ids in reading order: what a range runs over and Select all selects.
    pub(super) fn listed_ids(&self) -> Vec<usize> {
        self.shown().into_iter().map(|(_, id)| id).collect()
    }
}
