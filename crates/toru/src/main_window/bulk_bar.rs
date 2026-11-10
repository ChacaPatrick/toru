//! `.bulk` (DESIGN.md §4): the dark floating bar that takes the bottom bar's place while
//! anything is selected. "N selected · x saved · y unsaved", then Copy as files as the main
//! action, Copy paths with the saved count, Save with the unsaved count, Delete and ✕, which
//! clears the selection. Merge is v2 (CLAUDE.md). The bar lies over the gallery, centred on the
//! window as the mockups place it, so a press on it stops there while the wheel still scrolls
//! the list beneath. It is wider than the
//! gallery at the 900px minimum, so at widths under about 1450px it lies over the settings
//! column's foot too; the column keeps room for it (`settings_column`).

use gpui::{Context, Hsla, Pixels, Role, div, prelude::*, px};

use super::{MainWindow, act, selection::Counts};
use crate::theme::{ActiveTheme, Theme, ThemedText, contrast};
use crate::ui::{BULK_BUTTON_HEIGHT, Button, Icon, Readable};

/// `.bulk{bottom:20px}`: the bar's distance from the bottom of the window, and so of the gallery.
pub const BOTTOM: f32 = 20.;
/// `.bulk{padding:8px 10px}`.
const PADDING_Y: f32 = 8.;
const PADDING_X: f32 = 10.;
/// `.bulk .n{opacity:.7}`.
const COUNT_OPACITY: f32 = 0.7;

/// The bar's height: its buttons inside its padding and border. With `BOTTOM`, how much of the
/// gallery's foot the bar covers, which the gallery keeps a focused control clear of.
pub fn height(theme: &Theme) -> Pixels {
    px(PADDING_Y * 2. + BULK_BUTTON_HEIGHT) + theme.borders.pop.width * 2.
}

pub fn bulk_bar(counts: &Counts, cx: &Context<MainWindow>) -> impl IntoElement {
    let theme = cx.theme();
    let c = &theme.colors;
    let pop = theme.borders.pop;
    let count_text = contrast::over(
        Hsla {
            a: COUNT_OPACITY,
            ..c.bulk_text
        },
        c.bulk_bg,
    );
    let separator = || {
        div()
            .flex_none()
            .w(px(1.))
            .h(px(24.))
            .mx(px(4.))
            .bg(c.bulk_sep)
    };
    let clear = {
        let view = cx.entity();
        move |window: &mut gpui::Window, cx: &mut gpui::App| {
            view.update(cx, |this, cx| {
                this.gallery
                    .update(cx, |gallery, cx| gallery.clear_selection(window, cx))
            })
        }
    };

    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(BOTTOM))
        .flex()
        .justify_center()
        .child(
            div()
                .id("bulk")
                .role(Role::Toolbar)
                .aria_label("Bulk actions")
                .block_mouse_except_scroll()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(PADDING_X))
                .py(px(PADDING_Y))
                .bg(c.bulk_bg)
                .text_color(c.bulk_text)
                .border(pop.width)
                .border_color(pop.color)
                .rounded(theme.radii.lg)
                .shadow(theme.shadows.pop.clone())
                .whitespace_nowrap()
                .child(
                    div()
                        .id("bulk-count")
                        .read_as_text(format!(
                            "{} selected · {} saved · {} unsaved",
                            counts.selected,
                            counts.saved,
                            counts.unsaved()
                        ))
                        .px(px(8.))
                        .type_style(theme, theme.type_scale.bulk_selected)
                        .child(format!("{} selected", counts.selected)),
                )
                .child(
                    div()
                        .pr(px(6.))
                        .type_style(theme, theme.type_scale.bulk_count)
                        .text_color(count_text)
                        .child(format!(
                            "{} saved · {} unsaved",
                            counts.saved,
                            counts.unsaved()
                        )),
                )
                .child(separator())
                .child(
                    Button::new("copy-files", "Copy as files")
                        .icon(Icon::Copy)
                        .bulk(true)
                        .on_click(act(cx, |this, cx| this.copy_selected(cx))),
                )
                .child(
                    Button::new("copy-paths", "Copy paths")
                        .icon(Icon::Paths)
                        .note(format!("({})", counts.saved))
                        .bulk(false)
                        .on_click(act(cx, |this, cx| this.copy_selected_paths(cx))),
                )
                .child(
                    Button::new("save-selected", "Save")
                        .icon(Icon::Save)
                        .note(format!("({} new)", counts.unsaved()))
                        .bulk(false)
                        .on_click(act(cx, |this, cx| this.save_selected(cx))),
                )
                .child(separator())
                .child(
                    Button::new("delete-selected", "Delete")
                        .icon(Icon::Trash)
                        .bulk(false)
                        .on_click(act(cx, |this, cx| this.delete_selected(cx))),
                )
                .child(
                    Button::glyph_only("clear-selection", "✕", "Clear selection")
                        .bulk(false)
                        .on_click(clear),
                ),
        )
}
