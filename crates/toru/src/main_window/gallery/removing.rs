use gpui::Context;
use toru_core::store::CaptureId;

use super::{Card, Gallery, GroupKind};

impl Gallery {
    /// The session's unsaved captures, for Save all.
    pub fn unsaved_captures(&self) -> Vec<CaptureId> {
        self.groups[0]
            .cards
            .iter()
            .filter(|card| !card.capture.is_saved())
            .map(|card| card.files.id)
            .collect()
    }

    /// Takes the session's unsaved cards away and returns how many there were. A focused card
    /// that goes with them stops being rendered, which the main window answers by refocusing.
    pub fn remove_unsaved(&mut self, cx: &mut Context<Self>) -> usize {
        let going: Vec<usize> = self.groups[0]
            .cards
            .iter()
            .filter(|card| !card.capture.is_saved())
            .map(|card| card.id)
            .collect();
        self.move_current_off(|card| going.contains(&card.id));
        let (kept, cleared): (Vec<Card>, Vec<Card>) = std::mem::take(&mut self.groups[0].cards)
            .into_iter()
            .partition(|card| card.capture.is_saved());
        self.groups[0].cards = kept;
        let removed = cleared.len();
        self.cleared.extend(cleared);
        self.rebuild_rows(cx);
        cx.notify();
        removed
    }

    /// Takes away the cards of the captures the store has deleted, wherever they are listed, and
    /// returns how many went. A day group left empty goes with them; the session group stays. A
    /// fetch under way is dropped even when no listed card went, since its page was read before
    /// the delete and may list some of them, and the next render asks again. A focused card that
    /// goes stops being rendered, which the main window answers by refocusing, and the current
    /// card is chosen afresh if it went.
    pub fn remove_captures(&mut self, deleted: &[CaptureId], cx: &mut Context<Self>) -> usize {
        self.move_current_off(|card| deleted.contains(&card.files.id));
        let mut removed = 0;
        for group in &mut self.groups {
            let (gone, kept): (Vec<Card>, Vec<Card>) = std::mem::take(&mut group.cards)
                .into_iter()
                .partition(|card| deleted.contains(&card.files.id));
            group.cards = kept;
            removed += gone.len();
            self.cleared.extend(gone);
        }
        self.groups
            .retain(|group| matches!(group.kind, GroupKind::Session) || !group.cards.is_empty());
        if self.current.is_some_and(|id| self.card(id).is_none()) {
            self.current = None;
        }
        self.fetch = None;
        self.fetch_failed = false;
        self.rebuild_rows(cx);
        cx.notify();
        removed
    }

    /// Takes these cards away, whichever groups they are in. A focused card that goes with them
    /// stops being rendered, which the main window answers by refocusing.
    pub fn remove_cards(&mut self, ids: &[usize], cx: &mut Context<Self>) {
        self.move_current_off(|card| ids.contains(&card.id));
        for group in &mut self.groups {
            let (gone, kept): (Vec<Card>, Vec<Card>) = std::mem::take(&mut group.cards)
                .into_iter()
                .partition(|card| ids.contains(&card.id));
            group.cards = kept;
            self.cleared.extend(gone);
        }
        self.rebuild_rows(cx);
        cx.notify();
    }

    /// Before the cards `gone` picks are taken away: if the current card is one of them, the
    /// next listed card that stays becomes the current one, or failing that the last one before
    /// it, so focus lands beside what was deleted and not at the top of the view.
    fn move_current_off(&mut self, gone: impl Fn(&Card) -> bool) {
        let listed = self.listed_ids();
        self.current = current_after(&listed, self.current, |id| self.card(id).is_some_and(&gone));
    }
}

/// The current card once the `listed` cards that `gone` picks are taken away: itself if it
/// stays, else the next listed card that stays, else the last one before it.
fn current_after(
    listed: &[usize],
    current: Option<usize>,
    gone: impl Fn(usize) -> bool,
) -> Option<usize> {
    let Some(at) = listed.iter().position(|&id| Some(id) == current) else {
        return current;
    };
    let stays = |id: &usize| !gone(*id);
    let after = listed[at..].iter().copied().find(stays);
    after.or_else(|| listed[..at].iter().rev().copied().find(stays))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A deleted current card hands its place to the next card that stays, or the one before
    /// it at the end of the list; a current card that stays is left alone.
    #[test]
    fn the_current_card_moves_beside_what_was_deleted() {
        let listed = [10, 11, 12, 13];
        let after = |current, gone: &[usize]| {
            current_after(&listed, Some(current), |id| gone.contains(&id))
        };
        assert_eq!(after(11, &[11]), Some(12));
        assert_eq!(after(11, &[11, 12]), Some(13));
        assert_eq!(after(13, &[13]), Some(12));
        assert_eq!(after(12, &[12, 13]), Some(11));
        assert_eq!(after(11, &[12]), Some(11));
        assert_eq!(after(11, &listed), None);
        // A current card that isn't listed, as one a search hides, is not this function's to move.
        assert_eq!(after(99, &[11]), Some(99));
        assert_eq!(current_after(&listed, None, |_| true), None);
    }
}
