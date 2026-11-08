//! `.jump`: the return pill that brings the gallery back to "This session". It wears the toast's
//! colours and border on a pill shape, with the up arrow before the look's `jump` string. It lies
//! over the gallery, so a press on it must not reach the card beneath (`block_mouse_except_scroll`
//! stops that and still lets the wheel scroll the list), and assistive technology's click goes
//! through `on_a11y_action` rather than a simulated press (`docs/spikes.md` §1.5).

use gpui::{AccessibleAction, App, ElementId, Role, SharedString, Window, div, prelude::*, px};

use super::{Handler, Icon, focus_marks, keyed_focus};
use crate::theme::{ActiveTheme, ThemedText, contrast, hover_tint};

pub const HEIGHT: f32 = 36.;

#[derive(IntoElement)]
pub struct Jump {
    id: ElementId,
    label: SharedString,
    on_click: Option<Handler>,
}

impl Jump {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Jump {
        Jump {
            id: id.into(),
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Jump {
        self.on_click = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for Jump {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = keyed_focus(window, cx, &self.id);
        let theme = cx.theme();
        let c = &theme.colors;
        let border = theme.borders.toast;
        let radius = theme.radii.pill;
        let font = theme.type_scale.jump;
        div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(self.label.clone())
            .track_focus(&focus)
            .block_mouse_except_scroll()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(HEIGHT))
            .px(px(14.))
            .bg(c.toast_bg)
            .text_color(c.toast_text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .shadow(theme.shadows.pop.clone())
            .type_style(theme, font)
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(|style| style.bg(contrast::over(hover_tint(c.toast_text), c.toast_bg)))
            .when_some(self.on_click, |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |_, window, cx| handler(window, cx))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        for_a11y(window, cx)
                    })
            })
            .child(Icon::ArrowUp.render(theme, c.toast_text))
            .child(font.case.apply(&self.label))
            .children(focus_marks(
                focus.is_focused(window),
                theme,
                radius,
                border.width,
                window,
                cx,
            ))
    }
}
