//! The gallery (DESIGN.md §2 with CLAUDE.md's overrides): one scrolling list holding "This
//! session", then, while "Show previous screenshots" is on, "Earlier today" and one group per
//! earlier day, each a header over a 3-across grid of cards, then the loading row for the next
//! day. A day group's Collapse folds its cards away, and the search field in the session header
//! filters every group's cards by name as the user types. A card being renamed stays listed
//! whatever its name becomes, and is filtered again when the field loses focus, so the typing
//! has somewhere to go.
//!
//! The list is `gpui-kit`'s virtual list (`docs/spikes.md` §1.3): it needs every row's height
//! up front, so the rows are rebuilt when a group collapses or expands, the earlier groups are
//! shown or hidden, or the search changes, and the card height comes from `card::height` at the
//! gallery's current width. Each name field's text lives in its `InputState`, so edits survive
//! scrolling out of view and back.
//!
//! Keyboard (DESIGN.md §7): the cards are a roving tab stop. Tab enters the grid at the current
//! card, the arrow keys move between cards, Enter opens one, and only the current card's own
//! controls are tab stops, so Tab doesn't walk every control of every card. Moving focus scrolls
//! the card into view with its ring, and when the current card is scrolled out of the list the
//! first card still listed takes its place, so Tab never lands out of sight.

use std::{ops::Range, path::PathBuf, rc::Rc, time::Duration};

use gpui::{
    AnyElement, Context, Entity, EventEmitter, KeyDownEvent, Pixels, Render, Role, ScrollStrategy,
    SharedString, Size, Window, accesskit::Live, div, point, prelude::*, px, size,
};
use gpui_kit::component::{
    VirtualListScrollHandle,
    input::{InputEvent, InputState},
    scroll::ScrollableElement,
    v_virtual_list,
};
use toru_core::{capture::Capture, naming::validate_name, store::CaptureId, time::LocalDate};

use super::{act, card, mock};
use crate::theme::{ActiveTheme, DayDate, Theme, ThemedText};
use crate::ui::{Button, Flash, FocusInView, Icon, JUMP_HEIGHT, Jump, TextField};

const PADDING_X: f32 = 18.;
const PADDING_TOP: f32 = 16.;
/// Below the last row: room for the toast and return pill, as the mockups leave.
const PADDING_BOTTOM: f32 = 64.;
/// Between a header and its grid, between card rows, and between columns.
const GAP: f32 = 16.;
const COLUMNS: usize = 3;
/// A header row is as tall as its controls: the search field and Select, or Collapse.
const HEADER_HEIGHT: f32 = 32.;
const SEARCH_WIDTH: f32 = 180.;
/// `.earlier`'s margin and padding above its rule.
const EARLIER_MARGIN: f32 = 6.;
const EARLIER_PADDING: f32 = 14.;
/// `.loadmore`'s height, also used for the no-match notice.
const NOTICE_HEIGHT: f32 = 44.;
/// The return pill's distance from the top of the gallery.
const PILL_TOP: f32 = 18.;
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
    /// `None` on a mock card: the earlier groups stay mock until 7.1.
    pub files: Option<CardFiles>,
    pub capture: Capture,
    pub bytes: u64,
    pub name: Entity<InputState>,
    /// The card itself, which the arrow keys move between.
    pub focus: gpui::FocusHandle,
}

/// Where a real capture lives in the store.
pub struct CardFiles {
    pub id: CaptureId,
    pub image: PathBuf,
    pub thumbnail: PathBuf,
}

/// A capture the store has just taken in, on its way to becoming a card.
pub struct NewCard {
    pub files: CardFiles,
    pub capture: Capture,
    pub bytes: u64,
}

enum Row {
    Header(usize),
    /// Up to three of a group's cards, by index in the group: those the search lets through.
    Cards {
        group: usize,
        cards: Vec<usize>,
    },
    /// Under the session header while a search leaves the session empty. `elsewhere`: whether
    /// another listed group has a match, collapsed or not.
    NoMatch {
        elsewhere: bool,
    },
    /// The next day being fetched (a mock until 7.1).
    Loading,
    Footer,
}

pub enum GalleryEvent {
    /// A mock card's Copy: nothing to copy, only the feedback.
    Copied,
    /// A real card's Copy, which the main window carries out.
    Copy {
        card: usize,
        image: PathBuf,
    },
    Renamed {
        capture: CaptureId,
        name: String,
    },
}

