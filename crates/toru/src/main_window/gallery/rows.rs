use gpui::{Pixels, point, px};
use toru_core::time::LocalDate;

use super::search::name_matches;
use super::{
    BULK_PADDING_BOTTOM, EARLIER_MARGIN, EARLIER_PADDING, GAP, Gallery, GroupKind, HEADER_HEIGHT,
    MIN_CARD_WIDTH, NOTICE_HEIGHT, PADDING_BOTTOM, PADDING_TOP, PADDING_X, SELECT_HINT_GAP,
};
use crate::main_window::{HINT_LEADING, history::Next};
use crate::theme::{ActiveTheme, Theme};

pub(super) enum Row {
    Header(usize),
    /// Up to a row of a group's cards, by index in the group.
    Cards {
        group: usize,
        cards: Vec<usize>,
    },
    /// Under the session header while a search leaves the session empty. `elsewhere`: another
    /// listed group has a match, collapsed or not.
    NoMatch {
        elsewhere: bool,
    },
    /// The next day to come from the store. `None`: a first fetch has failed, so which day that
    /// is isn't known.
    NextDay(Option<LocalDate>),
    Footer,
}

/// What was at the top of the view before the columns changed: a group's header, or a card by id.
#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Header(usize),
    Card(usize),
}

impl Gallery {
    /// A width that changes how many columns fit lays the cards out again, keeping what was at
    /// the top of the view there.
    pub fn set_width(&mut self, width: Pixels, scale: f32, cx: &gpui::App) {
        self.width = width;
        self.scale = scale;
        let columns = columns_for(width);
        if columns == self.columns {
            return;
        }
        // A list at its start stays there.
        let theme = cx.theme();
        let at_top = row_at(self.row_heights(theme), self.scrolled(theme))
            .filter(|&at| at != (0, px(0.)))
            .and_then(|(row, into)| Some((self.anchor_of(&self.rows[row])?, into)));
        self.columns = columns;
        self.rebuild_rows(cx);
        let Some((anchor, into)) = at_top else {
            return;
        };
        let row = self
            .rows
            .iter()
            .position(|row| self.holds_anchor(row, anchor));
        if let Some(row) = row {
            // Straight to the offset: the list's `scroll_to_item` only brings a row into view,
            // which leaves one that was at the top at the bottom once the rows above it grow.
            let heights: Vec<Pixels> = self.row_heights(theme).collect();
            let viewport = self.scroll.bounds().size.height;
            let scrolled = scroll_showing(&heights, row, into, viewport);
            self.scroll.set_offset(point(px(0.), -scrolled));
        }
    }

    fn row_heights(&self, theme: &Theme) -> impl Iterator<Item = Pixels> {
        self.rows.iter().map(|row| self.row_height(row, theme))
    }

    fn anchor_of(&self, row: &Row) -> Option<Anchor> {
        match row {
            Row::Header(group) => Some(Anchor::Header(*group)),
            Row::Cards { group, cards } => {
                Some(Anchor::Card(self.groups[*group].cards[*cards.first()?].id))
            }
            _ => None,
        }
    }

    fn holds_anchor(&self, row: &Row, anchor: Anchor) -> bool {
        match (row, anchor) {
            (Row::Header(group), Anchor::Header(wanted)) => *group == wanted,
            (Row::Cards { group, cards }, Anchor::Card(id)) => cards
                .iter()
                .any(|&index| self.groups[*group].cards[index].id == id),
            _ => false,
        }
    }

