//! A tooltip: one line of hint text on a small pop surface, for a value with no room on the
//! control, such as a card's file size. The mockups show none, so it borrows the menu's surface.

use gpui::{AnyView, App, Context, Render, SharedString, Window, div, prelude::*};

use crate::theme::{ActiveTheme, ThemedText};

pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// What an element's `.tooltip(...)` takes.
    pub fn build(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_, cx| cx.new(|_| Tooltip { text: text.clone() }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let pop = theme.borders.pop;
        div()
            .px_2()
            .py_1()
            .bg(c.surface)
            .text_color(c.text)
            .border(pop.width)
            .border_color(pop.color)
            .rounded(theme.radii.sm)
            .shadow(theme.shadows.pop.clone())
            .type_style(theme, theme.type_scale.hint)
            .whitespace_nowrap()
            .child(self.text.clone())
    }
}
