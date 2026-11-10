//! The status chip on a card: `Unsaved`, or `Saved ✓` in the accent. Status is always text, never
//! colour alone, so the chip's words carry the state. A check mark at the end of the text is
//! drawn as the look's own check icon, as Copy's is: only Japandi's body face has the glyph, and
//! the one a fallback font lends the other looks suits neither.

use gpui::{App, ElementId, Pixels, SharedString, Window, div, prelude::*, px};

use super::{Icon, Readable};
use crate::theme::{ActiveTheme, Theme, ThemedText};

const PADDING_X: f32 = 9.;
/// The check mark a chip's text may end with, and the gap before the icon drawn for it. The
/// icon has air inside its own box, so with this it sits a space from the words and the chip
/// is as wide as the glyph made it.
const CHECK: &str = " ✓";
const CHECK_GAP: f32 = 1.;

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

    /// The words, and whether a check mark follows them.
    fn words(&self) -> (&str, bool) {
        match self.text.strip_suffix(CHECK) {
            Some(words) => (words, true),
            None => (&self.text, false),
        }
    }

    /// How wide the chip will be laid out, for a row that must know before it is.
    pub fn width(&self, theme: &Theme, window: &Window) -> Pixels {
        let border = if self.saved {
            theme.borders.accent
        } else {
            theme.borders.chip
        };
        let style = theme.type_scale.chip;
        let (words, check) = self.words();
        let check = if check {
            px(CHECK_GAP) + style.size
        } else {
            px(0.)
        };
        style.text_width(words, theme, window) + check + (px(PADDING_X) + border.width) * 2.
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
        let (words, check) = self.words();
        let words = style.case.apply(words);
        div()
            .id(self.id)
            // The check mark is decoration; the word says it.
            .read_as_text(self.text.trim_end_matches('✓').trim_end())
            .flex_none()
            .py(px(3.))
            .px(px(PADDING_X))
            .bg(bg)
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(theme.radii.pill)
            .type_style(theme, style)
            .line_height(line_height)
            .whitespace_nowrap()
            .flex()
            .items_center()
            .gap(px(CHECK_GAP))
            .child(words)
            .when(check, |chip| {
                chip.child(Icon::Check.render_at(theme, text, style.size))
            })
    }
}
