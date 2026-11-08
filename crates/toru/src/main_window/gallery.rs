//! The gallery (DESIGN.md §2 with CLAUDE.md's overrides): one scrolling list holding "This
//! session", then, while "Show previous screenshots" is on, "Earlier today" and one group per
//! earlier day, each a header over a 3-across grid of cards. A day group's Collapse folds its
//! cards away.
//!
//! The list is `gpui-kit`'s virtual list (`docs/spikes.md` §1.3): it needs every row's height
//! up front, so the rows are rebuilt when a group collapses or expands or the earlier groups are
//! shown or hidden, and the card height comes from `card::height` at the gallery's current
//! width. Each name field's text lives in its
//! `InputState`, so edits survive scrolling out of view and back.

use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    AnyElement, Context, Entity, EventEmitter, Pixels, Render, Size, Window, div, prelude::*, px,
    size,
};
use gpui_kit::component::{
    VirtualListScrollHandle, input::InputState, scroll::ScrollableElement, v_virtual_list,
};
use toru_core::{capture::Capture, time::LocalDate};

use super::{act, card, mock};
use crate::theme::{ActiveTheme, DayDate, Theme, ThemedText};
use crate::ui::{Button, Flash};

const PADDING_X: f32 = 18.;
const PADDING_TOP: f32 = 16.;
/// Below the last row: room for the toast and return pill, as the mockups leave.
const PADDING_BOTTOM: f32 = 64.;
/// Between a header and its grid, between card rows, and between columns.
const GAP: f32 = 16.;
const COLUMNS: usize = 3;
/// A header row is as tall as its controls, so the session header keeps its height when 4.4 adds
/// the search field and Select to it.
const HEADER_HEIGHT: f32 = 32.;
/// `.earlier`'s margin and padding above its rule.
const EARLIER_MARGIN: f32 = 6.;
const EARLIER_PADDING: f32 = 14.;
const COPIED_FOR: Duration = Duration::from_millis(1500);

pub enum GroupKind {
    Session,
    /// Today's captures from sessions that have ended.
    EarlierToday,
    Day(LocalDate),
}

pub struct Group {
    pub kind: GroupKind,
    pub cards: Vec<Card>,
    pub collapsed: bool,
}

pub struct Card {
    /// Unique among the gallery's cards; element ids are built from it.
    pub id: usize,
    pub capture: Capture,
    pub bytes: u64,
    pub name: Entity<InputState>,
}

enum Row {
    Header(usize),
    Cards { group: usize, cards: Range<usize> },
    Footer,
}

pub enum GalleryEvent {
    Copied,
}

pub struct Gallery {
    groups: Vec<Group>,
    rows: Vec<Row>,
    /// The "Show previous screenshots" setting: whether any group but the session's is listed.
    show_previous: bool,
    today: LocalDate,
    /// The width the main window gives the gallery, from which the card width follows.
    width: Pixels,
    card_height: Pixels,
    scroll: VirtualListScrollHandle,
    /// The card whose Copy shows a check mark.
    copied: Flash<usize>,
}

