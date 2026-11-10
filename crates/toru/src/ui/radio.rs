//! A radio group, as the wireframes draw On close: a column of rows, each a round (or, in the
//! square looks, square) mark beside its label, taking the toggle's colours so a chosen option
//! reads like a toggle that is on. One tab stop: focus sits on the chosen row, the arrow keys
//! move the choice and focus together, and any row, the full width of the group and the minimum
//! target tall (DESIGN.md §7), can be clicked. A row can carry a detail under its label, such as
//! the palette picker's swatches, which is part of the same target.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, AnyElement, App, ElementId, FocusHandle, KeyDownEvent, Role, SharedString,
    Toggled, Window, div, prelude::*, px,
};

use super::{MIN_TARGET, arrow_target, focus_marks, keyed_focus};
use crate::theme::{ActiveTheme, ThemedText};

type ChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

const MARK: f32 = 18.;
const DOT: f32 = 8.;
/// Between rows, each already `MIN_TARGET` tall: the smallest gap §7 asks for between targets.
const ROW_GAP: f32 = 6.;
const MARK_GAP: f32 = 10.;
/// Between a label and the detail under it, which sits closer to its own label than to the next
/// row's.
const DETAIL_GAP: f32 = 2.;

#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    label: SharedString,
    description: Option<SharedString>,
    options: Vec<SharedString>,
    selected: usize,
    details: Vec<Option<AnyElement>>,
    on_change: Option<ChangeHandler>,
}

impl RadioGroup {
    /// `label` names the group, such as "On close"; `selected` indexes `options`.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> RadioGroup {
        RadioGroup {
            id: id.into(),
            label: label.into(),
            description: None,
            options: options.into_iter().map(Into::into).collect(),
            selected,
            details: Vec::new(),
            on_change: None,
        }
    }

    /// A hint for the group as a whole, such as the text under it.
    pub fn description(mut self, description: impl Into<SharedString>) -> RadioGroup {
        self.description = Some(description.into());
        self
    }

    /// One element per option, drawn under its label and aligned with it.
    pub fn details(mut self, details: impl IntoIterator<Item = AnyElement>) -> RadioGroup {
        self.details = details.into_iter().map(Some).collect();
        self
    }

    /// A detail under one option's label alone.
    pub fn detail(mut self, option: usize, detail: AnyElement) -> RadioGroup {
        if self.details.len() <= option {
            self.details.resize_with(option + 1, || None);
        }
        self.details[option] = Some(detail);
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> RadioGroup {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.options.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let on_change = self.on_change;
        let id = Arc::new(self.id.clone());
        let row_id = |option: &SharedString| ElementId::NamedChild(id.clone(), option.clone());

        // Only the chosen row is in the tab order, so the group stays one tab stop.
        let focus: Vec<FocusHandle> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, option)| {
                keyed_focus(window, cx, &row_id(option)).tab_stop(index == selected)
            })
            .collect();
        let theme = cx.theme();
        let c = &theme.colors;
        let border = theme.borders.toggle;
        let radius = theme.radii.pill;
        let font = theme.type_scale.setting;

        let on_key_down = {
            let on_change = on_change.clone();
            let focus = focus.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                // A control inside a detail, such as a text field, keeps its own arrow keys.
                if !focus.iter().any(|row| row.is_focused(window)) {
                    return;
                }
                let Some(next) = arrow_target(&event.keystroke.key, selected, count) else {
                    return;
                };
                cx.stop_propagation();
                focus[next].focus(window, cx);
                if let Some(handler) = &on_change {
                    handler(next, window, cx);
                }
            }
        };

        let mut details = self.details.into_iter();
        let rows = self
            .options
            .iter()
            .zip(&focus)
            .enumerate()
            .map(|(index, (option, focus))| {
                let on = index == selected;
                let detail = details.next().flatten();
                let choose = on_change.clone();
                let choose_a11y = on_change.clone();
                let mark = div()
                    .relative()
                    .flex_none()
                    .size(px(MARK))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if on { c.toggle_on } else { c.toggle_off })
                    .border(border.width)
                    .border_color(border.color)
                    .rounded(radius)
                    .when(on, |mark| {
                        mark.child(div().size(px(DOT)).rounded(radius).bg(c.toggle_on_knob))
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
                    .id(row_id(option))
                    .role(Role::RadioButton)
                    .aria_label(option.clone())
                    .aria_toggled(if on { Toggled::True } else { Toggled::False })
                    .track_focus(focus)
                    .flex()
                    .flex_col()
                    .gap(px(DETAIL_GAP))
                    .text_color(c.text)
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
                    .child(
                        div()
                            .min_h(px(MIN_TARGET))
                            .flex()
                            .items_center()
                            .gap(px(MARK_GAP))
                            .child(mark)
                            .child(font.case.apply(option)),
                    )
                    .when_some(detail, |row, detail| {
                        row.child(div().pl(px(MARK + MARK_GAP)).child(detail))
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id(self.id)
            .role(Role::RadioGroup)
            .aria_label(self.label)
            .when_some(self.description, |this, text| this.aria_description(text))
            .on_key_down(on_key_down)
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .children(rows)
    }
}
