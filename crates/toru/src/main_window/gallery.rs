//! The gallery (DESIGN.md §2 with CLAUDE.md's overrides): one scrolling list holding "This
//! session", then, while "Show previous screenshots" is on, "Earlier today" and one group per
//! earlier day, each a header over a 3-across grid of cards, then the loading row for the next
//! day. A day group's Collapse folds its cards away, and the search field in the session header
//! filters every group's cards by name as the user types. A card being renamed stays listed
//! whatever its name becomes, and is filtered again when the field loses focus, so the typing
//! has somewhere to go.
//!
//! The list is `gpui-kit`'s virtual list (`docs/spikes.md` §1.3): it needs every row's height
//! up front, so the rows are rebuilt when a group collapses or expands, a day arrives, the earlier
//! groups are shown or hidden, or the search changes, and the card height comes from
//! `card::height` at the gallery's current width. Each name field's text lives in its
//! `InputState`, so edits survive scrolling out of view and back.
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
    history::{self, Next, Page},
    selection::{Counts, Selection, enters_select_mode},
};
use crate::theme::{ActiveTheme, DayDate, Theme, ThemedText};
use crate::ui::{Button, Flash, FocusInView, Icon, JUMP_HEIGHT, Jump, TextField};

const PADDING_X: f32 = 18.;
const PADDING_TOP: f32 = 16.;
/// Below the last row: room for the toast and return pill, as the mockups leave.
const PADDING_BOTTOM: f32 = 64.;
/// Below the last row while the bulk bar floats over the gallery's foot (the Select mockups'
/// `padding-bottom: 90px`).
const BULK_PADDING_BOTTOM: f32 = 90.;
/// Between a header and its grid, between card rows, and between columns.
const GAP: f32 = 16.;
const COLUMNS: usize = 3;
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
            width: px(0.),
            scale: 1.,
            card_height: px(0.),
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

    pub fn set_width(&mut self, width: Pixels, scale: f32) {
        self.width = width;
        self.scale = scale;
    }

    /// How wide a thumbnail has to be to stay sharp on a card, and no wider, since GPU memory
    /// grows quickly with it (`docs/spikes.md` §1.3): the card's width in physical pixels.
    /// How many of this session's captures aren't saved to the user's folder.
    pub fn unsaved_in_session(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| matches!(group.kind, GroupKind::Session))
            .flat_map(|group| &group.cards)
            .filter(|card| !card.capture.is_saved())
            .count()
    }

    pub fn thumbnail_width(&self) -> u32 {
        physical_width(self.card_width(), self.scale)
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

    /// The gallery's own focus, which Tab leaves for the current card. The main window sends
    /// focus here when the element holding it stops being drawn.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.focus().clone()
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

    /// The list's rows: each listed group's header, then its matching cards three to a row
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
            for chunk in shown.chunks(COLUMNS) {
                rows.push(Row::Cards {
                    group: index,
                    cards: chunk.to_vec(),
                });
            }
        }
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
        let step =
            |by: isize| shown[(at as isize + by).clamp(0, shown.len() as isize - 1) as usize];
        let target = match event.keystroke.key.as_str() {
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
            Row::Header(0) if self.select_mode => {
                px(PADDING_TOP + HEADER_HEIGHT + GAP) + select_hint_height(theme)
            }
            Row::Header(0) => px(PADDING_TOP + HEADER_HEIGHT + GAP),
            Row::Header(_) => {
                px(EARLIER_MARGIN + EARLIER_PADDING + HEADER_HEIGHT + GAP)
                    + theme.borders.earlier_rule.width
            }
            Row::Cards { .. } => self.card_height + px(GAP),
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
            .rows
            .iter()
            .fold(px(0.), |sum, row| sum + self.row_height(row, theme));
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
                            },
                            &self.thumbnails,
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
        let (title, hint, color) = match group.kind {
            GroupKind::Session => (
                strings.session.to_string(),
                format!(
                    "{} · {saved} saved · nothing saved to your folder until you save",
                    plural(count, "capture")
                ),
                c.text,
            ),
            GroupKind::Day(date) => (
                if date == self.today {
                    strings.earlier_today.to_string()
                } else {
                    format!("{}{}", strings.earlier, day_date(date, strings.day_date))
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
                        .mt(px(SELECT_HINT_GAP))
                        .type_style(theme, hint_style)
                        .line_height(hint_style.size * HINT_LEADING)
                        .text_color(c.muted)
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
        for card in std::mem::take(&mut self.cleared) {
            self.release_thumbnail(&card, window, cx);
        }
        self.release_far_thumbnails(&near, window, cx);
        self.fetch_when_near_bottom(content, scrolled, viewport, window, cx);
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

/// What select mode's hint line adds to the session header: its gap and one line of hint.
fn select_hint_height(theme: &Theme) -> Pixels {
    px(SELECT_HINT_GAP) + theme.type_scale.hint.size * HINT_LEADING
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
