//! The status chip on a card: `Unsaved`, or `Saved ✓` in the accent. Status is always text, never
//! colour alone, so the chip's words carry the state.

use gpui::{App, ElementId, SharedString, Window, div, prelude::*, px};

use super::Readable;
use crate::theme::{ActiveTheme, ThemedText};

#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    text: SharedString,
    saved: bool,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Chip {
        Chip {
            id: id.into(),
            text: text.into(),
            saved: false,
        }
    }

    pub fn saved(mut self, saved: bool) -> Chip {
        self.saved = saved;
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let (bg, text, border) = if self.saved {
            (c.chip_saved_bg, c.chip_saved_text, theme.borders.accent)
        } else {
            (c.chip_bg, c.chip_text, theme.borders.chip)
        };
        let style = theme.type_scale.chip;
        // The mockups leave the line height at `normal`, so the chip is as tall as its text.
        let line_height = style.natural_line_height(theme, cx);
        div()
            .id(self.id)
            // The check mark is decoration; the word says it.
            .read_as_text(self.text.trim_end_matches('✓').trim_end())
            .flex_none()
            .py(px(3.))
            .px(px(9.))
            .bg(bg)
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(theme.radii.pill)
            .type_style(theme, style)
            .line_height(line_height)
            .whitespace_nowrap()
            .child(style.case.apply(&self.text))
    }
}
