//! A pressable tile: the editor's `.tool`, `.opt`, `.sw` and `.corner button` are all a box of
//! the caller's colours, border and size with something in it, so one control draws them. It is
//! a button, or one radio button of a `Choices` group, with a hover tint, the §7 focus ring and
//! an assistive-technology click.
//!
//! `Choices` lays out a row of pressables as one tab stop: focus sits on the chosen one and the
//! arrow keys move the choice and the focus together, as `Segmented` does.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, AnyElement, App, ElementId, FocusHandle, Hsla, KeyDownEvent, Pixels, Role,
    SharedString, StyleRefinement, Toggled, Window, div, prelude::*, px,
};

use super::{Handler, RingFocus, arrow_target, focus_marks, keyed_focus, refined};
use crate::theme::{ActiveTheme, Border, Theme, contrast, hover_tint};

#[derive(IntoElement)]
pub struct Pressable {
    id: ElementId,
    aria_label: SharedString,
    bg: Option<Hsla>,
    text: Hsla,
    border: Border,
    radius: Pixels,
    /// Whether hovering tints the box; a colour swatch keeps its colour.
    hover: bool,
    /// `Some` for a radio button of a `Choices` group: whether it is the chosen one.
    chosen: Option<bool>,
    focus: Option<FocusHandle>,
    on_click: Option<Handler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    /// How far the pressable area reaches past the visible box on every side, so a box the
    /// mockups draw under §7's 32px can still be hit at 32px. It takes no room in the layout.
    hit_margin: Pixels,
}

impl Pressable {
    /// A square or sized box with no border or background, in `text`; set the rest with the
    /// builder methods and the size with `Styled`.
    pub fn new(id: impl Into<ElementId>, aria_label: impl Into<SharedString>, text: Hsla) -> Self {
        Pressable {
            id: id.into(),
            aria_label: aria_label.into(),
            bg: None,
            text,
            border: Border::none(),
            radius: px(0.),
            hover: true,
            chosen: None,
            focus: None,
            on_click: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
            hit_margin: px(0.),
        }
    }

    pub fn hit_margin(mut self, margin: Pixels) -> Self {
        self.hit_margin = margin;
        self
    }

    pub fn bg(mut self, bg: Hsla) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    pub fn no_hover(mut self) -> Self {
        self.hover = false;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Use `focus` instead of the pressable's own handle.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    fn radio(mut self, chosen: bool) -> Self {
        self.chosen = Some(chosen);
        self
    }
}

