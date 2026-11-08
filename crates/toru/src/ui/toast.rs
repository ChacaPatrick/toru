//! `.toast`: a message above the bottom bar, read out by screen readers as a polite live region.
//! It appears and disappears without motion, so a reduced-motion setting has nothing to turn off.

use gpui::{App, Role, SharedString, Window, accesskit::Live, div, prelude::*, px};

use super::Icon;
use crate::theme::{ActiveTheme, ThemedText};

#[derive(IntoElement, Clone)]
pub struct Toast {
    message: SharedString,
    /// Something went wrong, so the message stands alone without the check mark.
    failed: bool,
}

impl Toast {
    pub fn new(message: impl Into<SharedString>) -> Toast {
        Toast {
            message: message.into(),
            failed: false,
        }
    }

    pub fn failed(message: impl Into<SharedString>) -> Toast {
        Toast {
            message: message.into(),
            failed: true,
        }
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let border = theme.borders.toast;
        let font = theme.type_scale.toast;
        // The row spans the window so the toast can sit at its centre; it takes no clicks. The
        // toast itself lies over the gallery, so a press on it stops there rather than reaching
        // a card beneath, while the wheel still scrolls the list.
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(84.))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("toast")
                    .block_mouse_except_scroll()
                    .role(Role::Status)
                    .aria_label(self.message.clone())
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(Live::Polite);
                    })
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(40.))
                    .px_4()
                    .bg(c.toast_bg)
                    .text_color(c.toast_text)
                    .border(border.width)
                    .border_color(border.color)
                    .rounded(theme.radii.md)
                    .shadow(theme.shadows.pop.clone())
                    .type_style(theme, font)
                    .whitespace_nowrap()
                    .when(!self.failed, |toast| {
                        toast.child(Icon::Check.render(theme, c.toast_text))
                    })
                    .child(font.case.apply(&self.message)),
            )
    }
}