    /// `gpui-kit`'s virtual list needs every row's height up front (`docs/spikes.md` §1.3), so
    /// this runs whenever anything changes the rows. Only the session group is listed unless
    /// `show_previous`, and a search drops every other group it leaves empty.
    pub(super) fn rebuild_rows(&mut self, cx: &gpui::App) {
        let matching: Vec<Vec<usize>> = self
            .groups
            .iter()
            .map(|group| {
                if !self.show_previous && !matches!(group.kind, GroupKind::Session) {
                    return Vec::new();
                }
                (0..group.cards.len())
                    .filter(|&i| name_matches(&group.cards[i].name.read(cx).value(), &self.query))
                    .collect()
            })
            .collect();
        let elsewhere = matching
            .iter()
            .zip(&self.groups)
            .any(|(shown, group)| !shown.is_empty() && !matches!(group.kind, GroupKind::Session));
        let mut rows = Vec::new();
        for (index, (group, shown)) in self.groups.iter().zip(&matching).enumerate() {
            let session = matches!(group.kind, GroupKind::Session);
            if shown.is_empty() && !session {
                continue;
            }
            rows.push(Row::Header(index));
            // A session with no captures yet shows its header alone; the notice is for a search.
            if shown.is_empty() && !self.query.is_empty() {
                rows.push(Row::NoMatch { elsewhere });
            }
            if group.collapsed {
                continue;
            }
            for chunk in shown.chunks(self.columns) {
                rows.push(Row::Cards {
                    group: index,
                    cards: chunk.to_vec(),
                });
            }
        }
        // Only the session header is listed. While earlier days are on and the store hasn't run
        // out of them, a day may still arrive and the row for it shows, so the empty state waits.
        let more_to_come = self.show_previous && self.next != Next::End;
        self.show_empty_state = rows.len() == 1 && self.query.is_empty() && !more_to_come;
        if self.show_previous {
            match self.next {
                Next::Day(day) => rows.push(Row::NextDay(Some(day))),
                Next::Unknown if self.fetch_failed => rows.push(Row::NextDay(None)),
                _ => {}
            }
        }
        rows.push(Row::Footer);
        self.rows = rows;
        // A card filtered out, collapsed or hidden leaves the selection, so the bulk bar's counts
        // match the checkboxes on view.
        self.selection.retain_listed(&self.listed_ids());
    }

    /// `(row, card id)` in reading order.
    pub(super) fn shown(&self) -> Vec<(usize, usize)> {
        let mut shown = Vec::new();
        for (row, item) in self.rows.iter().enumerate() {
            if let Row::Cards { group, cards } = item {
                shown.extend(
                    cards
                        .iter()
                        .map(|&i| (row, self.groups[*group].cards[i].id)),
                );
            }
        }
        shown
    }

    pub(super) fn card_width(&self) -> Pixels {
        card_width_at(self.width, self.columns)
    }

    pub(super) fn row_height(&self, row: &Row, theme: &Theme) -> Pixels {
        match row {
            Row::Header(0) if self.select_mode => {
                px(PADDING_TOP + HEADER_HEIGHT + GAP) + select_hint_height(theme)
            }
            Row::Header(0) => px(PADDING_TOP + HEADER_HEIGHT + GAP),
            Row::Header(_) => {
                px(EARLIER_MARGIN + EARLIER_PADDING + HEADER_HEIGHT + GAP)
                    + theme.borders.earlier_rule.width
            }
            Row::Cards { .. } => self.card_layout.height + px(GAP),
            Row::NoMatch { .. } | Row::NextDay(_) => px(NOTICE_HEIGHT + GAP),
            Row::Footer if self.selection.is_empty() => px(PADDING_BOTTOM - GAP),
            Row::Footer => px(BULK_PADDING_BOTTOM - GAP),
        }
    }

    /// The virtual list clamps the offset only when it lays out, after this render, so an offset
    /// left over from a longer list is clamped here too; otherwise the return pill could stay up
    /// after turning Show previous off or collapsing groups has brought the session back.
    pub(super) fn scrolled(&self, theme: &Theme) -> Pixels {
        let content = self
            .row_heights(theme)
            .fold(px(0.), |sum, height| sum + height);
        let furthest = (content - self.scroll.bounds().size.height).max(px(0.));
        (-self.scroll.offset().y).min(furthest)
    }

    pub(super) fn session_bottom(&self, theme: &Theme) -> Pixels {
        self.rows
            .iter()
            .take_while(|row| {
                matches!(
                    row,
                    Row::Header(0) | Row::Cards { group: 0, .. } | Row::NoMatch { .. }
                )
            })
            .fold(px(0.), |sum, row| sum + self.row_height(row, theme))
    }
}

fn select_hint_height(theme: &Theme) -> Pixels {
    px(SELECT_HINT_GAP) + theme.type_scale.hint.size * HINT_LEADING
}

/// Also how much of that row is above the view. `None` once the list is scrolled past its last
/// row.
fn row_at(heights: impl Iterator<Item = Pixels>, scrolled: Pixels) -> Option<(usize, Pixels)> {
    let mut top = px(0.);
    for (row, height) in heights.enumerate() {
        if scrolled < top + height {
            return Some((row, scrolled - top));
        }
        top += height;
    }
    None
}

/// Puts `row` at the top of the view with `into` of it above the view, or as near to that as the
/// list's end allows.
fn scroll_showing(heights: &[Pixels], row: usize, into: Pixels, viewport: Pixels) -> Pixels {
    let sum = |heights: &[Pixels]| heights.iter().fold(px(0.), |sum, &height| sum + height);
    let furthest = (sum(heights) - viewport).max(px(0.));
    (sum(&heights[..row]) + into.min(heights[row])).min(furthest)
}

