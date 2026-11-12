//! A gallery card (DESIGN.md §2): the thumbnail with its caption tag, the name field, the status
//! chip, Copy with Save or Open folder, and the `⋯` menu. The file size is in a tooltip on the
//! thumbnail and at the top of the menu. The gallery's list needs every row's height before it
//! is laid out, and DESIGN.md §2 asks that every card be the same height whatever its status,
//! so `layout` works both out from the theme: the chip and the actions share one row, and when
//! a saved card's row has no room for Open folder's label at the gallery's card width, every
//! saved card shows it as a folder icon instead, named and with a tooltip.
//!
//! The card is focusable itself, for the gallery's arrow keys, and shows the §7 ring when it has
//! keyboard focus. Only the gallery's current card and its controls are tab stops; a press
//! anywhere on a card makes it the current one.
//!
//! In select mode (DESIGN.md §4) a checkbox sits at the card's top-left corner, a selected card
//! takes the accent outline and the `surface_sel` fill, and the lower row shows the chip with
//! the path hint instead of the actions. Nothing changes the card's shape. A click anywhere on
//! the card outside its controls, the checkbox included, is the gallery's `press_card`, and so
//! is a Ctrl-click outside select mode, which is how the mode is entered from a card. A press
//! that starts on the name field, the buttons or the menu is theirs alone, wherever the mouse is
//! let go, so dragging out of a name field never toggles the card.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, MouseDownEvent, ObjectFit, Pixels,
    RetainAllImageCache, Role, Stateful, Window, div, img, linear_color_stop, linear_gradient,
    prelude::*, px,
};
use toru_core::time::LocalDate;

use super::{
    act,
    gallery::{Card, Gallery, GalleryEvent},
    selection::path_hint,
};
use crate::theme::{ActiveTheme, Theme, ThemedText};
use crate::ui::{
    BUTTON_SMALL_HEIGHT, Button, CHECKBOX_SIZE, Checkbox, Chip, Icon, MIN_TARGET, Menu, RingFocus,
    TextField, Tooltip, fill, focus_marks, selection_ring, shadowed, spoken,
};

const PADDING: f32 = 10.;
const GAP: f32 = 8.;
pub const THUMB_HEIGHT: f32 = 140.;
const NAME_HEIGHT: f32 = 34.;
/// Between the chip and the actions.
const ROW_GAP: f32 = 10.;
const ACTION_GAP: f32 = 6.;
const TAG_LEFT: f32 = 8.;
const TAG_BOTTOM: f32 = 6.;
/// `.check{left:18px;top:18px}`: where the checkbox's box sits inside the card.
const CHECK_INSET: f32 = 18.;

/// What every card in the gallery shares at one card width.
#[derive(Clone, Copy)]
pub struct Layout {
    pub height: Pixels,
    /// A saved card's row has no room for Open folder's label, so it is a folder icon.
    pub open_folder_icon: bool,
}

/// The cards' layout at `card_width`, from the theme. The lower row is one button tall, since
/// nothing in it wraps.
pub fn layout(card_width: Pixels, theme: &Theme, window: &Window) -> Layout {
    let border = theme.borders.card.width;
    let inner = card_width - px(PADDING * 2.) - border * 2.;
    let saved_row = Chip::new("status", SAVED).saved(true).width(theme, window)
        + px(ROW_GAP)
        + copy_button(false).width(theme, window)
        + open_folder_button(false).width(theme, window)
        + px(BUTTON_SMALL_HEIGHT + ACTION_GAP * 2.);
    Layout {
        height: border * 2.
            + px(PADDING * 2. + THUMB_HEIGHT + GAP + NAME_HEIGHT + GAP + BUTTON_SMALL_HEIGHT),
        open_folder_icon: saved_row > inner,
    }
}

const SAVED: &str = "Saved ✓";

fn copy_button(copied: bool) -> Button {
    Button::new("copy", "Copy").small().check_mark(copied)
}