pub struct Gallery {
    groups: Vec<Group>,
    rows: Vec<Row>,
    /// The "Show previous screenshots" setting: whether any group but the session's is listed.
    show_previous: bool,
    today: LocalDate,
    /// The width the main window gives the gallery, from which the card width follows.
    width: Pixels,
    scale: f32,
    card_height: Pixels,
    next_card: usize,
    scroll: VirtualListScrollHandle,
    /// The card whose Copy shows a check mark.
    copied: Flash<usize>,
    search: Entity<InputState>,
    /// The search text, lower-cased, or empty for no filter.
    query: String,
    /// The card Tab enters the grid at, by id. `None` only while the grid has no card.
    current: Option<usize>,
    /// Tells a focus in the gallery from one elsewhere, and scrolls the list to a focused card.
    focus: FocusInView,
    /// The rows the list built last frame.
    rendered: Range<usize>,
}

impl EventEmitter<GalleryEvent> for Gallery {}

impl Gallery {
    pub fn new(today: LocalDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut next_card = 0;
        let session = Group {
            kind: GroupKind::Session,
            cards: Vec::new(),
            collapsed: false,
        };
        let earlier = mock::groups().into_iter().map(|(kind, captures)| Group {
            kind,
            cards: captures
                .into_iter()
                .map(|mock| {
                    next_card += 1;
                    new_card(next_card, None, mock.capture, mock.bytes, window, cx)
                })
                .collect(),
            collapsed: false,
        });
        let groups = std::iter::once(session).chain(earlier).collect::<Vec<_>>();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search names…"));
        cx.subscribe(&search, |this, search, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = search.read(cx).value().trim().to_lowercase();
                this.rebuild_rows(cx);
                cx.notify();
            }
        })
        .detach();
        let mut gallery = Gallery {
            rows: Vec::new(),
            current: groups.iter().flat_map(|g| &g.cards).next().map(|c| c.id),
            groups,
            show_previous: true,
            today,
            width: px(0.),
            scale: 1.,
            card_height: px(0.),
            next_card,
            scroll: VirtualListScrollHandle::new(),
            copied: Flash::new(),
            search,
            query: String::new(),
            focus: FocusInView::new(cx),
            rendered: 0..0,
        };
        gallery.rebuild_rows(cx);
        gallery
    }

    pub fn set_width(&mut self, width: Pixels, scale: f32) {
        self.width = width;
        self.scale = scale;
    }

    /// How wide a thumbnail has to be to stay sharp on a card, and no wider, since GPU memory
    /// grows quickly with it (`docs/spikes.md` §1.3): the card's width in physical pixels.
    pub fn thumbnail_width(&self) -> u32 {
        physical_width(self.card_width(), self.scale)
    }

    /// Puts a capture at the top of this session. The name field's `Window` is why the main
    /// window passes one in.
    pub fn add_session_card(&mut self, new: NewCard, window: &mut Window, cx: &mut Context<Self>) {
        self.today = new.capture.taken_at.date;
        self.next_card += 1;
        let card = new_card(
            self.next_card,
            Some(new.files),
            new.capture,
            new.bytes,
            window,
            cx,
        );
        self.groups[0].cards.insert(0, card);
        self.rebuild_rows(cx);
        cx.notify();
    }

    fn set_card_name(&mut self, capture: CaptureId, name: String) {
        let card = self.groups[0]
            .cards
            .iter_mut()
            .find(|card| card.files.as_ref().is_some_and(|files| files.id == capture));
        if let Some(card) = card {
            card.capture.name = name;
        }
    }

    /// Lists or drops the groups before this session. A focused card that goes with them stops
    /// being rendered, which the main window's `on_focus_lost` answers by refocusing.
    pub fn set_show_previous(&mut self, on: bool, cx: &Context<Self>) {
        if self.show_previous != on {
            self.show_previous = on;
            self.rebuild_rows(cx);
        }
    }

    /// This session's captures and how many of them are saved, for the bottom bar.
    pub fn session_counts(&self) -> (usize, usize) {
        let session = &self.groups[0].cards;
        let saved = session.iter().filter(|c| c.capture.is_saved()).count();
        (session.len(), saved)
    }

    fn toggle_collapsed(&mut self, group: usize, cx: &mut Context<Self>) {
        self.groups[group].collapsed = !self.groups[group].collapsed;
        self.rebuild_rows(cx);
        cx.notify();
    }

    pub fn copy(&mut self, id: usize, cx: &mut Context<Self>) {
        let image = self
            .groups
            .iter()
            .flat_map(|group| &group.cards)
            .find(|card| card.id == id)
            .and_then(|card| {
                let files = card.files.as_ref()?;
                Some(
                    card.capture
                        .saved_path
                        .clone()
                        .unwrap_or(files.image.clone()),
                )
            });
        match image {
            Some(image) => cx.emit(GalleryEvent::Copy { card: id, image }),
            None => {
                self.show_copied(id, cx);
                cx.emit(GalleryEvent::Copied);
            }
        }
    }

    pub fn show_copied(&mut self, card: usize, cx: &mut Context<Self>) {
        self.copied
            .show(card, COPIED_FOR, cx, |this| &mut this.copied);
    }

    /// Enter on a card. The editor arrives in 8.1.
    fn open(&mut self, _id: usize) {}

    /// Makes `id` the card Tab enters the grid at.
    pub(super) fn set_current(&mut self, id: usize, cx: &mut Context<Self>) {
        if self.current != Some(id) {
            self.current = Some(id);
            cx.notify();
        }
    }

    /// The list's rows: each listed group's header, then its matching cards three to a row
    /// unless it is collapsed, the loading row while earlier groups are listed, then the bottom
    /// padding. Only the session group is listed unless `show_previous`, and a search drops
    /// every other group it leaves empty.
    fn rebuild_rows(&mut self, cx: &gpui::App) {
        // Each group's matching cards, by index in the group; an unlisted group has none.
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
            for chunk in shown.chunks(COLUMNS) {
                rows.push(Row::Cards {
                    group: index,
                    cards: chunk.to_vec(),
                });
            }
        }
        if self.show_previous {
            rows.push(Row::Loading);
        }
        rows.push(Row::Footer);
        self.rows = rows;
    }

    fn card(&self, id: usize) -> Option<&Card> {
        self.groups
            .iter()
            .flat_map(|g| &g.cards)
            .find(|c| c.id == id)
    }

    /// The listed cards in reading order, each with its row.
    fn shown(&self) -> Vec<(usize, usize)> {
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

    /// The card whose own element has focus, as opposed to one of its controls.
    fn focused_card(&self, window: &Window) -> Option<usize> {
        self.groups
            .iter()
            .flat_map(|g| &g.cards)
            .find(|c| c.focus.is_focused(window))
            .map(|c| c.id)
    }

    /// Arrow keys, Home, End and Enter while a card itself has focus. A card's controls keep
    /// their own keys: the name field needs its arrows.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.focused_card(window) else {
            return;
        };
        let shown = self.shown();
        let Some(at) = shown.iter().position(|&(_, card)| card == id) else {
            return;
        };
        let step =
            |by: isize| shown[(at as isize + by).clamp(0, shown.len() as isize - 1) as usize];
        let target = match event.keystroke.key.as_str() {
            "enter" => {
                cx.stop_propagation();
                self.open(id);
                return;
            }
            "left" => step(-1),
            "right" => step(1),
            "up" | "down" => {
                let row = shown[at].0;
                let next_row = if event.keystroke.key == "up" {
                    shown[..at].iter().rev().find(|c| c.0 != row).map(|c| c.0)
                } else {
                    shown[at..].iter().find(|c| c.0 != row).map(|c| c.0)
                };
                let Some(next_row) = next_row else {
                    cx.stop_propagation();
                    return;
                };
                // The same column, or the last card of a shorter row.
                let column = shown[..at].iter().rev().take_while(|c| c.0 == row).count();
                let in_row: Vec<_> = shown.iter().filter(|c| c.0 == next_row).collect();
                *in_row[column.min(in_row.len() - 1)]
            }
            "home" => shown[0],
            "end" => shown[shown.len() - 1],
            _ => return,
        };
        cx.stop_propagation();
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
            Row::NoMatch { .. } | Row::Loading => px(NOTICE_HEIGHT + GAP),
            Row::Footer => px(PADDING_BOTTOM - GAP),
        }
    }

    /// How far down the content the session group ends, after which the return pill shows.
    fn session_bottom(&self, theme: &Theme) -> Pixels {
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

    /// Called by the list for the rows it builds. If the current card isn't among them, the
    /// first card that is takes over, so Tab enters the grid somewhere visible.
    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.rendered = range.clone();
        let shown = self.shown();
        let current_rendered = shown
            .iter()
            .any(|&(row, id)| Some(id) == self.current && range.contains(&row));
        if !current_rendered
            && let Some(&(_, id)) = shown.iter().find(|(row, _)| range.contains(row))
        {
            self.current = Some(id);
        }
        range.map(|row| self.render_row(row, window, cx)).collect()
    }

    fn render_row(&self, row: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
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
                    .children(cards.iter().map(|&index| {
                        let card = &self.groups[group].cards[index];
                        card::render(
                            card,
                            index,
                            self.today,
                            card::State {
                                earlier,
                                copied: self.copied.get() == Some(&card.id),
                                active: self.current == Some(card.id),
                            },
                            window,
                            cx,
                        )
                    }))
                    .children((cards.len()..COLUMNS).map(|_| card::empty_slot(theme)))
            }
            Row::NoMatch { elsewhere } => base.pb(px(GAP)).child(notice(
                "no-match",
                no_match_text(&self.query, elsewhere),
                None,
                theme,
                cx,
            )),
            Row::Loading => {
                let strings = &theme.strings;
                let text = format!(
                    "{}{}{}",
                    strings.loading.0,
                    day_date(mock::LOADING_DAY, strings.day_date),
                    strings.loading.1
                );
                base.pb(px(GAP))
                    .child(notice("loading", text, Some(Icon::Spinner), theme, cx))
            }
            Row::Footer => base,
        }
        .into_any_element()
    }

    /// The session header with the search field and Select at its right, or an earlier group's
    /// with its rule above and Collapse at its right.
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
            return row
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(8.))
                        .child(
                            TextField::new(&self.search, "Search names")
                                .height(px(HEADER_HEIGHT))
                                .w(px(SEARCH_WIDTH)),
                        )
                        // Select mode arrives in 4.5.
                        .child(Button::new("select", "Select").small().on_click(|_, _| {})),
                )
                .into_any_element();
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
        let session_bottom = self.session_bottom(theme);
        let pill = -self.scroll.offset().y > session_bottom;

        // Scroll a newly focused control into view, keeping it clear of the pill.
        let mut viewport = self.scroll.bounds();
        if pill {
            let covered = px(PILL_TOP + JUMP_HEIGHT);
            viewport.origin.y += covered;
            viewport.size.height -= covered;
        }
        let by = self.focus.scroll_by(viewport, window, cx);
        if by != px(0.) {
            self.scroll
                .set_offset(self.scroll.offset() + point(px(0.), by));
        }
        // After that scroll, so the pill answers to where the list is now.
        let pill = -self.scroll.offset().y > session_bottom;

        let theme = cx.theme();
        let sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            self.rows
                .iter()
                .map(|row| size(px(1.), self.row_height(row, theme)))
                .collect(),
        );
        let list = v_virtual_list(cx.entity(), "gallery", sizes, |this, range, window, cx| {
            this.render_rows(range, window, cx)
        })
        .track_scroll(&self.scroll);
        let jump = theme.strings.back_to_session;
        let gallery = cx.entity();
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("gallery")
                    .track_focus(self.focus.focus())
                    .on_key_down(
                        cx.listener(|this, event, window, cx| this.key_down(event, window, cx)),
                    )
                    .relative()
                    .size_full()
                    .child(list)
                    .vertical_scrollbar(&self.scroll),
            )
            // The pill lies over the list, outside the element `focus` tracks: it isn't content
            // that scrolls, so focusing it must not scroll the list to bring it "into view".
            .when(pill, |this| {
                // The row spans the gallery so the pill can sit at its centre; it takes no clicks.
                this.child(
                    div()
                        .absolute()
                        .top(px(PILL_TOP))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(Jump::new("jump", jump).on_click(move |window, cx| {
                            gallery.update(cx, |this, cx| {
                                // Straight to the offset, so this render already hides the pill.
                                this.scroll.set_offset(point(px(0.), px(0.)));
                                // The pill goes with it, so focus moves into the gallery, where
                                // Tab reaches the current card, rather than falling back to the
                                // window's first control.
                                this.focus.focus().focus(window, cx);
                                cx.notify();
                            })
                        })),
                )
            })
    }
}