fn columns_for(width: Pixels) -> usize {
    let room = f32::from(width) - PADDING_X * 2. + GAP;
    ((room / (MIN_CARD_WIDTH + GAP)).floor() as usize).max(1)
}

fn card_width_at(width: Pixels, columns: usize) -> Pixels {
    (width - px(PADDING_X * 2. + GAP * (columns - 1) as f32)) / columns as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::main_window::gallery::thumbnails::thumbnail_width_for;
    use crate::main_window::gallery::{GAP, MAX_CARD_WIDTH, MIN_CARD_WIDTH, PADDING_X};

    fn narrowest_gallery() -> f32 {
        crate::MIN_WINDOW_SIZE.0 - crate::main_window::settings::WIDTH
    }

    #[test]
    fn columns_follow_the_gallery_width() {
        // The mockups' window: 1280 wide with the 300px settings column open.
        assert_eq!(columns_for(px(980.)), 3);
        assert_eq!(card_width_at(px(980.), 3), px(304.));
        // The smallest window, settings open, and the same window with them hidden.
        assert_eq!(columns_for(px(narrowest_gallery())), 2);
        assert_eq!(columns_for(px(crate::MIN_WINDOW_SIZE.0)), 3);
        // Hiding the settings column at the mockups' size adds a column; so does a wide window.
        assert_eq!(columns_for(px(1280.)), 4);
        assert_eq!(columns_for(px(1900. - 300.)), 5);
        // A column is added only once every card can be `MIN_CARD_WIDTH` wide.
        let four = PADDING_X * 2. + MIN_CARD_WIDTH * 4. + GAP * 3.;
        assert_eq!(columns_for(px(four - 1.)), 3);
        assert_eq!(columns_for(px(four)), 4);
        assert_eq!(columns_for(px(100.)), 1);
        assert_eq!(columns_for(px(0.)), 1);
    }

    #[test]
    fn the_top_of_the_view_stays_put_when_the_columns_change() {
        let px_all = |heights: &[f32]| heights.iter().map(|&h| px(h)).collect::<Vec<_>>();
        // A header, three rows of cards, a second header, a row of cards and the footer.
        let three = px_all(&[64., 300., 300., 300., 70., 300., 48.]);
        // The view starts 20px into the second header.
        let scrolled = px(64. + 900. + 20.);
        assert_eq!(row_at(three.iter().copied(), scrolled), Some((4, px(20.))));
        assert_eq!(row_at(three.iter().copied(), px(0.)), Some((0, px(0.))));
        assert_eq!(row_at(three.iter().copied(), px(64.)), Some((1, px(0.))));
        assert_eq!(row_at(three.iter().copied(), px(5000.)), None);

        // Two columns: the cards above take five rows, and the header is still 20px in.
        let two = px_all(&[64., 300., 300., 300., 300., 300., 70., 300., 300., 48.]);
        assert_eq!(scroll_showing(&two, 6, px(20.), px(400.)), px(1584.));
        // Five columns: they take two, and the header is the same 20px in.
        let five = px_all(&[64., 300., 300., 70., 300., 300., 300., 48.]);
        assert_eq!(scroll_showing(&five, 3, px(20.), px(400.)), px(684.));
        // A list too short to scroll that far stops at its end, and never above its start.
        let short = px_all(&[64., 300., 70., 300., 48.]);
        assert_eq!(scroll_showing(&short, 2, px(20.), px(600.)), px(182.));
        assert_eq!(scroll_showing(&short, 2, px(20.), px(900.)), px(0.));
        // A row shorter than the old one was scrolled into is passed whole, not the next too.
        assert_eq!(scroll_showing(&two, 6, px(200.), px(400.)), px(1634.));
    }

    #[test]
    fn cards_stay_near_the_mockups_width_at_every_window_width() {
        assert_eq!(MAX_CARD_WIDTH, 410.);
        for width in narrowest_gallery() as u32..=6000 {
            let width = px(width as f32);
            let columns = columns_for(width);
            let card = f32::from(card_width_at(width, columns));
            assert!(columns >= 2, "{width:?} has {columns} column");
            assert!(
                (MIN_CARD_WIDTH..=MAX_CARD_WIDTH).contains(&card),
                "{width:?}: {columns} columns of {card}"
            );
            // What is stored is never narrower than the card, so no thumbnail is stretched.
            for scale in [1., 1.25, 1.5, 2.] {
                let stored = thumbnail_width_for((3840, 2160), scale);
                assert!(stored as f32 >= card * scale, "{width:?} at {scale}");
            }
        }
    }
}