/// Open folder, as its label or, with no room for it, as an icon. It shares its id with Save,
/// so keyboard focus stays on the button when a save finishes.
fn open_folder_button(icon: bool) -> Button {
    const NAME: &str = "Open folder";
    if icon {
        Button::icon_only("save-or-open", Icon::Folder, NAME)
            .small()
            .tooltip(NAME)
    } else {
        Button::new("save-or-open", NAME).small()
    }
}

/// A mouse press stops at a control this is on, so it never registers on the card beneath and
/// can't become a click on it when the mouse is let go on the card's padding.
fn keep_press(_: &MouseDownEvent, _: &mut Window, cx: &mut App) {
    cx.stop_propagation();
}

/// What assistive technology calls a card: its name, its caption (or why its name was refused,
/// which the caption tag shows instead), and whether it is saved and selected, so the status is
/// never only the colour (DESIGN.md §7).
fn card_label(name: &str, caption: &str, saved: bool, selected: bool) -> String {
    let status = if saved { "Saved" } else { "Unsaved" };
    let selected = if selected { ", selected" } else { "" };
    format!("{name}, {}, {status}{selected}", spoken(caption))
}

/// Whether a click on the card is a press on it. GPUI also turns Space and Enter's key-up on a
/// focused card into a click, but the gallery's `key_down` has already handled those keys.
fn counts_as_press(event: &ClickEvent) -> bool {
    !event.is_keyboard()
}

