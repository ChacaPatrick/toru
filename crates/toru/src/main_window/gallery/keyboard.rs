use gpui::{Bounds, Context, FocusHandle, KeyDownEvent, Pixels, ScrollStrategy, Window, px};
use toru_core::store::CaptureId;

use super::{Gallery, PILL_TOP};
use crate::ui::JUMP_HEIGHT;

impl Gallery {
    /// The gallery's own focus, which Tab leaves for the current card.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.focus().clone()
    }

    /// Where the main window sends focus when the element holding it inside the gallery stops
    /// being drawn. If cards were taken away in the frame just drawn, as by a delete, it is the
    /// current card, which is then the one beside them, so the arrow keys carry on from there.
    /// Otherwise it is the gallery itself, from where Tab reaches the current card: a name field
    /// the user scrolls out of the list must not pull the list back to a card.
    pub fn focus_to_restore(&self) -> FocusHandle {
        let drawn = self
            .shown()
            .into_iter()
            .find(|&(row, id)| Some(id) == self.current && self.rendered.contains(&row))
            .filter(|_| self.cards_just_removed);
        drawn
            .and_then(|(_, id)| self.card(id))
            .map_or_else(|| self.focus_handle(), |card| card.focus.clone())
    }

    /// Focuses the card of `capture`, scrolling to it if need be, when the editor closes on it.
    /// A card a search or a collapsed group keeps out of the list is left alone.
    pub fn focus_capture(
        &mut self,
        capture: CaptureId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let listed = self
            .shown()
            .into_iter()
            .find(|&(_, id)| self.card(id).is_some_and(|card| card.files.id == capture));
        if let Some((row, id)) = listed {
            self.focus_card(id, row, window, cx);
        }
    }

    /// Makes `id` the card Tab enters the grid at.
    pub(in crate::main_window) fn set_current(&mut self, id: usize, cx: &mut Context<Self>) {
        if self.current != Some(id) {
            self.current = Some(id);
            cx.notify();
        }
    }

    /// The card whose own element has focus, as opposed to one of its controls.
    fn focused_card(&self, window: &Window) -> Option<usize> {
        self.groups
            .iter()
            .flat_map(|g| &g.cards)
            .find(|c| c.focus.is_focused(window))
            .map(|c| c.id)
    }

    /// Arrow keys, Home, End, Enter and, in select mode, Space while a card itself has focus. A
    /// card's controls keep their own keys: the name field needs its arrows. Esc and Ctrl+A are
    /// the main window's, so they work from anywhere in it.
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.focused_card(window) else {
            return;
        };
        let shown = self.shown();
        let Some(at) = shown.iter().position(|&(_, card)| card == id) else {
            return;
        };
        let key = event.keystroke.key.as_str();
        match key {
            "enter" => {
                cx.stop_propagation();
                self.open_editor(id, cx);
                return;
            }
            "space" if self.select_mode => {
                cx.stop_propagation();
                self.press_card(id, event.keystroke.modifiers, cx);
                return;
            }
            _ => {}
        }
        let Some(target) = key_target(&shown, at, key) else {
            return;
        };
        // An arrow with nowhere to go still belongs to the grid.
        cx.stop_propagation();
        if target == at {
            return;
        }
        let target = shown[target];
        self.focus_card(target.1, target.0, window, cx);
    }

    /// Focuses card `id`, in `row`, scrolling the list just enough to build it when the row isn't
    /// among those rendered; `FocusInView` then brings its ring fully into view.
    fn focus_card(&mut self, id: usize, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.rendered.contains(&row) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        if let Some(card) = self.card(id) {
            card.focus.clone().focus(window, cx);
        }
        self.set_current(id, cx);
        cx.notify();
    }
}

/// The part of the list's `bounds` nothing floats over: below the return pill while it shows,
/// and above the bulk bar, `bar` tall with its margin, while it does. A focused control is
/// scrolled into this, so its ring isn't hidden under either.
pub(super) fn uncovered(
    mut bounds: Bounds<Pixels>,
    pill: bool,
    bar: Option<Pixels>,
) -> Bounds<Pixels> {
    if pill {
        let covered = px(PILL_TOP + JUMP_HEIGHT);
        bounds.origin.y += covered;
        bounds.size.height -= covered;
    }
    if let Some(bar) = bar {
        bounds.size.height -= bar;
    }
    bounds.size.height = bounds.size.height.max(px(0.));
    bounds
}

