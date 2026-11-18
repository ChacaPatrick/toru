//! `.check`: drawn inside a `MIN_TARGET` box (DESIGN.md §7), which takes the press and the focus.

use std::rc::Rc;

use gpui::{
    AccessibleAction, App, ElementId, FocusHandle, Modifiers, Role, SharedString, Toggled, Window,
    div, prelude::*, px,
};

use super::{Icon, MIN_TARGET, RingFocus, focus_marks, keyed_focus};
use crate::theme::ActiveTheme;

pub const SIZE: f32 = 26.;

type PressHandler = Rc<dyn Fn(Modifiers, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    on: bool,
    tab_stop: bool,
    focus: Option<FocusHandle>,
    on_press: Option<PressHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Checkbox {
        Checkbox {
            id: id.into(),
            label: label.into(),
            on,
            tab_stop: true,
            focus: None,
            on_press: None,
        }
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Checkbox {
        self.tab_stop = tab_stop;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Checkbox {
        self.focus = Some(focus.clone());
        self
    }

    /// Gets the modifiers, since a Shift-click selects a range. Assistive technology's toggle
    /// comes with none.
    pub fn on_press(
        mut self,
        handler: impl Fn(Modifiers, &mut Window, &mut App) + 'static,
    ) -> Checkbox {
        self.on_press = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self
            .focus
            .unwrap_or_else(|| keyed_focus(window, cx, &self.id))
            .tab_stop(self.tab_stop);
        let theme = cx.theme();
        let c = &theme.colors;
        let (bg, border) = if self.on {
            (c.accent_bg, theme.borders.accent)
        } else {
            (c.surface, theme.borders.input)
        };
        let radius = theme.radii.sm;
        let toggled = if self.on {
            Toggled::True
        } else {
            Toggled::False
        };

        let boxed = div()
            .relative()
            .size(px(SIZE))
            .flex()
            .items_center()
            .justify_center()
            .bg(bg)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .when(self.on, |this| {
                this.child(Icon::Check.render(theme, c.accent_text))
            })
            .children(focus_marks(
                focus.is_focused(window),
                theme,
                radius,
                border.width,
                window,
                cx,
            ));
        div()
            .id(self.id)
            .role(Role::CheckBox)
            .aria_label(self.label)
            .aria_toggled(toggled)
            .track_ring_focus(&focus)
            .flex_none()
            .size(px(MIN_TARGET))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when_some(self.on_press, |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |event, window, cx| {
                    // Not the card's press too.
                    cx.stop_propagation();
                    handler(event.modifiers(), window, cx)
                })
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    for_a11y(Modifiers::default(), window, cx)
                })
            })
            .child(boxed)
    }
}
