//! The gallery (DESIGN.md §2 with CLAUDE.md's overrides; keyboard §7, select mode §4).

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
/// Room for the toast and return pill, as the mockups leave.
const PADDING_BOTTOM: f32 = 64.;
/// The Select mockups' `padding-bottom: 90px`.
const BULK_PADDING_BOTTOM: f32 = 90.;
const GAP: f32 = 16.;
/// Sets how many columns fit: the mockups' three at their 980px gallery, and two beside the
/// settings column at the smallest window. Every look's chip and actions fit one row at this
/// width (`card::layout`).
const MIN_CARD_WIDTH: f32 = 268.;
/// Thumbnails are stored this wide, so no card ever stretches one.
const MAX_CARD_WIDTH: f32 = (MIN_CARD_WIDTH + GAP) * 1.5 - GAP;
const HEADER_HEIGHT: f32 = 32.;
const SELECT_HINT_GAP: f32 = 6.;
const SELECT_HINT: &str = "Select mode · Shift-click for a range · Ctrl+A for all";
const SEARCH_WIDTH: f32 = 180.;
/// `.earlier`'s margin and padding above its rule.
const EARLIER_MARGIN: f32 = 6.;
const EARLIER_PADDING: f32 = 14.;
/// `.loadmore`'s height.
const NOTICE_HEIGHT: f32 = 44.;
const PILL_TOP: f32 = 18.;
const COPIED_FOR: Duration = Duration::from_millis(1500);
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
    /// Any of its thumbnails may be held, so they need letting go.
    pub drawn: bool,
}

pub struct Card {
    /// Unique among the gallery's cards; element ids are built from it.
    pub id: usize,
    pub files: CardFiles,
    pub capture: Capture,
    pub bytes: u64,
    /// Holds the text, so edits survive scrolling out of view and back.
    pub name: Entity<InputState>,
    pub focus: gpui::FocusHandle,
    /// Why the name being typed wasn't accepted. The record keeps the last good name.
    pub name_error: Option<SharedString>,
}

#[derive(Debug)]
pub struct CardFiles {
    pub id: CaptureId,
    pub thumbnail: PathBuf,
}

#[derive(Debug)]
pub struct NewCard {
    pub files: CardFiles,
    pub capture: Capture,
    pub bytes: u64,
}

pub enum GalleryEvent {
    Copy { card: usize, capture: CaptureId },
    Renamed { capture: CaptureId, name: String },
    Save { capture: CaptureId },
    OpenFolder { file: PathBuf },
    OpenEditor(usize),
    Delete(usize),
}

pub struct Gallery {
    store: Arc<Store>,
    /// Earlier today's group leaves out this session's captures.
    session: SessionId,
    groups: Vec<Group>,
    rows: Vec<Row>,
    show_previous: bool,
    today: LocalDate,
    next: Next,
    /// Dropping it abandons the fetch.
    fetch: Option<Task<()>>,
    fetch_failed: bool,
    thumbnails: Entity<RetainAllImageCache>,
    /// Taken away since the last render, which lets go of their thumbnails.
    cleared: Vec<Card>,
    /// The last render had cards in `cleared` (`focus_to_restore`).
    cards_just_removed: bool,
    width: Pixels,
    scale: f32,
    columns: usize,
    card_layout: card::Layout,
    show_empty_state: bool,
    /// `None` unless one is working.
    region_shortcut: Option<SharedString>,
    next_card: usize,
    scroll: VirtualListScrollHandle,
    copied: Flash<usize>,
    search: Entity<InputState>,
    /// Lower-cased; empty for no filter.
    query: String,
    /// The card Tab enters the grid at, by id. `None` only while the grid has no card.
    current: Option<usize>,
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
            show_empty_state: false,
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

    pub fn set_region_shortcut(&mut self, shortcut: Option<SharedString>) {
        self.region_shortcut = shortcut;
    }

    pub fn unsaved_in_session(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| matches!(group.kind, GroupKind::Session))
            .flat_map(|group| &group.cards)
            .filter(|card| !card.capture.is_saved())
            .count()
    }

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

    /// A card from a day not fetched yet is dropped, as it will come from the store with the rest
    /// of that day.
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

    pub fn mark_saved(&mut self, capture: CaptureId, file: SavedFile, cx: &mut Context<Self>) {
        if let Some(card) = self.stored_card(capture) {
            card.capture.saved_path = Some(file.path);
            card.bytes = file.bytes;
            cx.notify();
        }
    }

    /// `(captures, saved)`.
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

    pub fn open_editor(&mut self, id: usize, cx: &mut Context<Self>) {
        cx.emit(GalleryEvent::OpenEditor(id));
    }

    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }

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

        // A newly focused control scrolls into view clear of the pill and the bulk bar.
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
        let empty = self.show_empty_state.then(|| {
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
                                // The pill goes with it, so focus moves into the gallery rather
                                // than falling back to the window's first control.
                                this.focus.focus().focus(window, cx);
                                cx.notify();
                            })
                        })),
                )
            })
    }
}

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

/// `.loadmore`, read out as a polite live region (DESIGN.md §7) when it appears or changes.
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