/// Where `key` moves the grid's focus from the card at `at` in `shown`, the listed cards in
/// reading order, each with its row: the neighbour for Left and Right, the same column a row
/// up or down, or that row's last card if it is shorter, and the first or last card for Home
/// and End. A move off either end stays put. `None` for a key that isn't the grid's.
fn key_target(shown: &[(usize, usize)], at: usize, key: &str) -> Option<usize> {
    let last = shown.len() - 1;
    let row = shown[at].0;
    let row_start = |row: usize| shown.iter().position(|card| card.0 == row);
    Some(match key {
        "left" => at.saturating_sub(1),
        "right" => (at + 1).min(last),
        "up" | "down" => {
            let next_row = if key == "up" {
                shown[..at].iter().rev().find(|card| card.0 != row)
            } else {
                shown[at..].iter().find(|card| card.0 != row)
            };
            let Some(&(next_row, _)) = next_row else {
                return Some(at);
            };
            let column = at - row_start(row)?;
            let start = row_start(next_row)?;
            let length = shown[start..]
                .iter()
                .take_while(|card| card.0 == next_row)
                .count();
            start + column.min(length - 1)
        }
        "home" => 0,
        "end" => last,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use gpui::{point, size};

    use super::*;
    use crate::main_window::gallery::PILL_TOP;
    #[test]
    fn focus_viewport_stays_clear_of_the_pill_and_the_bulk_bar() {
        let list = Bounds::new(point(px(0.), px(100.)), size(px(600.), px(500.)));
        assert_eq!(uncovered(list, false, None), list);
        let under_pill = uncovered(list, true, None);
        assert_eq!(under_pill.top(), px(100. + PILL_TOP + JUMP_HEIGHT));
        assert_eq!(under_pill.bottom(), px(600.));
        let above_bar = uncovered(list, false, Some(px(72.)));
        assert_eq!(above_bar.top(), px(100.));
        assert_eq!(above_bar.bottom(), px(600. - 72.));
        let both = uncovered(list, true, Some(px(72.)));
        assert_eq!(both.top(), under_pill.top());
        assert_eq!(both.bottom(), above_bar.bottom());
        // A list shorter than what covers it leaves nothing, not a negative height.
        let tiny = Bounds::new(point(px(0.), px(0.)), size(px(600.), px(40.)));
        assert_eq!(uncovered(tiny, true, Some(px(72.))).size.height, px(0.));
    }

    /// The listed cards of groups of these sizes laid out `columns` across, as `shown` gives
    /// them: each with its row, where a header takes a row before each group's cards.
    fn grid(groups: &[usize], columns: usize) -> Vec<(usize, usize)> {
        let mut shown = Vec::new();
        let (mut row, mut id) = (0, 0);
        for &cards in groups {
            for chunk in 0..cards.div_ceil(columns) {
                row += 1;
                let in_row = columns.min(cards - chunk * columns);
                shown.extend((0..in_row).map(|offset| (row, id + offset)));
                id += in_row;
            }
            row += 1;
        }
        shown
    }

    /// The id of the card `keys` lead to from card `from`.
    fn walk(shown: &[(usize, usize)], from: usize, keys: &[&str]) -> usize {
        let at = keys.iter().fold(from, |at, key| {
            key_target(shown, at, key).expect("a grid key")
        });
        shown[at].1
    }

    #[test]
    fn arrows_follow_the_columns_the_window_has_room_for() {
        // Five cards over four: 0 1 2 / 3 4, then 5 6 7 / 8.
        let three = grid(&[5, 4], 3);
        assert_eq!(walk(&three, 0, &["right", "right"]), 2);
        // Down keeps the column, or takes the last card of a shorter row.
        assert_eq!(walk(&three, 1, &["down"]), 4);
        assert_eq!(walk(&three, 2, &["down"]), 4);
        // From a short row, down goes on into the next group in the same column.
        assert_eq!(walk(&three, 4, &["down"]), 6);
        assert_eq!(walk(&three, 8, &["up"]), 5);
        assert_eq!(walk(&three, 6, &["up"]), 4);
        // Left and Right cross rows and groups in reading order.
        assert_eq!(walk(&three, 2, &["right"]), 3);
        assert_eq!(walk(&three, 5, &["left"]), 4);
        assert_eq!(walk(&three, 4, &["home"]), 0);
        assert_eq!(walk(&three, 4, &["end"]), 8);

        // The same cards at the smallest window: 0 1 / 2 3 / 4, then 5 6 / 7 8.
        let two = grid(&[5, 4], 2);
        assert_eq!(walk(&two, 1, &["down"]), 3);
        assert_eq!(walk(&two, 3, &["down"]), 4);
        assert_eq!(walk(&two, 4, &["down"]), 5);
        // Up through the short row lands in its column, the first.
        assert_eq!(walk(&two, 8, &["up", "up"]), 4);
        assert_eq!(walk(&two, 8, &["up", "up", "up"]), 2);

        // And on a wide one: 0 1 2 3 / 4, then 5 6 7 8.
        let four = grid(&[5, 4], 4);
        assert_eq!(walk(&four, 3, &["down"]), 4);
        assert_eq!(walk(&four, 4, &["down"]), 5);
        assert_eq!(walk(&four, 8, &["up"]), 4);
        assert_eq!(walk(&four, 2, &["down", "up"]), 0);
    }

    #[test]
    fn arrows_stop_at_the_ends_and_other_keys_are_not_the_grids() {
        let shown = grid(&[5, 4], 3);
        assert_eq!(walk(&shown, 0, &["left"]), 0);
        assert_eq!(walk(&shown, 1, &["up"]), 1);
        assert_eq!(walk(&shown, 8, &["right"]), 8);
        assert_eq!(walk(&shown, 8, &["down"]), 8);
        assert_eq!(key_target(&shown, 0, "tab"), None);
        assert_eq!(key_target(&shown, 0, "space"), None);
        // One card alone goes nowhere.
        let alone = grid(&[1], 3);
        for key in ["left", "right", "up", "down", "home", "end"] {
            assert_eq!(key_target(&alone, 0, key), Some(0));
        }
    }
}
