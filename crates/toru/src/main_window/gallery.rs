//! The gallery (DESIGN.md §2 with CLAUDE.md's overrides): one scrolling list holding "This
//! session", then, while "Show previous screenshots" is on, "Earlier today" and one group per
//! earlier day, each a header over a grid of cards, then the loading row for the next day. The
//! grid has as many columns as fit the gallery's width with no card narrower than
//! `MIN_CARD_WIDTH`: three at the mockups' window, two at the smallest, more on a wide one, so a
//! card stays close to the mockups' size and its thumbnail is never stretched. With no card to
//! list and no search, the look's empty state (`empty`) fills the space under the session header.
//! A day group's Collapse folds its cards away, and the search field in the session header
//! filters every group's cards by name as the user types. A card being renamed stays listed
//! whatever its name becomes, and is filtered again when the field loses focus, so the typing
//! has somewhere to go.
//!
//! The list is `gpui-kit`'s virtual list (`docs/spikes.md` §1.3): it needs every row's height
//! up front, so the rows are rebuilt when a group collapses or expands, a day arrives, the earlier
//! groups are shown or hidden, the search changes or the window's width changes the column
//! count, and the card height comes from `card::layout` at the gallery's current width. Each
//! name field's text lives in its `InputState`, so edits survive scrolling out of view and back.
//!
//! Earlier days come from the store a day at a time (`history`), off the main thread, once the
//! list is nearly scrolled to its end. A search only looks through what is loaded and doesn't pull
//! in more days, which the row at the bottom says. Thumbnails take up GPU memory for as long as
//! they are held, so a group's are let go once it is far from the view or collapsed, and load
//! again when it comes back.
//!
//! Keyboard (DESIGN.md §7): the cards are a roving tab stop. Tab enters the grid at the current
//! card, the arrow keys move between cards, Enter opens one, and only the current card's own
//! controls are tab stops, so Tab doesn't walk every control of every card. Moving focus scrolls
//! the card into view with its ring, and when the current card is scrolled out of the list the
//! first card still listed takes its place, so Tab never lands out of sight.
//!
//! Select mode (DESIGN.md §4): Select, a Ctrl-click on a thumbnail, or Ctrl+A enters it; every
//! card then shows a checkbox, the session header keeps its search field (CLAUDE.md) with Select
//! all and Done in Select's place and the mode's hint in its own hint's, and a card's lower row
//! shows its path hint in place of its actions. A press on a checkbox or thumbnail toggles the card, a Shift-press ranges from the
//! last one pressed, and Space does the same on the focused card. The selection is kept by card
//! id, so it survives scrolling, and is cut down to the listed cards whenever the rows are
//! rebuilt: a card a search filters out, one folded away by Collapse or one hidden by turning
//! Show previous off drops out of it, so the bulk bar's counts always match the checkboxes on
//! view. Esc or Done leaves, clearing the selection. The main window draws the bulk bar from
//! `selection_counts` and routes Esc and Ctrl+A here from anywhere in the window.

use std::{ops::Range, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    Modifiers, Pixels, Render, Resource, RetainAllImageCache, Role, ScrollStrategy, SharedString,
    Size, Task, Window, accesskit::Live, div, point, prelude::*, px, size,
};
use gpui_kit::component::{
    VirtualListScrollHandle,
    input::{InputEvent, InputState},
    scroll::ScrollableElement,
    v_virtual_list,
};
use toru_core::{
    capture::Capture,
    export::SavedFile,
    store::{CaptureId, SessionId, Store},
    time::LocalDate,
};

use super::{
    HINT_LEADING, act, bulk_bar, card,
    delete::DeleteCard,
    empty,
    history::{self, Next, Page},
    selection::{Counts, Selection, enters_select_mode},
};
use crate::theme::{ActiveTheme, DayDate, Theme, ThemedText};
use crate::ui::{Button, Flash, FocusInView, Icon, JUMP_HEIGHT, Jump, Readable, TextField};

