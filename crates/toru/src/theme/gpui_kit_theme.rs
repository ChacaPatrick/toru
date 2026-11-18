//! `gpui-kit`'s text input and scrollbar read the kit's own theme global, which element styling
//! can't reach (caret, selection, placeholder, scrollbar), so the active `Theme` is copied into it.

use gpui::{App, Hsla};

use super::{Theme, selection_tint};

pub fn sync_kit_theme(theme: &Theme, cx: &mut App) {
    let c = &theme.colors;
    // `update`, not `global_mut`: it pushes the colours down to the layer that paints scrollbars.
    gpui_kit::component::Theme::update(cx, |kit| {
        kit.colors.foreground = c.text;
        kit.colors.caret = c.text;
        kit.colors.selection = selection_tint(c.accent);
        kit.colors.muted_foreground = c.muted;
        kit.colors.background = c.input_bg;
        kit.colors.input = theme.borders.input.color;
        kit.colors.border = c.hair;
        kit.colors.ring = c.focus;
        // The mockups show no scrollbar, so it is kept faint.
        kit.colors.scrollbar = gpui::transparent_black();
        kit.colors.scrollbar_thumb = Hsla { a: 0.5, ..c.muted };
        kit.colors.scrollbar_thumb_hover = c.muted;
        kit.radius = theme.radii.md;
        kit.font_family = theme.fonts.body.clone();
        kit.mono_font_family = theme.fonts.mono.clone();
    });
}
