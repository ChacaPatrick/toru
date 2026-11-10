//! `.bottom` (DESIGN.md §2): this session's counts, then Clear unsaved, Copy all and Save all,
//! which skips what is already saved. Select mode's bulk bar (4.5) takes this bar's place while
//! anything is selected.
//!
//! The bar is about this session only, whatever history is listed above it, so the summary
//! starts by saying so, and each button is disabled while this session gives it nothing to do.
//! The bar is one row at every window width: the note about Save all is left off when it has
//! no room.

use gpui::{Context, Div, Window, div, prelude::*, px};

use super::{MainWindow, act, gallery::plural};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Button, Readable};

const PADDING_X: f32 = 18.;
/// Between the summary and the buttons.
const GAP: f32 = 12.;
const BUTTON_GAP: f32 = 8.;
const SAVE_ALL_NOTE: &str = " — Save all skips anything already saved.";

/// "This session: 3 captures · 1 saved · 2 unsaved", or that there is nothing yet.
fn summary(count: usize, saved: usize) -> String {
    if count == 0 {
        return "This session: no captures yet".to_string();
    }
    format!(
        "This session: {} · {saved} saved · {} unsaved",
        plural(count, "capture"),
        count - saved
    )
}

pub fn bottom_bar(this: &MainWindow, window: &Window, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let hint = theme.type_scale.hint;
    let (count, saved) = this.gallery.read(cx).session_counts();
    let unsaved = count - saved;

    let clear = Button::new("clear-unsaved", "Clear unsaved").disabled(unsaved == 0);
    let copy = Button::new("copy-all", "Copy all").disabled(count == 0);
    let save = Button::new("save-all", format!("Save all ({unsaved} new)"))
        .primary()
        .disabled(unsaved == 0);

    let buttons = [&clear, &copy, &save]
        .iter()
        .fold(px(BUTTON_GAP * 2.), |sum, button| {
            sum + button.width(theme, window)
        });
    let room = window.viewport_size().width - px(PADDING_X * 2. + GAP) - buttons;
    let mut summary = summary(count, saved);
    // The note explains Save all, so it shows only while there is something to save and room.
    let with_note = format!("{summary}{SAVE_ALL_NOTE}");
    if unsaved > 0 && hint.text_width(&with_note, theme, window) <= room {
        summary = with_note;
    }

    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(GAP))
        .px(px(PADDING_X))
        .py(px(12.))
        .bg(c.surface_2)
        .border_t(hair.width)
        .border_color(hair.color)
        .child(
            div()
                .id("session-summary")
                .read_as_text(&summary)
                .flex_1()
                .min_w_0()
                .type_style(theme, hint)
                .text_color(c.muted)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(summary),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap(px(BUTTON_GAP))
                .child(clear.on_click(act(cx, |this, cx| this.clear_unsaved(cx))))
                .child(copy.on_click(act(cx, |this, cx| this.copy_all(cx))))
                .child(save.on_click(act(cx, |this, cx| this.save_all(cx)))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_says_it_is_about_this_session() {
        assert_eq!(summary(0, 0), "This session: no captures yet");
        assert_eq!(
            summary(1, 0),
            "This session: 1 capture · 0 saved · 1 unsaved"
        );
        assert_eq!(
            summary(3, 1),
            "This session: 3 captures · 1 saved · 2 unsaved"
        );
    }
}
