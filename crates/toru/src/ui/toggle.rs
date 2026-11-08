//! `.toggle`: a 44×24 switch whose knob sits at the left when off and the right when on.

use std::rc::Rc;

use gpui::{
    AccessibleAction, App, ElementId, Role, SharedString, Toggled, Window, div, prelude::*, px,
};

use super::{focus_marks, keyed_focus};
use crate::theme::ActiveTheme;

type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    label: SharedString,
    on: bool,
    on_change: Option<ChangeHandler>,
}

impl Toggle {
    /// `label` is the setting's name, read out with the on/off state.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Toggle {
        Toggle {
            id: id.into(),
            label: label.into(),
            on,
            on_change: None,
        }
    }

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Toggle {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = keyed_focus(window, cx, &self.id);
        let theme = cx.theme();
        let c = &theme.colors;
        let (bg, knob, knob_left) = if self.on {
            (c.toggle_on, c.toggle_on_knob, px(23.))
        } else {
            (c.toggle_off, c.toggle_knob, px(3.))
        };
        let border = theme.borders.toggle;
        let radius = theme.radii.pill;
        let on = self.on;
        let toggled = if on { Toggled::True } else { Toggled::False };

        div()
            .id(self.id)
            .role(Role::Switch)
            .aria_label(self.label)
            .aria_toggled(toggled)
            .track_focus(&focus)
            .relative()
            .flex_none()
            .w(px(44.))
            .h(px(24.))
            .bg(bg)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .cursor_pointer()
            .when_some(self.on_change, |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |_, window, cx| handler(!on, window, cx))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        for_a11y(!on, window, cx)
                    })
            })
            .child(
                div()
                    .absolute()
                    .top(px(3.))
                    .left(knob_left)
                    .size(px(14.))
                    .rounded(radius)
                    .bg(knob),
            )
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
