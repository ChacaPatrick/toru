//! `gpui-kit`'s text input and scrollbar read their colours and fonts from the kit's own theme
//! global, which their element styling can't reach (the caret, selection and placeholder live
//! inside the text element; the scrollbar is painted by the kit's base layer). So the active
//! `Theme` is copied into the parts of it they use.

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
        // The mockups show no scrollbar, so the gallery's has no track and a thumb in the muted
        // colour, at full strength when hovered.
        kit.colors.scrollbar = gpui::transparent_black();
        kit.colors.scrollbar_thumb = Hsla { a: 0.5, ..c.muted };
        kit.colors.scrollbar_thumb_hover = c.muted;
        kit.radius = theme.radii.md;
        kit.font_family = theme.fonts.body.clone();
        kit.mono_font_family = theme.fonts.mono.clone();
    });
}
