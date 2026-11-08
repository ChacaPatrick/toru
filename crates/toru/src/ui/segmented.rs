//! `.seg`: one choice among a few, such as the save format. It is one tab stop, like a radio
//! group: focus sits on the chosen segment, the arrow keys move the choice and focus together, and
//! each segment can also be clicked.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, App, ElementId, FocusHandle, KeyDownEvent, Role, SharedString, Toggled,
    Window, div, prelude::*, px,
};

use super::{focus_marks, keyed_focus};
use crate::theme::{ActiveTheme, ThemedText};

type ChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    label: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    on_change: Option<ChangeHandler>,
}

impl Segmented {
    /// `label` names the group, such as "Image format"; `selected` indexes `options`.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Segmented {
        Segmented {
            id: id.into(),
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            on_change: None,
        }
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Segmented {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.options.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let on_change = self.on_change;
        let id = Arc::new(self.id.clone());
        let segment_id = |option: &SharedString| ElementId::NamedChild(id.clone(), option.clone());

        // Only the chosen segment is in the tab order, so the group stays one tab stop.
        let focus: Vec<FocusHandle> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, option)| {
                keyed_focus(window, cx, &segment_id(option)).tab_stop(index == selected)
            })
            .collect();
        let theme = cx.theme();
        let c = &theme.colors;
        let border = theme.borders.input;
        let radius = theme.radii.md;
        let inner_radius = radius - border.width;
        let font = theme.type_scale.segment;
        let focused = focus.iter().any(|focus| focus.is_focused(window));

        let on_key_down = {
            let on_change = on_change.clone();
            let focus = focus.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let next = match event.keystroke.key.as_str() {
                    "left" | "up" => (selected + count - 1) % count,
                    "right" | "down" => (selected + 1) % count,
                    "home" => 0,
                    "end" => count - 1,
                    _ => return,
                };
                cx.stop_propagation();
                focus[next].focus(window, cx);
                if let Some(handler) = &on_change {
                    handler(next, window, cx);
                }
            }
        };

        let segments = self
            .options
            .iter()
            .zip(&focus)
            .enumerate()
            .map(|(index, (option, focus))| {
                let on = index == selected;
                let (bg, text) = if on {
                    (Some(c.on_bg), c.on_text)
                } else {
                    (None, c.text)
                };
                let choose = on_change.clone();
                let choose_a11y = on_change.clone();
                div()
                    .id(segment_id(option))
                    .role(Role::RadioButton)
                    .aria_label(option.clone())
                    .aria_toggled(if on { Toggled::True } else { Toggled::False })
                    .track_focus(focus)
                    .flex_1()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(index == 0, |this| this.rounded_l(inner_radius))
                    .when(index + 1 == count, |this| this.rounded_r(inner_radius))
                    .when_some(bg, |this, bg| this.bg(bg))
                    .text_color(text)
                    .type_style(theme, font)
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        if let Some(handler) = &choose {
                            handler(index, window, cx);
                        }
                    })
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        if let Some(handler) = &choose_a11y {
                            handler(index, window, cx);
                        }
                    })
                    .child(font.case.apply(option))
            })
            .collect::<Vec<_>>();

        div()
            .id(self.id)
            .role(Role::RadioGroup)
            .aria_label(self.label)
            .on_key_down(on_key_down)
            .relative()
            .flex()
            .w_full()
            .bg(c.input_bg)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .children(segments)
            .children(focus_marks(
                focused,
                theme,
                radius,
                border.width,
                window,
                cx,
            ))
    }
}
