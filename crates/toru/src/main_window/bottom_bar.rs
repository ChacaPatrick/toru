//! `.bottom` (DESIGN.md §2): this session's counts, then Clear unsaved, Copy all and Save all,
//! which skips what is already saved. Select mode's bulk bar (4.5) takes this bar's place while
//! anything is selected.

use gpui::{Context, Div, div, prelude::*, px};

use super::{MainWindow, act, gallery::plural};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::Button;

pub fn bottom_bar(this: &MainWindow, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let (count, saved) = this.gallery.read(cx).session_counts();
    let unsaved = count - saved;
    let summary = format!(
        "{} · {saved} saved · {unsaved} unsaved — Save all skips anything already saved.",
        plural(count, "capture")
    );
    div()
        .flex_none()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .px(px(18.))
        .py(px(12.))
        .bg(c.surface_2)
        .border_t(hair.width)
        .border_color(hair.color)
        .child(
            div()
                .min_w_0()
                .type_style(theme, theme.type_scale.hint)
                .text_color(c.muted)
                .child(summary),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap(px(8.))
                .child(
                    Button::new("clear-unsaved", "Clear unsaved")
                        .on_click(act(cx, |this, cx| this.clear_unsaved(cx))),
                )
                .child(
                    Button::new("copy-all", "Copy all")
                        .on_click(act(cx, |this, cx| this.copy_all(cx))),
                )
                .child(
                    Button::new("save-all", format!("Save all ({unsaved} new)"))
                        .primary()
                        .on_click(act(cx, |this, cx| this.save_all(cx))),
                ),
        )
}