impl Styled for Pressable {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Pressable {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Pressable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self
            .focus
            .unwrap_or_else(|| keyed_focus(window, cx, &self.id));
        let theme = cx.theme();
        let text = self.text;
        let border = self.border;
        let radius = self.radius;
        let handler = self.on_click;
        let for_a11y = handler.clone();
        // The tint goes over the box's own fill. The chosen one of a group isn't tinted at all:
        // the mockups give `.tool.on` and `.opt.on` no hover, and a tint of its text over its
        // fill would take some pairs under §7's 4.5:1.
        let hovered = (self.hover && self.chosen != Some(true)).then(|| hover_bg(self.bg, text));
        // The visible box, in the caller's style. The `hit` wrapper around it takes the press,
        // the focus and the hover, reaching `hit_margin` past the box on every side and giving
        // that back as a negative margin, so the box sits in the layout as if alone.
        let tile = div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .when_some(self.bg, |this, bg| this.bg(bg))
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .when_some(hovered, |this, hovered| {
                this.group_hover(HIT_GROUP, |style| style.bg(hovered))
            })
            .children(self.children)
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
            .group(HIT_GROUP)
            .role(match self.chosen {
                Some(_) => Role::RadioButton,
                None => Role::Button,
            })
            .aria_label(self.aria_label)
            .when_some(self.chosen, |this, chosen| {
                this.aria_toggled(if chosen {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .track_ring_focus(&focus)
            .flex_none()
            .p(self.hit_margin)
            .m(-self.hit_margin)
            .cursor_pointer()
            .when_some(handler, |this, handler| {
                this.on_click(move |_, window, cx| handler(window, cx))
            })
            .when_some(for_a11y, |this, handler| {
                this.on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    handler(window, cx)
                })
            })
            .child(refined(tile, &self.style))
    }
}

/// The hover group a pressable's wrapper and box share.
const HIT_GROUP: &str = "pressable";

/// The hovered background: `text`'s tint laid over `bg` when the box has one, else the tint
/// alone, which the surface shows through.
fn hover_bg(bg: Option<Hsla>, text: Hsla) -> Hsla {
    match bg {
        Some(bg) => contrast::over(hover_tint(text), bg),
        None => hover_tint(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::contrast::ratio;
    use toru_core::settings::{BUILT_IN_PALETTES, Look};

    /// DESIGN.md §7: an unchosen tool, option or corner button stays readable under the pointer
    /// in every palette. (The chosen one isn't tinted.)
    #[test]
    fn a_hovered_box_keeps_its_contrast() {
        let failures: Vec<String> = [Look::Japandi, Look::PixelArt, Look::NeoBrutalism]
            .into_iter()
            .flat_map(|look| (0..BUILT_IN_PALETTES).map(move |palette| Theme::new(look, palette)))
            .flat_map(|theme| {
                let c = &theme.colors;
                let (look, text) = (theme.look, c.text);
                [
                    ("tool", contrast::over(hover_bg(None, text), c.surface)),
                    ("option", hover_bg(Some(c.input_bg), text)),
                ]
                .into_iter()
                .filter_map(move |(what, bg)| {
                    let ratio = ratio(text, bg);
                    (ratio < 4.5).then(|| format!("{look:?} {what}: {ratio:.2}"))
                })
            })
            .collect();
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn an_unfilled_box_is_tinted_alone() {
        let text = gpui::rgb(0x2b2a26).into();
        assert_eq!(hover_bg(None, text), hover_tint(text));
    }
}

type ChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ItemBuilder = Box<dyn Fn(usize, bool, &Theme) -> Pressable>;

/// A row of pressables of which one is chosen, as one tab stop. `item` builds the pressable at
/// an index given whether it is the chosen one and the theme; the group adds its focus and
/// radio role.
#[derive(IntoElement)]
pub struct Choices {
    id: ElementId,
    label: SharedString,
    count: usize,
    chosen: usize,
    item: ItemBuilder,
    on_change: Option<ChangeHandler>,
    style: StyleRefinement,
}

impl Choices {
    /// `label` names the group, such as "Stroke"; `chosen` is below `count`.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        count: usize,
        chosen: usize,
        item: impl Fn(usize, bool, &Theme) -> Pressable + 'static,
    ) -> Choices {
        Choices {
            id: id.into(),
            label: label.into(),
            count,
            chosen,
            item: Box::new(item),
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Choices {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for Choices {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Choices {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.count;
        let chosen = self.chosen.min(count.saturating_sub(1));
        let id = Arc::new(self.id.clone());
        // Only the chosen item is in the tab order, so the group stays one tab stop.
        let focus: Vec<FocusHandle> = (0..count)
            .map(|index| {
                let item_id = ElementId::NamedChild(id.clone(), format!("{index}").into());
                keyed_focus(window, cx, &item_id).tab_stop(index == chosen)
            })
            .collect();
        let on_change = self.on_change;
        let on_key_down = {
            let on_change = on_change.clone();
            let focus = focus.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let Some(next) = arrow_target(&event.keystroke.key, chosen, count) else {
                    return;
                };
                cx.stop_propagation();
                focus[next].focus(window, cx);
                if let Some(handler) = &on_change {
                    handler(next, window, cx);
                }
            }
        };
        let theme = cx.theme();
        let items = (0..count).map(|index| {
            let choose = on_change.clone();
            (self.item)(index, index == chosen, theme)
                .radio(index == chosen)
                .track_focus(&focus[index])
                .on_click(move |window, cx| {
                    if let Some(handler) = &choose {
                        handler(index, window, cx);
                    }
                })
        });
        let group = div()
            .id(self.id)
            .role(Role::RadioGroup)
            .aria_label(self.label)
            .on_key_down(on_key_down)
            .flex()
            .items_center()
            .children(items);
        refined(group, &self.style)
    }
}