/// A card for `capture`, with its name field. The field's `Window` is why the main window passes
/// one into `add_session_card`. A real card's name goes to the store as it is typed; a renamed
/// card must also still be found by its new name, so a search filters it again afterwards.
fn new_card(
    id: usize,
    files: Option<CardFiles>,
    capture: Capture,
    bytes: u64,
    window: &mut Window,
    cx: &mut Context<Gallery>,
) -> Card {
    let text = capture.name.clone();
    let name = cx.new(|cx| InputState::new(window, cx).default_value(text));
    let stored = files.as_ref().map(|files| files.id);
    cx.subscribe_in(
        &name,
        window,
        move |this, name, event: &InputEvent, window, cx| match event {
            InputEvent::Change => {
                // Anything that isn't a valid file name isn't kept; 5.5 says so on the card.
                let typed = name.read(cx).value().to_string();
                if let Some(capture) = stored
                    && validate_name(&typed).is_ok()
                {
                    this.set_card_name(capture, typed.clone());
                    cx.emit(GalleryEvent::Renamed {
                        capture,
                        name: typed,
                    });
                }
            }
            // Filtering a renamed card as it is typed would drop it from the list mid-edit, taking
            // the rest of the typing with it, so it is filtered once the edit is over. The field
            // also blurs when the window is deactivated, though the edit isn't over: it keeps
            // focus and blurs again, with the window active, when the user does leave it.
            InputEvent::Blur if !this.query.is_empty() && window.is_window_active() => {
                this.rebuild_rows(cx);
                cx.notify();
            }
            _ => {}
        },
    )
    .detach();
    Card {
        id,
        files,
        capture,
        bytes,
        name,
        focus: cx.focus_handle(),
    }
}

