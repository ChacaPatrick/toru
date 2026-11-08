//! A gallery card (DESIGN.md §2): the thumbnail with its caption tag, the name field, the status
//! chip, Copy with Save or Open folder, and the `⋯` menu. The file size is in a tooltip on the
//! thumbnail and at the top of the menu. The gallery's list needs every row's height before it
//! is laid out, so `height` works a card's height out from the theme, and every card in a look
//! gets the same height whatever its status, as DESIGN.md §2 asks.
//!
//! The card is focusable itself, for the gallery's arrow keys, and shows the §7 ring when it has
//! keyboard focus. Only the gallery's current card and its controls are tab stops; a press
//! anywhere on a card makes it the current one.

use gpui::{
    AnyElement, App, Context, Div, MouseButton, ObjectFit, Pixels, Role, Stateful, Window, div,
    img, linear_color_stop, linear_gradient, prelude::*, px,
};
use toru_core::time::LocalDate;

use super::{
    act,
    gallery::{Card, Gallery},
};
use crate::theme::{ActiveTheme, Theme, ThemedText, TypeStyle};
use crate::ui::{
    BUTTON_SMALL_HEIGHT, Button, Chip, Menu, TextField, Tooltip, fill, focus_marks, shadowed,
};

const PADDING: f32 = 10.;
const GAP: f32 = 8.;
const THUMB_HEIGHT: f32 = 140.;
const NAME_HEIGHT: f32 = 34.;
/// Between the chip and the actions, and between wrapped lines of them.
const ROW_GAP: f32 = 10.;
const ACTION_GAP: f32 = 6.;
const TAG_LEFT: f32 = 8.;
const TAG_BOTTOM: f32 = 6.;

/// The card's height at `card_width`, from the theme: the actions wrap under the chip and onto
/// further lines as the mockups' larger-type looks and narrower windows need, and the height
/// allows for whichever status needs more lines, so every card in a look is the same height.
pub fn height(card_width: Pixels, theme: &Theme, window: &Window, cx: &gpui::App) -> Pixels {
    let scale = &theme.type_scale;
    let b = &theme.borders;
    let inner = card_width - px(PADDING * 2.) - b.card.width * 2.;
    let button = |text: &str, style: TypeStyle| {
        style.text_width(text, theme, window)
            + theme.sizes.button_sm_padding * 2.
            + b.button.width * 2.
    };
    let chip = |text: &str, border: Pixels| {
        scale.chip.text_width(text, theme, window) + px(9. * 2.) + border * 2.
    };
    let chip_height = scale.chip.natural_line_height(theme, cx) + px(3. * 2.) + b.accent.width * 2.;
    let button_height = px(BUTTON_SMALL_HEIGHT);

    // The chip and the buttons on one line if they fit; else the chip alone, then the buttons
    // on as many lines as they need.
    let actions_height = |chip_width: Pixels, buttons: [Pixels; 3]| {
        let all = buttons.iter().fold(px(0.), |sum, w| sum + *w) + px(ACTION_GAP * 2.);
        if chip_width + px(ROW_GAP) + all <= inner {
            return button_height;
        }
        let mut lines = 1;
        let mut line = px(0.);
        for width in buttons {
            let next = if line > px(0.) {
                line + px(ACTION_GAP) + width
            } else {
                width
            };
            if next > inner && line > px(0.) {
                lines += 1;
                line = width;
            } else {
                line = next;
            }
        }
        chip_height
            + px(ROW_GAP)
            + button_height * lines as f32
            + px(ACTION_GAP) * (lines - 1) as f32
    };
    let more = button_height;
    let unsaved = actions_height(
        chip("Unsaved", b.chip.width),
        [
            button("Copy", scale.button_sm),
            button("Save", scale.primary_button_sm),
            more,
        ],
    );
    let saved = actions_height(
        chip("Saved ✓", b.accent.width),
        [
            button("Copy", scale.button_sm),
            button("Open folder", scale.button_sm),
            more,
        ],
    );
    b.card.width * 2.
        + px(PADDING * 2. + THUMB_HEIGHT + GAP + NAME_HEIGHT + GAP)
        + unsaved.max(saved)
}

