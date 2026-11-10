//! `.bulk` (DESIGN.md §4): the dark floating bar that takes the bottom bar's place while
//! anything is selected. "N selected · x saved · y unsaved", then Copy as files as the main
//! action, Copy paths with the saved count, Save with the unsaved count, Delete and ✕, which
//! clears the selection. Merge is v2 (CLAUDE.md). The bar lies over the gallery's foot, centred
//! on the gallery and never over the settings column, so a press on it stops there while the
//! wheel still scrolls the list beneath.
//!
//! The mockups' bar is wider than the gallery of a narrow window, so it gives up what it can
//! do without as the gallery narrows: first the saved and unsaved counts, which Copy paths and
//! Save repeat, then the buttons' labels, leaving each icon with its count, its name for
//! assistive technology and a tooltip.

use gpui::{Context, Hsla, Pixels, Role, Window, div, prelude::*, px};

use super::{MainWindow, act, selection::Counts};
use crate::theme::{ActiveTheme, Theme, ThemedText, contrast};
use crate::ui::{BULK_BUTTON_HEIGHT, Button, Icon, Readable};

/// `.bulk{bottom:20px}`: the bar's distance from the bottom of the window, and so of the gallery.
pub const BOTTOM: f32 = 20.;
/// `.bulk{padding:8px 10px}`.
const PADDING_Y: f32 = 8.;
const PADDING_X: f32 = 10.;
const GAP: f32 = 6.;
/// The least air between the bar and the gallery's edges.
const MARGIN: f32 = 16.;
/// Around "N selected", and after the counts.
const SELECTED_PADDING: f32 = 8.;
const COUNTS_PADDING: f32 = 6.;
/// A separator's line and the margin on each side of it.
const SEPARATOR: f32 = 1.;
const SEPARATOR_MARGIN: f32 = 4.;
/// `.bulk .n{opacity:.7}`.
const COUNT_OPACITY: f32 = 0.7;

/// The bar's height: its buttons inside its padding and border. With `BOTTOM`, how much of the
/// gallery's foot the bar covers, which the gallery keeps a focused control clear of.
pub fn height(theme: &Theme) -> Pixels {
    px(PADDING_Y * 2. + BULK_BUTTON_HEIGHT) + theme.borders.pop.width * 2.
}

/// How much the bar shows, by the room it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Detail {
    Full,
    /// Without "x saved · y unsaved".
    NoCounts,
    /// Without the counts or the buttons' labels.
    Icons,
}

/// The most detail whose width fits `room`; `Icons` if none does.
fn detail_for(room: Pixels, width_of: impl Fn(Detail) -> Pixels) -> Detail {
    [Detail::Full, Detail::NoCounts]
        .into_iter()
        .find(|detail| width_of(*detail) <= room)
        .unwrap_or(Detail::Icons)
}

/// The four actions, in the bar's order, as `detail` shows them; Delete is the last.
fn actions(counts: &Counts, detail: Detail) -> [Button; 4] {
    let compact = detail == Detail::Icons;
    [
        Button::new("copy-files", "Copy as files")
            .icon(Icon::Copy)
            .bulk(true),
        Button::new("copy-paths", "Copy paths")
            .icon(Icon::Paths)
            .note(format!("({})", counts.saved))
            .bulk(false),
        Button::new("save-selected", "Save")
            .icon(Icon::Save)
            .note(format!("({} new)", counts.unsaved()))
            .bulk(false),
        Button::new("delete-selected", "Delete")
            .icon(Icon::Trash)
            .bulk(false),
    ]
    .map(|button| button.compact(compact))
}

fn selected_text(counts: &Counts) -> String {
    format!("{} selected", counts.selected)
}

fn counts_text(counts: &Counts) -> String {
    format!("{} saved · {} unsaved", counts.saved, counts.unsaved())
}

/// How wide the bar is laid out at `detail`.
fn width(counts: &Counts, detail: Detail, theme: &Theme, window: &Window) -> Pixels {
    let scale = &theme.type_scale;
    let selected = scale
        .bulk_selected
        .text_width(&selected_text(counts), theme, window)
        + px(SELECTED_PADDING * 2.);
    let counts_width = (detail == Detail::Full).then(|| {
        scale
            .bulk_count
            .text_width(&counts_text(counts), theme, window)
            + px(COUNTS_PADDING)
    });
    let buttons = actions(counts, detail)
        .iter()
        .fold(px(BULK_BUTTON_HEIGHT), |sum, button| {
            sum + button.width(theme, window)
        });
    // "N selected", two separators, four actions and ✕, and the counts if shown.
    let items = 8 + usize::from(counts_width.is_some());
    selected
        + counts_width.unwrap_or_default()
        + px((SEPARATOR + SEPARATOR_MARGIN * 2.) * 2.)
        + buttons
        + px(GAP) * (items - 1) as f32
        + (px(PADDING_X) + theme.borders.pop.width) * 2.
}

/// `gallery_width` is the gallery's, at the window's left, which the bar is centred on.
pub fn bulk_bar(
    counts: &Counts,
    gallery_width: Pixels,
    window: &Window,
    cx: &Context<MainWindow>,
) -> impl IntoElement {
    let theme = cx.theme();
    let c = &theme.colors;
    let pop = theme.borders.pop;
    let detail = detail_for(gallery_width - px(MARGIN * 2.), |detail| {
        width(counts, detail, theme, window)
    });
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
            .w(px(SEPARATOR))
            .h(px(24.))
            .mx(px(SEPARATOR_MARGIN))
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
    let [copy_files, copy_paths, save, delete] = actions(counts, detail);

    div()
        .absolute()
        .left_0()
        .w(gallery_width)
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
                .gap(px(GAP))
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
                        // Read whole even when the bar has no room to show the counts.
                        .read_as_text(format!(
                            "{} selected · {} saved · {} unsaved",
                            counts.selected,
                            counts.saved,
                            counts.unsaved()
                        ))
                        .px(px(SELECTED_PADDING))
                        .type_style(theme, theme.type_scale.bulk_selected)
                        .child(selected_text(counts)),
                )
                .when(detail == Detail::Full, |bar| {
                    bar.child(
                        div()
                            .pr(px(COUNTS_PADDING))
                            .type_style(theme, theme.type_scale.bulk_count)
                            .text_color(count_text)
                            .child(counts_text(counts)),
                    )
                })
                .child(separator())
                .child(copy_files.on_click(act(cx, |this, cx| this.copy_selected(cx))))
                .child(copy_paths.on_click(act(cx, |this, cx| this.copy_selected_paths(cx))))
                .child(save.on_click(act(cx, |this, cx| this.save_selected(cx))))
                .child(separator())
                .child(delete.on_click(act(cx, |this, cx| this.delete_selected(cx))))
                .child(
                    Button::glyph_only("clear-selection", "✕", "Clear selection")
                        .bulk(false)
                        .on_click(clear),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_gives_up_the_counts_and_then_the_labels() {
        let width_of = |detail| match detail {
            Detail::Full => px(836.),
            Detail::NoCounts => px(720.),
            Detail::Icons => px(450.),
        };
        assert_eq!(detail_for(px(948.), width_of), Detail::Full);
        assert_eq!(detail_for(px(836.), width_of), Detail::Full);
        assert_eq!(detail_for(px(835.), width_of), Detail::NoCounts);
        assert_eq!(detail_for(px(719.), width_of), Detail::Icons);
        // The narrowest gallery still gets the bar, as icons.
        assert_eq!(detail_for(px(300.), width_of), Detail::Icons);
    }
}