/// `.loadmore`: a centred line of muted text on the load fill, dashed around, read out as a
/// polite live region (DESIGN.md §7) when it appears or changes.
fn notice(
    id: &'static str,
    text: String,
    icon: Option<Icon>,
    theme: &Theme,
    cx: &gpui::App,
) -> impl IntoElement {
    let c = &theme.colors;
    let border = theme.borders.load;
    let font = theme.type_scale.load_more;
    let text: SharedString = font.case.apply(&text).into();
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(text.clone())
        .a11y_synthetic_children(|builder| {
            builder.parent_node().set_live(Live::Polite);
        })
        .w_full()
        .h(px(NOTICE_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .bg(c.load_bg)
        .text_color(c.muted)
        .border(border.width)
        .border_color(border.color)
        .when(border.dashed, |this| this.border_dashed())
        .rounded(theme.radii.md)
        .type_style(theme, font)
        .line_height(font.natural_line_height(theme, cx))
        .whitespace_nowrap()
        .children(icon.map(|icon| icon.render(theme, c.muted)))
        .child(text)
}

fn physical_width(width: Pixels, scale: f32) -> u32 {
    (f32::from(width) * scale).ceil().max(1.) as u32
}

/// Whether a search for `query`, already lower-cased and trimmed, lets a card called `name`
/// through. An empty query lets every card through.
fn name_matches(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

/// The notice under the session header when a search leaves it empty. `elsewhere`: whether an
/// earlier group has a match, even one folded away by Collapse, in which case the notice
/// says so rather than claiming nothing matches at all.
fn no_match_text(query: &str, elsewhere: bool) -> String {
    if elsewhere {
        format!("Nothing in this session matches “{query}”")
    } else {
        format!("Nothing matches “{query}”")
    }
}

pub fn plural(count: usize, noun: &str) -> String {
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
    fn thumbnail_widths_are_physical_pixels() {
        assert_eq!(physical_width(px(304.), 1.), 304);
        assert_eq!(physical_width(px(304.5), 2.), 609);
        assert_eq!(physical_width(px(0.), 1.), 1);
    }

    #[test]
    fn search_ignores_case_and_an_empty_query_matches_all() {
        assert!(name_matches("Login-Bug-Repro", "bug"));
        assert!(!name_matches("Login-Bug-Repro", "crash"));
        assert!(name_matches("anything", ""));
    }

    #[test]
    fn no_match_notice_points_at_earlier_groups() {
        assert_eq!(
            no_match_text("bug", true),
            "Nothing in this session matches “bug”"
        );
        assert_eq!(no_match_text("bug", false), "Nothing matches “bug”");
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(plural(1, "capture"), "1 capture");
        assert_eq!(plural(5, "capture"), "5 captures");
        assert_eq!(plural(0, "capture"), "0 captures");
    }
}