/// "1.4 MB" or "412 KB".
pub fn file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * KB;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

/// What varies between two cards of one capture: `earlier` marks a group whose session has
/// ended, `copied` shows the check mark, and `active` makes the card and its controls the
/// grid's tab stops.
#[derive(Clone, Copy)]
pub struct State {
    pub earlier: bool,
    pub copied: bool,
    pub active: bool,
}

/// `position` is the card's place in its group, which chooses among the look's thumbnail fills.
pub fn render(
    card: &Card,
    position: usize,
    today: LocalDate,
    state: State,
    window: &Window,
    cx: &Context<Gallery>,
) -> AnyElement {
    let State {
        earlier,
        copied,
        active,
    } = state;
    let focus = card.focus.clone().tab_stop(active);
    let theme = cx.theme();
    let c = &theme.colors;
    let border = theme.borders.card;
    let radius = theme.radii.lg;
    let saved = card.capture.is_saved();
    let size = file_size(card.bytes);

    let id = card.id;
    let copy = act(cx, move |this, cx| this.copy(id, cx));
    let make_current = act(cx, move |this, cx| this.set_current(id, cx));
    let save_or_open = act(cx, move |this, cx| this.save_or_open(id, cx));
    let open_editor = act(cx, move |this, cx| this.open_editor(id, cx));
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
    .item("Reveal", |_, _| {})
    .divider()
    .item("Delete", |_, _| {})
    .align_right();

    // Save and Open folder share an id, so keyboard focus stays on the button when a save finishes.
    let second = Button::new("save-or-open", if saved { "Open folder" } else { "Save" })
        .small()
        .tab_stop(active);
    let second = if saved { second } else { second.primary() };
    let open_editor_by_click = act(cx, move |this, cx| this.open_editor(id, cx));
    let actions = div()
        .flex()
        .flex_wrap()
        .justify_end()
        .gap(px(ACTION_GAP))
        .child(
            Button::new("copy", "Copy")
                .small()
                .check_mark(copied)
                .tab_stop(active)
                .on_click(copy),
        )
        .child(second.on_click(save_or_open))
        .child(menu);
    let status = div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(ROW_GAP))
        .child(Chip::new("status", if saved { "Saved ✓" } else { "Unsaved" }).saved(saved))
        .child(actions);

    let card_element = div()
        .id(("card", card.id))
        .role(Role::Group)
        .aria_label(card.name.read(cx).value())
        .track_focus(&focus)
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            make_current(window, cx)
        });
    shadowed(card_element, &theme.shadows.card, radius)
        .relative()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .p(px(PADDING))
        .bg(if earlier {
            c.earlier_card_bg
        } else {
            c.surface
        })
        .border(border.width)
        .border_color(border.color)
        .rounded(radius)
        .child(
            // A double-click on the thumbnail opens the editor (DESIGN.md §5).
            thumbnail(card, position, today, size, theme, cx).on_click(move |event, window, cx| {
                if event.click_count() == 2 {
                    open_editor_by_click(window, cx);
                }
            }),
        )
        .child(
            TextField::new(&card.name, "Screenshot name")
                .height(px(NAME_HEIGHT))
                .tab_stop(active)
                .w_full(),
        )
        .child(status)
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

/// The theme's fill with the capture's thumbnail over it, under the caption tag. A mock card, or
/// one whose file hasn't loaded, shows the fill alone.
fn thumbnail(
    card: &Card,
    position: usize,
    today: LocalDate,
    size: String,
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
    let image = card.files.as_ref().map(|files| {
        img(files.thumbnail.clone())
            .absolute()
            .inset_0()
            .size_full()
            .rounded(radius)
            .object_fit(ObjectFit::Cover)
    });

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
        .children(image)
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
    use super::*;

    #[test]
    fn file_sizes_read_like_a_file_manager() {
        assert_eq!(file_size(500), "1 KB");
        assert_eq!(file_size(412 * 1024), "412 KB");
        assert_eq!(file_size(1_468_006), "1.4 MB");
        assert_eq!(file_size(4 * 1024 * 1024), "4.0 MB");
    }
}