impl EventEmitter<GalleryEvent> for Gallery {}

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut next_id = 0;
        let groups = mock::groups()
            .into_iter()
            .map(|(kind, captures)| Group {
                kind,
                cards: captures
                    .into_iter()
                    .map(|mock| {
                        next_id += 1;
                        let name = mock.capture.name.clone();
                        Card {
                            id: next_id,
                            capture: mock.capture,
                            bytes: mock.bytes,
                            name: cx.new(|cx| InputState::new(window, cx).default_value(name)),
                        }
                    })
                    .collect(),
                collapsed: false,
            })
            .collect::<Vec<_>>();
        Gallery {
            rows: rows(&groups, true),
            groups,
            show_previous: true,
            today: mock::TODAY,
            width: px(0.),
            card_height: px(0.),
            scroll: VirtualListScrollHandle::new(),
            copied: Flash::new(),
        }
    }

    pub fn set_width(&mut self, width: Pixels) {
        self.width = width;
    }

    /// Lists or drops the groups before this session. A focused card that goes with them stops
    /// being rendered, which the main window's `on_focus_lost` answers by refocusing.
    pub fn set_show_previous(&mut self, on: bool) {
        if self.show_previous != on {
            self.show_previous = on;
            self.rows = rows(&self.groups, on);
        }
    }

    fn toggle_collapsed(&mut self, group: usize, cx: &mut Context<Self>) {
        self.groups[group].collapsed = !self.groups[group].collapsed;
        self.rows = rows(&self.groups, self.show_previous);
        cx.notify();
    }

    /// Mock: the image goes nowhere yet, but the feedback is real.
    pub fn copy(&mut self, id: usize, cx: &mut Context<Self>) {
        self.copied
            .show(id, COPIED_FOR, cx, |this| &mut this.copied);
        cx.emit(GalleryEvent::Copied);
    }

    fn card_width(&self) -> Pixels {
        (self.width - px(PADDING_X * 2. + GAP * (COLUMNS - 1) as f32)) / COLUMNS as f32
    }

    fn row_height(&self, row: &Row, theme: &Theme) -> Pixels {
        match row {
            Row::Header(0) => px(PADDING_TOP + HEADER_HEIGHT + GAP),
            Row::Header(_) => {
                px(EARLIER_MARGIN + EARLIER_PADDING + HEADER_HEIGHT + GAP)
                    + theme.borders.earlier_rule.width
            }
            Row::Cards { .. } => self.card_height + px(GAP),
            Row::Footer => px(PADDING_BOTTOM - GAP),
        }
    }

    fn render_row(&self, row: usize, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let row = &self.rows[row];
        let height = self.row_height(row, theme);
        let base = div().w_full().h(height).px(px(PADDING_X)).flex();
        match *row {
            Row::Header(group) => base
                .when(group == 0, |base| base.pt(px(PADDING_TOP)))
                .pb(px(GAP))
                .child(self.header(group, cx)),
            Row::Cards { group, ref cards } => {
                let earlier = !matches!(self.groups[group].kind, GroupKind::Session);
                base.pb(px(GAP))
                    .gap(px(GAP))
                    .children(cards.clone().map(|index| {
                        let card = &self.groups[group].cards[index];
                        card::render(
                            card,
                            index,
                            earlier,
                            self.today,
                            self.copied.get() == Some(&card.id),
                            cx,
                        )
                    }))
                    .children((cards.len()..COLUMNS).map(|_| card::empty_slot(theme)))
            }
            Row::Footer => base,
        }
        .into_any_element()
    }

    /// The session header, or an earlier group's with its rule above and Collapse at its right.
    fn header(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let group = &self.groups[index];
        let strings = &theme.strings;
        let count = group.cards.len();
        let saved = group
            .cards
            .iter()
            .filter(|card| card.capture.is_saved())
            .count();
        let section = theme.type_scale.section;
        let (title, hint, color) = match group.kind {
            GroupKind::Session => (
                strings.session.to_string(),
                format!(
                    "{} · {saved} saved · nothing saved to your folder until you save",
                    plural(count, "capture")
                ),
                c.text,
            ),
            ref kind => (
                match kind {
                    GroupKind::Day(date) => {
                        format!("{}{}", strings.earlier, day_date(*date, strings.day_date))
                    }
                    _ => strings.earlier_today.to_string(),
                },
                format!(
                    "{} · {} unsaved · shown because “Show previous” is on",
                    plural(count, "capture"),
                    count - saved
                ),
                c.muted,
            ),
        };
        let titles = div()
            .flex()
            .items_baseline()
            .gap(px(10.))
            .min_w_0()
            .child(
                div()
                    .flex_none()
                    .type_style(theme, section)
                    .text_color(color)
                    .whitespace_nowrap()
                    .child(section.case.apply(&title)),
            )
            .child(
                div()
                    .min_w_0()
                    .type_style(theme, theme.type_scale.hint)
                    .text_color(c.muted)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(hint),
            );
        let row = div()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(titles);
        if index == 0 {
            // The search field and Select arrive at the right in 4.4.
            return row.into_any_element();
        }
        let rule = theme.borders.earlier_rule;
        div()
            .w_full()
            .mt(px(EARLIER_MARGIN))
            .pt(px(EARLIER_PADDING))
            .border_t(rule.width)
            .border_color(rule.color)
            .when(rule.dashed, |rule| rule.border_dashed())
            .child(
                row.child(
                    Button::new(
                        ("collapse", index),
                        if group.collapsed {
                            "Expand ▾"
                        } else {
                            "Collapse ▴"
                        },
                    )
                    .small()
                    .on_click(act(cx, move |this, cx| this.toggle_collapsed(index, cx))),
                ),
            )
            .into_any_element()
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        self.card_height = card::height(self.card_width(), theme, window, cx);
        let sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            self.rows
                .iter()
                .map(|row| size(px(1.), self.row_height(row, theme)))
                .collect(),
        );
        let list = v_virtual_list(cx.entity(), "gallery", sizes, |this, range, _, cx| {
            range
                .map(|row| this.render_row(row, cx))
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll);
        div()
            .relative()
            .size_full()
            .child(list)
            .vertical_scrollbar(&self.scroll)
    }
}

/// The list's rows: each listed group's header, then its cards three to a row unless it is
/// collapsed, then the bottom padding. Only the session group is listed unless `show_previous`.
fn rows(groups: &[Group], show_previous: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        if !show_previous && !matches!(group.kind, GroupKind::Session) {
            continue;
        }
        rows.push(Row::Header(index));
        if group.collapsed {
            continue;
        }
        let mut start = 0;
        while start < group.cards.len() {
            let end = (start + COLUMNS).min(group.cards.len());
            rows.push(Row::Cards {
                group: index,
                cards: start..end,
            });
            start = end;
        }
    }
    rows.push(Row::Footer);
    rows
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A day as the look writes it: "Tue, Nov 4", "NOV 4" or "TUE NOV 4".
fn day_date(date: LocalDate, style: DayDate) -> String {
    let (weekday, month, day) = (date.weekday_short(), date.month_short(), date.day);
    match style {
        DayDate::Long => format!("{weekday}, {month} {day}"),
        DayDate::ShortUpper => format!("{month} {day}").to_uppercase(),
        DayDate::LongUpper => format!("{weekday} {month} {day}").to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOV_4: LocalDate = LocalDate {
        year: 2026,
        month: 11,
        day: 4,
    };

    #[test]
    fn day_dates_follow_the_look() {
        assert_eq!(day_date(NOV_4, DayDate::Long), "Wed, Nov 4");
        assert_eq!(day_date(NOV_4, DayDate::ShortUpper), "NOV 4");
        assert_eq!(day_date(NOV_4, DayDate::LongUpper), "WED NOV 4");
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(plural(1, "capture"), "1 capture");
        assert_eq!(plural(5, "capture"), "5 captures");
        assert_eq!(plural(0, "capture"), "0 captures");
    }
}