const PADDING_X: f32 = 18.;
const PADDING_TOP: f32 = 16.;
/// Below the last row: room for the toast and return pill, as the mockups leave.
const PADDING_BOTTOM: f32 = 64.;
/// Below the last row while the bulk bar floats over the gallery's foot (the Select mockups'
/// `padding-bottom: 90px`).
const BULK_PADDING_BOTTOM: f32 = 90.;
/// Between a header and its grid, between card rows, and between columns.
const GAP: f32 = 16.;
/// The narrowest a card may be, which sets how many columns a width holds: the mockups' three
/// at their 980px gallery, where a card is 304px, and two beside the settings column at the
/// smallest window. Every look's chip and actions fit one row at this width (`card::layout`).
const MIN_CARD_WIDTH: f32 = 268.;
/// The widest a card gets, just before the width has room for a third column: 1.35 times the
/// mockups' card. Thumbnails are stored this wide, so no card ever stretches one.
const MAX_CARD_WIDTH: f32 = (MIN_CARD_WIDTH + GAP) * 1.5 - GAP;
/// A header row is as tall as its controls: the search field and Select, or Collapse.
const HEADER_HEIGHT: f32 = 32.;
/// Between the session header's row and select mode's hint line beneath it.
const SELECT_HINT_GAP: f32 = 6.;
const SELECT_HINT: &str = "Select mode · Shift-click for a range · Ctrl+A for all";
const SEARCH_WIDTH: f32 = 180.;
/// `.earlier`'s margin and padding above its rule.
const EARLIER_MARGIN: f32 = 6.;
const EARLIER_PADDING: f32 = 14.;
/// `.loadmore`'s height, also used for the no-match notice.
const NOTICE_HEIGHT: f32 = 44.;
/// The return pill's distance from the top of the gallery.
const PILL_TOP: f32 = 18.;
const COPIED_FOR: Duration = Duration::from_millis(1500);
/// How far below the view the next day is fetched.
const FETCH_AHEAD: Pixels = px(1200.);
/// How many views' heights above and below the view a group keeps its thumbnails, so scrolling a
/// little and back doesn't reload them.
const KEEP_VIEWS: f32 = 1.;
const RETRY_FETCH_AFTER: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub enum GroupKind {
    Session,
    /// A day's captures from sessions that have ended, titled "Earlier today" if that's today.
    Day(LocalDate),
}

pub struct Group {
    pub kind: GroupKind,
    pub cards: Vec<Card>,
    pub collapsed: bool,
    /// Whether any of its thumbnails may be held, so they need letting go.
    pub drawn: bool,
}

pub struct Card {
    /// Unique among the gallery's cards; element ids are built from it.
    pub id: usize,
    pub files: CardFiles,
    pub capture: Capture,
    pub bytes: u64,
    pub name: Entity<InputState>,
    /// The card itself, which the arrow keys move between.
    pub focus: gpui::FocusHandle,
    /// Why the name being typed wasn't accepted. The record keeps the last good name.
    pub name_error: Option<SharedString>,
}

/// Where a capture lives in the store.
#[derive(Debug)]
pub struct CardFiles {
    pub id: CaptureId,
    pub thumbnail: PathBuf,
}

/// A capture from the store on its way to becoming a card.
#[derive(Debug)]
pub struct NewCard {
    pub files: CardFiles,
    pub capture: Capture,
    pub bytes: u64,
}

enum Row {
    Header(usize),
    /// Up to a row of a group's cards, by index in the group: those the search lets through.
    Cards {
        group: usize,
        cards: Vec<usize>,
    },
    /// Under the session header while a search leaves the session empty. `elsewhere`: whether
    /// another listed group has a match, collapsed or not.
    NoMatch {
        elsewhere: bool,
    },
    /// The next day to come from the store. `None`: a first fetch has failed, so which day that
    /// is isn't known.
    NextDay(Option<LocalDate>),
    Footer,
}

pub enum GalleryEvent {
    /// A card's Copy, which the main window carries out.
    Copy {
        card: usize,
        capture: CaptureId,
    },
    Renamed {
        capture: CaptureId,
        name: String,
    },
    Save {
        capture: CaptureId,
    },
    OpenFolder {
        file: PathBuf,
    },
    /// A card's `⋯ → Open in editor`, a double-click on its thumbnail, or Enter on the card.
    OpenEditor(usize),
    /// A card's `⋯ → Delete`.
    Delete(usize),
}

pub struct Gallery {
    store: Arc<Store>,
    /// The running session, whose captures earlier today's group leaves out.
    session: SessionId,
    groups: Vec<Group>,
    rows: Vec<Row>,
    /// The "Show previous screenshots" setting: whether any group but the session's is listed.
    show_previous: bool,
    today: LocalDate,
    next: Next,
    /// The fetch under way. Dropping it abandons the fetch.
    fetch: Option<Task<()>>,
    fetch_failed: bool,
    /// Holds the thumbnails, and lets them go from the GPU's atlas when asked.
    thumbnails: Entity<RetainAllImageCache>,
    /// Cards taken away since the last render, whose thumbnails it then lets go of.
    cleared: Vec<Card>,
    /// Whether the last render had any such cards (`focus_to_restore`).
    cards_just_removed: bool,
    /// The width the main window gives the gallery, from which the columns and the card width
    /// follow.
    width: Pixels,
    scale: f32,
    columns: usize,
    card_layout: card::Layout,
    /// Nothing is listed and nothing is being searched or loaded, so the empty state shows.
    empty: bool,
    /// The Region shortcut as the settings show it, if one is working, for the empty state.
    region_shortcut: Option<SharedString>,
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
    select_mode: bool,
    selection: Selection,
}

