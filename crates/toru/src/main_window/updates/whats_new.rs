use gpui::{
    AnyElement, Bounds, Context, ScrollHandle, canvas, div, fill, point, prelude::*, px, size,
};
use gpui_kit::component::scroll::ScrollableElement;
use toru_core::update::{NoteLine, note_lines};

use super::NOTICE_HINT;
use crate::main_window::{HINT_LEADING, MainWindow, act};
use crate::theme::{ActiveTheme, Theme, ThemedText};
use crate::ui::{Dialog, Readable};

const NOTES_MAX_HEIGHT: f32 = 360.;

const NOTE_GAP: f32 = 6.;

const PART_GAP: f32 = 10.;

const BULLET_SIZE: f32 = 5.;

const BULLET_GAP: f32 = 10.;

const SCROLLBAR_ROOM: f32 = 12.;

const LINE_SCROLL: f32 = 40.;

impl MainWindow {
    pub(super) fn open_whats_new(&mut self, cx: &mut Context<Self>) {
        self.updates.whats_new_open = true;
        self.updates.notes_scroll.set_offset(point(px(0.), px(0.)));
        self.focus_dialog(cx);
        cx.notify();
    }

    fn close_whats_new(&mut self, cx: &mut Context<Self>) {
        self.updates.whats_new_open = false;
        cx.notify();
    }

    /// Esc only closes it. The arrow and page keys scroll the notes wherever focus is, since the
    /// buttons have no use for them.
    pub(in crate::main_window) fn whats_new_dialog(&self, cx: &Context<Self>) -> Option<Dialog> {
        let update = self
            .updates
            .ready
            .as_ref()
            .filter(|_| self.updates.whats_new_open)?;
        let lines = note_lines(&update.notes);
        let dialog = Dialog::new(
            "whats-new",
            format!("Toru {}", update.version),
            if lines.is_empty() {
                NOTICE_HINT.to_string()
            } else {
                format!("Here's what's new. {NOTICE_HINT}")
            },
            &self.dialog_focus,
            act(cx, |this, cx| this.close_whats_new(cx)),
        )
        .primary_action("Update", act(cx, |this, cx| this.install_update(cx)))
        .action("Later", act(cx, |this, cx| this.dismiss_update(cx)));
        if lines.is_empty() {
            return Some(dialog);
        }
        let scroll = self.updates.notes_scroll.clone();
        Some(
            dialog
                .content(notes(cx.theme(), &lines, &scroll), Vec::new())
                .on_key(move |event, window, _| {
                    let Some(to) = scroll_target(
                        &event.keystroke.key,
                        scroll.offset().y.into(),
                        scroll.max_offset().y.into(),
                        scroll.bounds().size.height.into(),
                    ) else {
                        return false;
                    };
                    scroll.set_offset(point(px(0.), px(to)));
                    window.refresh();
                    true
                }),
        )
    }
}

/// Offsets run from zero at the top to `-max` at the bottom.
fn scroll_target(key: &str, offset: f32, max: f32, page: f32) -> Option<f32> {
    let by = match key {
        "up" => LINE_SCROLL,
        "down" => -LINE_SCROLL,
        "pageup" => page - LINE_SCROLL,
        "pagedown" => LINE_SCROLL - page,
        "home" => f32::INFINITY,
        "end" => f32::NEG_INFINITY,
        _ => return None,
    };
    Some((offset + by).clamp(-max.max(0.), 0.))
}

fn notes(theme: &Theme, lines: &[NoteLine], scroll: &ScrollHandle) -> impl IntoElement {
    let c = &theme.colors;
    let body = theme.type_scale.body;
    let label = theme.type_scale.label;
    let leading = body.size * HINT_LEADING;
    let rows = lines.iter().enumerate().map(|(index, line)| -> AnyElement {
        // A new part of the notes: a heading, or plain text once a list has ended.
        let parts = match line {
            NoteLine::Heading(_) => index > 0,
            NoteLine::Text(_) => index > 0 && matches!(lines[index - 1], NoteLine::Bullet(_)),
            NoteLine::Bullet(_) => false,
        };
        let row = div()
            .id(("note", index))
            .when(parts, |row| row.mt(px(PART_GAP)));
        match *line {
            NoteLine::Heading(text) => row
                .read_as_heading(text)
                .type_style(theme, label)
                .text_color(c.label)
                .child(label.case.apply(text))
                .into_any_element(),
            NoteLine::Bullet(text) => row
                .read_as_text(text)
                .flex()
                .gap(px(BULLET_GAP))
                // As tall as a line, so the dot sits beside the first one.
                .child(
                    div().flex_none().h(leading).flex().items_center().child(
                        div()
                            .size(px(BULLET_SIZE))
                            .rounded(theme.radii.dot)
                            .bg(c.label),
                    ),
                )
                .child(div().flex_1().min_w_0().child(text.to_string()))
                .into_any_element(),
            NoteLine::Text(text) => row
                .read_as_text(text)
                .child(text.to_string())
                .into_any_element(),
        }
    });
    // A rule across the top or bottom edge while there are notes past it, since the scrollbar
    // only shows while it is used. Painted after the list is laid out, so it is right from the
    // first frame.
    let hair = theme.borders.hair;
    let more = canvas(|_, _, _| (), {
        let scroll = scroll.clone();
        move |bounds, (), window, _| {
            let rule = size(bounds.size.width, hair.width);
            // Less than a pixel is the rounding of lines that fit, not notes out of view.
            let (offset, hidden) = (scroll.offset().y, px(1.));
            if offset < -hidden {
                window.paint_quad(fill(Bounds::new(bounds.origin, rule), hair.color));
            }
            if offset > hidden - scroll.max_offset().y {
                let origin = point(bounds.left(), bounds.bottom() - hair.width);
                window.paint_quad(fill(Bounds::new(origin, rule), hair.color));
            }
        }
    })
    .absolute()
    .inset_0();
    // In a short window the notes give way and scroll, and the actions stay in view.
    div()
        .relative()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .id("whats-new-notes")
                .min_h_0()
                .max_h(px(NOTES_MAX_HEIGHT))
                .overflow_y_scroll()
                .track_scroll(scroll)
                .flex()
                .flex_col()
                .gap(px(NOTE_GAP))
                .pr(px(SCROLLBAR_ROOM))
                .type_style(theme, body)
                .line_height(leading)
                .children(rows),
        )
        .child(more)
        .vertical_scrollbar(scroll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_scroll_the_notes_within_their_ends() {
        let (max, page) = (300., 280.);
        assert_eq!(scroll_target("down", 0., max, page), Some(-40.));
        assert_eq!(scroll_target("up", 0., max, page), Some(0.));
        assert_eq!(scroll_target("pagedown", -40., max, page), Some(-280.));
        assert_eq!(scroll_target("pagedown", -280., max, page), Some(-300.));
        assert_eq!(scroll_target("pageup", -300., max, page), Some(-60.));
        assert_eq!(scroll_target("end", 0., max, page), Some(-300.));
        assert_eq!(scroll_target("home", -300., max, page), Some(0.));
        // Notes that fit don't move.
        assert_eq!(scroll_target("down", 0., 0., page), Some(0.));
        assert_eq!(scroll_target("enter", 0., max, page), None);
        assert_eq!(scroll_target("tab", 0., max, page), None);
    }
}
