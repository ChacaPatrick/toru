//! `gpui-kit`'s text input reads its colours and fonts from the kit's own theme global, which its
//! element styling can't reach (the caret, selection and placeholder live inside the text element).
//! So the active `Theme` is copied into the parts of it the input uses.

use gpui::{App, Hsla};

use super::Theme;

pub fn sync_kit_theme(theme: &Theme, cx: &mut App) {
    let c = &theme.colors;
    let kit = gpui_kit::component::Theme::global_mut(cx);
    kit.colors.foreground = c.text;
    kit.colors.caret = c.text;
    kit.colors.selection = Hsla { a: 0.3, ..c.accent };
    kit.colors.muted_foreground = c.muted;
    kit.colors.background = c.input_bg;
    kit.colors.input = theme.borders.input.color;
    kit.colors.border = c.hair;
    kit.colors.ring = c.focus;
    kit.radius = theme.radii.md;
    kit.font_family = theme.fonts.body.clone();
    kit.mono_font_family = theme.fonts.mono.clone();
}
