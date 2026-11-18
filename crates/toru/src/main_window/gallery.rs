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

mod earlier_days;
mod header;
mod keyboard;
mod removing;
mod renaming;
mod rows;
mod search;
mod select_mode;
mod thumbnails;

use std::{ops::Range, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, Context, Entity, EventEmitter, Pixels, Render, RetainAllImageCache, Role,
    SharedString, Size, Task, Window, accesskit::Live, div, point, prelude::*, px, size,
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
    bulk_bar, card, empty,
    history::{self, Next},
    selection::Selection,
};
use crate::theme::{ActiveTheme, Theme, ThemedText};
use crate::ui::{Flash, FocusInView, Icon, Jump};
use header::day_date;
pub use header::plural;
use keyboard::uncovered;
use rows::Row;
use search::no_match_text;

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

    fn stored_card(&mut self, capture: CaptureId) -> Option<&mut Card> {
        self.groups
            .iter_mut()
            .flat_map(|group| &mut group.cards)
            .find(|card| card.files.id == capture)
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

    /// The card with `id`, for the editor to open.
    pub fn card(&self, id: usize) -> Option<&Card> {
        self.groups
            .iter()
            .flat_map(|g| &g.cards)
            .find(|c| c.id == id)
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