impl EventEmitter<GalleryEvent> for Gallery {}

impl Gallery {
    pub fn new(
        store: Arc<Store>,
        session: SessionId,
        today: LocalDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let groups = vec![Group {
            kind: GroupKind::Session,
            cards: Vec::new(),
            collapsed: false,
            drawn: false,
        }];
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
            store,
            session,
            rows: Vec::new(),
            current: None,
            groups,
            show_previous: true,
            today,
            next: Next::Unknown,
            fetch: None,
            fetch_failed: false,
            thumbnails: RetainAllImageCache::new(cx),
            cleared: Vec::new(),
            cards_just_removed: false,
            width: px(0.),
            scale: 1.,
            columns: 1,
            card_layout: card::Layout {
                height: px(0.),
                open_folder_icon: false,
            },
            empty: false,
            region_shortcut: None,
            next_card: 0,
            scroll: VirtualListScrollHandle::new(),
            copied: Flash::new(),
            search,
            query: String::new(),
            focus: FocusInView::new(cx),
            rendered: 0..0,
            select_mode: false,
            selection: Selection::default(),
        };
        gallery.rebuild_rows(cx);
        gallery
    }

    /// Takes the width the main window gives the gallery. A width that changes how many columns
    /// fit lays the cards out again, keeping what was at the top of the view there.
    pub fn set_width(&mut self, width: Pixels, scale: f32, cx: &gpui::App) {
        self.width = width;
        self.scale = scale;
        let columns = columns_for(width);
        if columns == self.columns {
            return;
        }
        // The row at the top of the view and how much of it is above the view. A list at its
        // start stays there.
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

    /// What `row` shows first, to find it again once the rows are rebuilt.
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

    /// The Region shortcut to name in the empty state, or `None` if none is working.
    pub fn set_region_shortcut(&mut self, shortcut: Option<SharedString>) {
        self.region_shortcut = shortcut;
    }

    /// How many of this session's captures aren't saved to the user's folder.
    pub fn unsaved_in_session(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| matches!(group.kind, GroupKind::Session))
            .flat_map(|group| &group.cards)
            .filter(|card| !card.capture.is_saved())
            .count()
    }

    /// How wide to store the thumbnail of a picture of `size` so that no card has to stretch
    /// it, whatever the window's width.
    pub fn thumbnail_width(&self, size: (u32, u32)) -> u32 {
        thumbnail_width_for(size, self.scale)
    }

    /// Puts a capture at the top of this session. The name field's `Window` is why the main
    /// window passes one in.
    /// Returns the new card's id, for "Open editor after capture".
    pub fn add_session_card(
        &mut self,
        new: NewCard,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let card = self.new_card(new, window, cx);
        let id = card.id;
        self.groups[0].cards.insert(0, card);
        self.rebuild_rows(cx);
        cx.notify();
        id
    }

    fn new_card(&mut self, new: NewCard, window: &mut Window, cx: &mut Context<Self>) -> Card {
        self.next_card += 1;
        new_card(self.next_card, new, window, cx)
    }

    /// Ends the session: its cards go down into their day's group, and `session` starts empty on
    /// `today`. A card from a day not fetched yet is dropped, as it will come from the store with
    /// the rest of that day.
    pub fn end_session(
        &mut self,
        session: SessionId,
        today: LocalDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session = session;
        self.today = today;
        // Asked for by the old session, so it may list what is about to move.
        self.fetch = None;
        self.fetch_failed = false;
        for card in std::mem::take(&mut self.groups[0].cards).into_iter().rev() {
            let day = card.capture.taken_at.date;
            if !self.next.has_fetched(day) {
                self.release_thumbnail(&card, window, cx);
                continue;
            }
            let days: Vec<LocalDate> = self.groups[1..]
                .iter()
                .filter_map(|group| match group.kind {
                    GroupKind::Day(day) => Some(day),
                    GroupKind::Session => None,
                })
                .collect();
            let (slot, exists) = history::day_slot(&days, day);
            let index = slot + 1;
            if !exists {
                self.groups.insert(
                    index,
                    Group {
                        kind: GroupKind::Day(day),
                        cards: Vec::new(),
                        collapsed: false,
                        drawn: false,
                    },
                );
            }
            self.groups[index].cards.insert(0, card);
            self.groups[index].drawn = true;
        }
        self.groups[0].drawn = false;
        self.rebuild_rows(cx);
        cx.notify();
    }

    fn release_thumbnail(&self, card: &Card, window: &mut Window, cx: &mut Context<Self>) {
        let source = Resource::from(card.files.thumbnail.clone());
        self.thumbnails
            .update(cx, |thumbnails, cx| thumbnails.remove(&source, window, cx));
    }

    /// Lets go of the thumbnails of every group with none of its cards near the view.
    fn release_far_thumbnails(
        &mut self,
        near: &[bool],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (index, near) in near.iter().enumerate() {
            if self.groups[index].drawn && !near {
                self.groups[index].drawn = false;
                for card in &self.groups[index].cards {
                    self.release_thumbnail(card, window, cx);
                }
            }
        }
    }

    /// Starts fetching the next day if the list is close to running out and nothing holds it back.
    /// Called each frame, so days keep coming until the view has filled.
    fn fetch_when_near_bottom(
        &mut self,
        content: Pixels,
        scrolled: Pixels,
        viewport: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.show_previous
            || !self.query.is_empty()
            || self.fetch.is_some()
            || self.next == Next::End
        {
            return;
        }
        if !history::near_bottom(content, scrolled, viewport, FETCH_AHEAD) {
            return;
        }
        let (store, session, today, next) =
            (self.store.clone(), self.session, self.today, self.next);
        self.fetch = Some(cx.spawn_in(window, async move |this, cx| {
            let page = cx
                .background_spawn(async move { history::fetch_page(&store, session, today, next) })
                .await;
            match page {
                Ok(page) => {
                    this.update_in(cx, |this, window, cx| this.show_page(page, window, cx))
                        .ok();
                }
                Err(_) => {
                    this.update(cx, |this, cx| {
                        this.fetch_failed = true;
                        this.rebuild_rows(cx);
                        cx.notify();
                    })
                    .ok();
                    cx.background_executor().timer(RETRY_FETCH_AFTER).await;
                    this.update(cx, |this, cx| {
                        this.fetch = None;
                        cx.notify();
                    })
                    .ok();
                }
            }
        }));
    }

    /// Adds a fetched day below the groups listed so far, unless it holds nothing to list.
    fn show_page(&mut self, page: Option<Page>, window: &mut Window, cx: &mut Context<Self>) {
        self.fetch = None;
        self.fetch_failed = false;
        self.next = page.as_ref().map_or(Next::End, |page| page.next);
        if let Some(page) = page.filter(|page| !page.cards.is_empty()) {
            self.remake_narrow_thumbnails(&page.cards, window, cx);
            let cards = page
                .cards
                .into_iter()
                .map(|new| self.new_card(new, window, cx))
                .collect();
            self.groups.push(Group {
                kind: GroupKind::Day(page.day),
                cards,
                collapsed: false,
                drawn: false,
            });
        }
        self.rebuild_rows(cx);
        cx.notify();
    }

    /// Thumbnails stored before the cards could grow, or at a lower display scale, are narrower
    /// than a card needs, so the card would stretch them: they are made again from the pictures,
    /// off the main thread, and the cards then read them afresh.
    fn remake_narrow_thumbnails(
        &self,
        cards: &[NewCard],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let wanted: Vec<_> = cards
            .iter()
            .map(|card| {
                let size = (card.capture.width, card.capture.height);
                (card.files.id, self.thumbnail_width(size))
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let remade: Vec<_> = cx
                .background_spawn(async move {
                    wanted
                        .into_iter()
                        .filter(|&(id, width)| {
                            store.remake_narrow_thumbnail(id, width).unwrap_or(false)
                        })
                        .map(|(id, _)| id)
                        .collect()
                })
                .await;
            if remade.is_empty() {
                return;
            }
            this.update_in(cx, |this, window, cx| {
                for capture in remade {
                    this.reload_thumbnail(capture, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Lets go of the card's thumbnail, so the next frame reads it from its file again.
    fn reload_thumbnail(
        &mut self,
        capture: CaptureId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        let source = Resource::from(card.files.thumbnail.clone());
        self.thumbnails
            .update(cx, |thumbnails, cx| thumbnails.remove(&source, window, cx));
        cx.notify();
    }

    fn stored_card(&mut self, capture: CaptureId) -> Option<&mut Card> {
        self.groups
            .iter_mut()
            .flat_map(|group| &mut group.cards)
            .find(|card| card.files.id == capture)
    }

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

    fn restore_refused_name(
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

    /// The card's picture has changed: its size, and the thumbnail, read afresh from its file.
    /// A saved card's `bytes` are its file's, which going Back doesn't touch.
    pub fn picture_changed(
        &mut self,
        capture: CaptureId,
        (width, height): (u32, u32),
        bytes: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        card.capture.width = width;
        card.capture.height = height;
        if !card.capture.is_saved() {
            card.bytes = bytes;
        }
        self.reload_thumbnail(capture, window, cx);
    }

    /// Turns a card to Saved once its file is written.
    pub fn mark_saved(&mut self, capture: CaptureId, file: SavedFile, cx: &mut Context<Self>) {
        if let Some(card) = self.stored_card(capture) {
            card.capture.saved_path = Some(file.path);
            card.bytes = file.bytes;
            cx.notify();
        }
    }

    /// This session's captures and how many of them are saved, for the bottom bar.
    pub fn session_counts(&self) -> (usize, usize) {
        let cards = &self.groups[0].cards;
        let saved = cards.iter().filter(|c| c.capture.is_saved()).count();
        (cards.len(), saved)
    }

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

    /// Lists or drops the groups before this session. A focused card that goes with them stops
    /// being rendered, which the main window's `on_focus_lost` answers by refocusing.
    pub fn set_show_previous(&mut self, on: bool, cx: &Context<Self>) {
        if self.show_previous != on {
            self.show_previous = on;
            self.rebuild_rows(cx);
        }
    }

    fn toggle_collapsed(&mut self, group: usize, cx: &mut Context<Self>) {
        self.groups[group].collapsed = !self.groups[group].collapsed;
        self.rebuild_rows(cx);
        cx.notify();
    }

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

    fn enter_select_mode(&mut self, cx: &mut Context<Self>) {
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

    /// Before the cards `gone` picks are taken away: if the current card is one of them, the
    /// next listed card that stays becomes the current one, or failing that the last one before
    /// it, so focus lands beside what was deleted and not at the top of the view.
    fn move_current_off(&mut self, gone: impl Fn(&Card) -> bool) {
        let listed = self.listed_ids();
        self.current = current_after(&listed, self.current, |id| self.card(id).is_some_and(&gone));
    }

    /// A press on a card's checkbox or thumbnail, or Space on the card, with the modifiers held.
    /// Outside select mode only a Ctrl-press, without Alt, counts, and enters it; a Shift-press
    /// ranges from the last card pressed on its own.
    pub(super) fn press_card(&mut self, id: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
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
    fn listed_ids(&self) -> Vec<usize> {
        self.shown().into_iter().map(|(_, id)| id).collect()
    }

    pub fn copy(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(capture) = self.card(id).map(|card| card.files.id) {
            cx.emit(GalleryEvent::Copy { card: id, capture });
        }
    }

    /// A card's Save, or its Open folder once saved.
    pub fn save_or_open(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(card) = self.card(id) else {
            return;
        };
        match card.capture.saved_path.clone() {
            Some(file) => cx.emit(GalleryEvent::OpenFolder { file }),
            None => cx.emit(GalleryEvent::Save {
                capture: card.files.id,
            }),
        }
    }

    pub fn show_copied(&mut self, card: usize, cx: &mut Context<Self>) {
        self.copied
            .show(card, COPIED_FOR, cx, |this| &mut this.copied);
    }

    /// Enter on a card, its `⋯ → Open in editor`, or a double-click on its thumbnail (DESIGN.md
    /// §5, §7).
    pub fn open_editor(&mut self, id: usize, cx: &mut Context<Self>) {
        cx.emit(GalleryEvent::OpenEditor(id));
    }

    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }

    /// Makes `id` the card Tab enters the grid at.
    pub(super) fn set_current(&mut self, id: usize, cx: &mut Context<Self>) {
        if self.current != Some(id) {
            self.current = Some(id);
            cx.notify();
        }
    }

    /// The list's rows: each listed group's header, then its matching cards a row at a time
    /// unless it is collapsed, the row for the next day while there is one, then the bottom
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
        self.empty = rows.len() == 1 && self.query.is_empty() && !more_to_come;
        if self.show_previous {
            match self.next {
                Next::Day(day) => rows.push(Row::NextDay(Some(day))),
                Next::Unknown if self.fetch_failed => rows.push(Row::NextDay(None)),
                _ => {}
            }
        }
        rows.push(Row::Footer);
        self.rows = rows;
        self.selection.retain_listed(&self.listed_ids());
    }

    /// The card with `id`, for the editor to open.
    pub fn card(&self, id: usize) -> Option<&Card> {
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

    /// Arrow keys, Home, End, Enter and, in select mode, Space while a card itself has focus. A
    /// card's controls keep their own keys: the name field needs its arrows. Esc and Ctrl+A are
    /// the main window's, so they work from anywhere in it.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    fn card_width(&self) -> Pixels {
        card_width_at(self.width, self.columns)
    }

    fn row_height(&self, row: &Row, theme: &Theme) -> Pixels {
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

    /// How far the list is scrolled, as the next frame will show it. The virtual list clamps
    /// the offset only when it lays out, after this render, so an offset left over from a
    /// longer list is clamped here too; otherwise the return pill could stay up after turning
    /// Show previous off or collapsing groups has brought the session back into view.
    fn scrolled(&self, theme: &Theme) -> Pixels {
        let content = self
            .row_heights(theme)
            .fold(px(0.), |sum, height| sum + height);
        let furthest = (content - self.scroll.bounds().size.height).max(px(0.));
        (-self.scroll.offset().y).min(furthest)
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
        for row in range.clone() {
            if let Row::Cards { group, .. } = self.rows[row] {
                self.groups[group].drawn = true;
            }
        }
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
                                select_mode: self.select_mode,
                                selected: self.selection.contains(card.id),
                                open_folder_icon: self.card_layout.open_folder_icon,
                            },
                            &self.thumbnails,
                            window,
                            cx,
                        )
                    }))
                    .children((cards.len()..self.columns).map(|_| card::empty_slot(theme)))
            }
            Row::NoMatch { elsewhere } => base.pb(px(GAP)).child(notice(
                "no-match",
                no_match_text(&self.query, elsewhere),
                None,
                theme,
                cx,
            )),
            Row::NextDay(day) => {
                let strings = &theme.strings;
                let date = day.map_or_else(
                    || "earlier days".to_string(),
                    |day| day_date(day, strings.day_date),
                );
                let (text, icon) = if !self.query.is_empty() {
                    (
                        format!("Searching loaded days only · {date} and earlier not loaded"),
                        None,
                    )
                } else if self.fetch_failed {
                    (format!("Couldn't load {date}. Trying again…"), None)
                } else {
                    let (before, after) = strings.loading;
                    (format!("{before}{date}{after}"), Some(Icon::Spinner))
                };
                base.pb(px(GAP))
                    .child(notice("next-day", text, icon, theme, cx))
            }
            Row::Footer => base,
        }
        .into_any_element()
    }

    /// The session header with the search field and Select at its right, and select mode's
    /// hint on a line beneath while the mode is on, or an earlier group's header with its rule
    /// above and Collapse at its right.
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
        // Collapse and Expand name their group by this, without the look's "Earlier — ".
        let short_title = match group.kind {
            GroupKind::Session => strings.session.to_string(),
            GroupKind::Day(date) if date == self.today => strings.earlier_today.to_string(),
            GroupKind::Day(date) => day_date(date, strings.day_date),
        };
        let (title, hint, color) = match group.kind {
            GroupKind::Session => (
                short_title.clone(),
                format!(
                    "{} · {saved} saved · nothing saved to your folder until you save",
                    plural(count, "capture")
                ),
                c.text,
            ),
            GroupKind::Day(date) => (
                if date == self.today {
                    short_title.clone()
                } else {
                    format!("{}{short_title}", strings.earlier)
                },
                format!(
                    "{} · {} unsaved · shown because “Show previous” is on",
                    plural(count, "capture"),
                    count - saved
                ),
                theme.window_muted(),
            ),
        };
        let titles = div()
            .id(("group-heading", index))
            .read_as_heading(format!("{title} · {hint}"))
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
                    .text_color(theme.window_muted())
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
            // The search field stays through select mode (CLAUDE.md), so a search that is still
            // filtering the cards is on view; Select all and Done take Select's place.
            let controls = div().flex_none().flex().items_center().gap(px(8.)).child(
                TextField::new(&self.search, "Search names")
                    .height(px(HEADER_HEIGHT))
                    .w(px(SEARCH_WIDTH)),
            );
            let controls = if self.select_mode {
                controls
                    .child(
                        Button::new("select-all", "Select all")
                            .small()
                            .on_click(act(cx, |this, cx| this.select_all(cx))),
                    )
                    .child(
                        Button::new("done", "Done")
                            .small()
                            .primary()
                            .on_click(act(cx, |this, cx| this.leave_select_mode(cx))),
                    )
            } else {
                controls.child(
                    Button::new("select", "Select")
                        .small()
                        .on_click(act(cx, |this, cx| this.enter_select_mode(cx))),
                )
            };
            let row = row.child(controls);
            if !self.select_mode {
                return row.into_any_element();
            }
            // The session line stays through select mode (CLAUDE.md), so the mode's hint goes
            // on a line of its own beneath the row, where it has the gallery's whole width.
            let hint_style = theme.type_scale.hint;
            return div()
                .w_full()
                .flex()
                .flex_col()
                .child(row)
                .child(
                    div()
                        .id("select-hint")
                        .read_as_text(SELECT_HINT)
                        .mt(px(SELECT_HINT_GAP))
                        .type_style(theme, hint_style)
                        .line_height(hint_style.size * HINT_LEADING)
                        .text_color(theme.window_muted())
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(SELECT_HINT),
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
                    .named(collapse_name(group.collapsed, &short_title))
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
        self.card_layout = card::layout(self.card_width(), theme, window);
        let session_bottom = self.session_bottom(theme);
        let pill = self.scrolled(theme) > session_bottom;

        // Scroll a newly focused control into view, keeping it clear of the pill and the bulk bar.
        let bar =
            (!self.selection.is_empty()).then(|| px(bulk_bar::BOTTOM) + bulk_bar::height(theme));
        let viewport = uncovered(self.scroll.bounds(), pill, bar);
        let by = self.focus.scroll_by(viewport, window, cx);
        if by != px(0.) {
            self.scroll
                .set_offset(self.scroll.offset() + point(px(0.), by));
        }
        // After that scroll, so the pill answers to where the list is now.
        let theme = cx.theme();
        let pill = self.scrolled(theme) > session_bottom;

        let sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            self.rows
                .iter()
                .map(|row| size(px(1.), self.row_height(row, theme)))
                .collect(),
        );
        let content = sizes.iter().fold(px(0.), |sum, row| sum + row.height);
        let rows = self.rows.iter().zip(sizes.iter()).map(|(row, size)| {
            let group = match row {
                Row::Cards { group, .. } => Some(*group),
                _ => None,
            };
            (group, size.height)
        });
        let scrolled = -self.scroll.offset().y;
        let viewport = self.scroll.bounds().size.height;
        let near = history::groups_near(
            rows,
            scrolled - viewport * KEEP_VIEWS..scrolled + viewport * (1. + KEEP_VIEWS),
            self.groups.len(),
        );
        let list = v_virtual_list(cx.entity(), "gallery", sizes, |this, range, window, cx| {
            this.render_rows(range, window, cx)
        })
        .track_scroll(&self.scroll);
        let jump = theme.strings.back_to_session;
        let gallery = cx.entity();
        self.cards_just_removed = !self.cleared.is_empty();
        for card in std::mem::take(&mut self.cleared) {
            self.release_thumbnail(&card, window, cx);
        }
        self.release_far_thumbnails(&near, window, cx);
        self.fetch_when_near_bottom(content, scrolled, viewport, window, cx);
        // Under the session header, over the list, which has nothing else to show or scroll.
        let empty = self.empty.then(|| {
            let theme = cx.theme();
            div()
                .absolute()
                .top(self.row_height(&Row::Header(0), theme))
                .left_0()
                .right_0()
                .bottom_0()
                .px(px(PADDING_X))
                .overflow_hidden()
                .child(empty::empty_state(self.region_shortcut.as_deref(), theme))
        });
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
            .children(empty)
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

/// A card for a capture, with its name field. The field's `Window` is why the main window passes
/// one into `add_session_card`. The name goes to the store as it is typed, and the store refuses
/// what it can't take; a renamed card must also still be found by its new name, so a search
/// filters it again afterwards.
fn new_card(id: usize, new: NewCard, window: &mut Window, cx: &mut Context<Gallery>) -> Card {
    let NewCard {
        files,
        capture,
        bytes,
    } = new;
    let text = capture.name.clone();
    let name = cx.new(|cx| InputState::new(window, cx).default_value(text));
    let stored = files.id;
    cx.subscribe_in(
        &name,
        window,
        move |this, name, event: &InputEvent, window, cx| match event {
            InputEvent::Change => cx.emit(GalleryEvent::Renamed {
                capture: stored,
                name: name.read(cx).value().to_string(),
            }),
            // The field also blurs when the window is deactivated, though the edit isn't over: it
            // keeps focus and blurs again, with the window active, when the user does leave it.
            InputEvent::Blur if window.is_window_active() => {
                // Leaving the field gives up on a name that wasn't accepted.
                this.restore_refused_name(stored, window, cx);
                // Filtering a renamed card as it is typed would drop it from the list mid-edit,
                // taking the rest of the typing with it, so it is filtered once the edit is over.
                if !this.query.is_empty() {
                    this.rebuild_rows(cx);
                    cx.notify();
                }
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
        name_error: None,
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
        .text_color(theme.window_muted())
        .border(border.width)
        .border_color(border.color)
        .when(border.dashed, |this| this.border_dashed())
        .rounded(theme.radii.md)
        .type_style(theme, font)
        .line_height(font.natural_line_height(theme, cx))
        .whitespace_nowrap()
        .children(icon.map(|icon| icon.render(theme, theme.window_muted())))
        .child(text)
}

/// The part of the list's `bounds` nothing floats over: below the return pill while it shows,
/// and above the bulk bar, `bar` tall with its margin, while it does. A focused control is
/// scrolled into this, so its ring isn't hidden under either.
fn uncovered(mut bounds: Bounds<Pixels>, pill: bool, bar: Option<Pixels>) -> Bounds<Pixels> {
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

/// What select mode's hint line adds to the session header: its gap and one line of hint.
fn select_hint_height(theme: &Theme) -> Pixels {
    px(SELECT_HINT_GAP) + theme.type_scale.hint.size * HINT_LEADING
}

/// What was at the top of the view before the columns changed: a group's header, or a card by id.
#[derive(Clone, Copy)]
enum Anchor {
    Header(usize),
    Card(usize),
}

/// The row at the top of the view of a list of rows this tall scrolled down by `scrolled`, and
/// how much of that row is above the view. `None` once the list is scrolled past its last row.
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

/// How far down to scroll a list of rows this tall, in a view `viewport` tall, to put `row` at
/// the top of the view with `into` of it above the view, or as near to that as the list's end
/// allows.
fn scroll_showing(heights: &[Pixels], row: usize, into: Pixels, viewport: Pixels) -> Pixels {
    let sum = |heights: &[Pixels]| heights.iter().fold(px(0.), |sum, &height| sum + height);
    let furthest = (sum(heights) - viewport).max(px(0.));
    (sum(&heights[..row]) + into.min(heights[row])).min(furthest)
}

/// How many cards go across a gallery `width` wide: as many as stay `MIN_CARD_WIDTH` or wider,
/// and at least one.
fn columns_for(width: Pixels) -> usize {
    let room = f32::from(width) - PADDING_X * 2. + GAP;
    ((room / (MIN_CARD_WIDTH + GAP)).floor() as usize).max(1)
}

/// The width of each of `columns` cards across a gallery `width` wide.
fn card_width_at(width: Pixels, columns: usize) -> Pixels {
    (width - px(PADDING_X * 2. + GAP * (columns - 1) as f32)) / columns as f32
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

fn physical_width(width: Pixels, scale: f32) -> u32 {
    (f32::from(width) * scale).ceil().max(1.) as u32
}

/// How wide a `width`×`height` picture's thumbnail has to be to stay sharp on a card, and no
/// wider, since GPU memory grows quickly with it (`docs/spikes.md` §1.3): the widest card in
/// physical pixels, or, for a picture so wide that it would then be shorter than a card's
/// thumbnail, which it fills, wide enough to be that tall. The store never makes a thumbnail
/// wider than its picture.
fn thumbnail_width_for((width, height): (u32, u32), scale: f32) -> u32 {
    let widest_card = physical_width(px(MAX_CARD_WIDTH), scale);
    let thumb_height = u64::from(physical_width(px(card::THUMB_HEIGHT), scale));
    let fills_height = (thumb_height * u64::from(width)).div_ceil(u64::from(height.max(1)));
    widest_card.max(u32::try_from(fills_height).unwrap_or(u32::MAX))
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

/// What assistive technology calls a group's Collapse or Expand button.
fn collapse_name(collapsed: bool, group: &str) -> String {
    let action = if collapsed { "Expand" } else { "Collapse" };
    format!("{action} {group}")
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

    #[test]
    fn day_dates_follow_the_look() {
        assert_eq!(day_date(NOV_4, DayDate::Long), "Wed, Nov 4");
        assert_eq!(day_date(NOV_4, DayDate::ShortUpper), "NOV 4");
        assert_eq!(day_date(NOV_4, DayDate::LongUpper), "WED NOV 4");
    }

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

    #[test]
    fn thumbnail_widths_are_physical_pixels() {
        assert_eq!(physical_width(px(304.), 1.), 304);
        assert_eq!(physical_width(px(304.5), 2.), 609);
        assert_eq!(physical_width(px(0.), 1.), 1);
    }

    /// The gallery's width beside the open settings column at the smallest window.
    fn narrowest_gallery() -> f32 {
        crate::MIN_WINDOW_SIZE.0 - super::super::settings::WIDTH
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
        // Never none, however narrow.
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

    #[test]
    fn a_wide_picture_gets_a_thumbnail_tall_enough_to_fill_the_card() {
        // Most pictures: the widest card.
        assert_eq!(thumbnail_width_for((1920, 1080), 1.), 410);
        assert_eq!(thumbnail_width_for((800, 1000), 1.5), 615);
        // A strip 12 times as wide as it is tall fills the 140px thumbnail at 1680 wide.
        assert_eq!(thumbnail_width_for((1200, 100), 1.), 1680);
        assert_eq!(thumbnail_width_for((1200, 100), 2.), 3360);
        // A picture with no height can't divide by it.
        assert_eq!(thumbnail_width_for((10, 0), 1.), 1400);
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
    fn collapse_buttons_name_their_group_without_the_glyph() {
        assert_eq!(
            collapse_name(false, "Earlier today"),
            "Collapse Earlier today"
        );
        assert_eq!(collapse_name(true, "Sun, Nov 8"), "Expand Sun, Nov 8");
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(plural(1, "capture"), "1 capture");
        assert_eq!(plural(5, "capture"), "5 captures");
        assert_eq!(plural(0, "capture"), "0 captures");
    }
}