/// "1.4 MB" or "412 KB".
pub fn file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * KB;
    if bytes == 0 {
        // History couldn't read the file, so the size isn't known.
        "Size unknown".to_string()
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

/// What varies between two cards of one capture: `earlier` marks a group whose session has
/// ended, `copied` shows the check mark, `active` makes the card and its controls the grid's
/// tab stops, `select_mode` shows the checkbox and the path hint, and `selected` the outline.
/// `open_folder_icon` is the gallery's `Layout`'s.
#[derive(Clone, Copy)]
pub struct State {
    pub earlier: bool,
    pub copied: bool,
    pub active: bool,
    pub select_mode: bool,
    pub selected: bool,
    pub open_folder_icon: bool,
}

/// `position` is the card's place in its group, which chooses among the look's thumbnail fills.
pub fn render(
    card: &Card,
    position: usize,
    today: LocalDate,
    state: State,
    thumbnails: &Entity<RetainAllImageCache>,
    window: &Window,
    cx: &Context<Gallery>,
) -> AnyElement {
    let State {
        earlier,
        copied,
        active,
        select_mode,
        selected,
        open_folder_icon,
    } = state;
    let focus = card.focus.clone().tab_stop(active);
    let theme = cx.theme();
    let c = &theme.colors;
    let border = theme.borders.card;
    let radius = theme.radii.lg;
    let saved = card.capture.is_saved();
    let size = file_size(card.bytes);
    // `.card .row .hint`: the saved path, or "no path yet", elided from the left.
    let path = card
        .capture
        .saved_path
        .as_deref()
        .map_or_else(|| "no path yet".to_string(), path_hint);
    let label = card_label(
        &card.name.read(cx).value(),
        &card
            .name_error
            .clone()
            .map_or_else(|| card.capture.caption(today), |error| error.to_string()),
        saved,
        select_mode && selected,
    );
    let description = if select_mode {
        format!("{size}, {path}")
    } else {
        size.clone()
    };

    let id = card.id;
    let copy = act(cx, move |this, cx| this.copy(id, cx));
    let make_current = act(cx, move |this, cx| this.set_current(id, cx));
    let save_or_open = act(cx, move |this, cx| this.save_or_open(id, cx));
    let open_editor = act(cx, move |this, cx| this.open_editor(id, cx));
    let delete = act(cx, move |_, cx| cx.emit(GalleryEvent::Delete(id)));
    let gallery = cx.entity();
    let press = Rc::new(move |modifiers, _: &mut Window, cx: &mut App| {
        gallery.update(cx, |this, cx| this.press_card(id, modifiers, cx))
    });
    let press_box = press.clone();
    let rename = {
        let name = card.name.clone();
        move |window: &mut Window, cx: &mut gpui::App| {
            name.update(cx, |name, cx| {
                name.focus(window, cx);
                name.select_all(window, cx);
            })
        }
    };
    let menu = Menu::new(
        "more",
        "More actions",
        Button::glyph_only("more-button", "⋯", "More")
            .small()
            .tab_stop(active),
    )
    .header(size.clone())
    .item("Open in editor", open_editor)
    .item("Rename", rename)
    // Reveal needs a file on disk, which only a saved capture has.
    .when(saved, |menu| {
        menu.item("Reveal", act(cx, move |this, cx| this.save_or_open(id, cx)))
    })
    .divider()
    .item("Delete", delete)
    .align_right();

    let second = if saved {
        open_folder_button(open_folder_icon)
    } else {
        Button::new("save-or-open", "Save").small().primary()
    };
    let open_editor_by_click = act(cx, move |this, cx| this.open_editor(id, cx));
    let actions = div()
        .on_any_mouse_down(keep_press)
        .flex_none()
        .flex()
        .gap(px(ACTION_GAP))
        .child(copy_button(copied).tab_stop(active).on_click(copy))
        .child(second.tab_stop(active).on_click(save_or_open))
        .child(menu);
    let chip = Chip::new("status", if saved { SAVED } else { "Unsaved" }).saved(saved);
    // One button tall in select mode too, so the card keeps its shape (DESIGN.md §2).
    let status = div()
        .h(px(BUTTON_SMALL_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(ROW_GAP))
        .child(chip);
    let status = if select_mode {
        status.child(
            div()
                .flex_1()
                .min_w_0()
                .text_right()
                .type_style(theme, theme.type_scale.hint)
                .text_color(c.muted)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(path),
        )
    } else {
        status.child(actions)
    };
    let checkbox = select_mode.then(|| {
        // The box sits where the mockups put it, inside a target the minimum size (§7).
        let margin = (MIN_TARGET - CHECKBOX_SIZE) / 2.;
        div()
            .on_any_mouse_down(keep_press)
            .absolute()
            .left(px(CHECK_INSET - margin))
            .top(px(CHECK_INSET - margin))
            .child(
                Checkbox::new(
                    ("check", id),
                    format!("Select {}", card.name.read(cx).value()),
                    selected,
                )
                .tab_stop(active)
                .on_press(move |modifiers, window, cx| press_box(modifiers, window, cx)),
            )
    });

    let card_element = div()
        .id(("card", card.id))
        .role(Role::Group)
        .aria_label(label)
        .aria_description(description)
        .track_ring_focus(&focus)
        // On the way down, so it runs for a press a control keeps to itself.
        .capture_any_mouse_down(move |_, window, cx| make_current(window, cx))
        .on_click(move |event, window, cx| {
            if counts_as_press(event) {
                press(event.modifiers(), window, cx);
            }
        });
    shadowed(card_element, &theme.shadows.card, radius)
        .relative()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .p(px(PADDING))
        .bg(if selected {
            c.surface_sel
        } else if earlier {
            c.earlier_card_bg
        } else {
            c.surface
        })
        .border(border.width)
        .border_color(border.color)
        .rounded(radius)
        .child(
            // A double-click on the thumbnail opens the editor (DESIGN.md §5), outside select
            // mode, where every click is a press (§4) and Enter opens (§7). Any other click
            // goes on to the card.
            thumbnail(card, position, today, size, thumbnails, theme, cx).on_click(
                move |event, window, cx| {
                    if event.click_count() == 2 && !select_mode {
                        cx.stop_propagation();
                        open_editor_by_click(window, cx);
                    }
                },
            ),
        )
        .child(
            TextField::new(&card.name, "Screenshot name")
                .height(px(NAME_HEIGHT))
                .tab_stop(active)
                .w_full(),
        )
        .child(status)
        .children(checkbox)
        .when(selected, |this| {
            this.child(selection_ring(theme, radius, border.width))
        })
        .children(focus_marks(
            focus.is_focused(window),
            theme,
            radius,
            border.width,
            window,
            cx,
        ))
        .into_any_element()
}

/// Fills a column a short row leaves empty. It has a card's padding and border, so the cards
/// beside it stay the width of a full row's: a flex item can't be narrower than those.
pub fn empty_slot(theme: &Theme) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .p(px(PADDING))
        .border(theme.borders.card.width)
}

/// The theme's fill with the capture's thumbnail over it, under the caption tag. A thumbnail not
/// loaded yet, or whose file has gone, leaves the fill showing.
fn thumbnail(
    card: &Card,
    position: usize,
    today: LocalDate,
    size: String,
    thumbnails: &Entity<RetainAllImageCache>,
    theme: &Theme,
    cx: &App,
) -> Stateful<Div> {
    let c = &theme.colors;
    let radius = theme.radii.md;
    let border = theme.borders.thumb;
    let fills = std::iter::once(&c.thumb).chain(&c.thumb_alt);
    let fill_choice = fills.cycle().nth(position).expect("at least one fill");
    // The mockups' shade: transparent down to 60%, then the theme's shade at the bottom.
    let shade = linear_gradient(
        180.,
        linear_color_stop(
            gpui::Hsla {
                a: 0.,
                ..c.thumb_shade
            },
            0.6,
        ),
        linear_color_stop(c.thumb_shade, 1.),
    );
    let tag = theme.type_scale.tag;
    let image = img(card.files.thumbnail.clone())
        .image_cache(thumbnails)
        .absolute()
        .inset_0()
        .size_full()
        .rounded(radius)
        .object_fit(ObjectFit::Cover);

    div()
        .id("thumb")
        .relative()
        .flex_none()
        .h(px(THUMB_HEIGHT))
        .overflow_hidden()
        .rounded(radius)
        .border(border.width)
        .border_color(border.color)
        .tooltip(Tooltip::build(size))
        .child(fill(fill_choice, radius))
        .child(image)
        .child(div().absolute().inset_0().rounded(radius).bg(shade))
        .child(
            div()
                .absolute()
                .left(px(TAG_LEFT))
                .right(px(TAG_LEFT))
                .bottom(px(TAG_BOTTOM))
                .flex()
                .child(
                    div()
                        .max_w_full()
                        .px(theme.sizes.tag_padding_x)
                        .py(px(2.))
                        .bg(c.tag_bg)
                        .text_color(c.tag_text)
                        .rounded(theme.radii.sm)
                        .type_style(theme, tag)
                        .line_height(tag.natural_line_height(theme, cx))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        // A refused name is said here, right above its field, so the card keeps
                        // its shape (DESIGN.md §2).
                        .child(
                            card.name_error
                                .clone()
                                .unwrap_or_else(|| card.capture.caption(today).into()),
                        ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, KeyboardButton, KeyboardClickEvent, MouseClickEvent};

    use super::*;

    #[test]
    fn a_key_released_on_a_card_is_not_a_second_press() {
        let keyboard = ClickEvent::Keyboard(KeyboardClickEvent {
            button: KeyboardButton::Space,
            bounds: Bounds::default(),
        });
        assert!(!counts_as_press(&keyboard));
        assert!(counts_as_press(&ClickEvent::Mouse(
            MouseClickEvent::default()
        )));
    }

    #[test]
    fn a_card_is_read_with_its_caption_and_status() {
        assert_eq!(
            card_label(
                "Settings-Panel",
                "800×1000 · Window · 9:44 AM",
                false,
                false
            ),
            "Settings-Panel, 800×1000, Window, 9:44 AM, Unsaved"
        );
        assert_eq!(
            card_label("Dashboard", "1×1 · Region · 1:00 PM", true, true),
            "Dashboard, 1×1, Region, 1:00 PM, Saved, selected"
        );
    }

    #[test]
    fn file_sizes_read_like_a_file_manager() {
        assert_eq!(file_size(0), "Size unknown");
        assert_eq!(file_size(500), "1 KB");
        assert_eq!(file_size(412 * 1024), "412 KB");
        assert_eq!(file_size(1_468_006), "1.4 MB");
        assert_eq!(file_size(4 * 1024 * 1024), "4.0 MB");
    }
}
