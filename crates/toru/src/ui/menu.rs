//! A popup menu under a button, such as a card's `⋯` or the Delayed seconds. The mockups show no
//! open menu, so it borrows the editor pill's surface, border and shadow, and the corner buttons'
//! item size. The arrow keys and Tab move between items, Enter or Space picks one, Esc closes.

use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, Anchor, App, Bounds, ElementId, Entity, FocusHandle, KeyDownEvent, Pixels,
    Role, SharedString, Window, actions, anchored, deferred, div, point, prelude::*, px,
};

use super::{Button, Handler, focus_ring, keyboard_focus, keyed_focus};
use crate::theme::{ActiveTheme, ThemedText};

actions!(menu, [NextItem, PreviousItem]);

/// The key context of an open menu's panel, which `ui::init` binds Tab in.
pub const KEY_CONTEXT: &str = "Menu";

const ITEM_HEIGHT: f32 = 32.;
const PANEL_PADDING: f32 = 4.;

#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    label: SharedString,
    trigger: Button,
    items: Vec<(SharedString, Handler)>,
    /// Line the panel up with the trigger's right edge, for buttons near the window's right side.
    align_right: bool,
}

impl Menu {
    /// `label` names the menu for screen readers; `trigger` opens it and its click is replaced.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, trigger: Button) -> Menu {
        Menu {
            id: id.into(),
            label: label.into(),
            trigger,
            items: Vec::new(),
            align_right: false,
        }
    }

    pub fn item(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Menu {
        self.items.push((label.into(), Rc::new(handler)));
        self
    }

    pub fn align_right(mut self) -> Menu {
        self.align_right = true;
        self
    }
}

/// Which item has keyboard focus, if any.
fn focused_item(items: &[FocusHandle], window: &Window) -> Option<usize> {
    items.iter().position(|focus| focus.is_focused(window))
}

/// Tab and Shift+Tab: moves focus `step` items on from the focused one, wrapping at either end.
fn step_focus(items: &[FocusHandle], step: isize, window: &mut Window, cx: &mut App) {
    let count = items.len() as isize;
    let current = focused_item(items, window).unwrap_or(0) as isize;
    let next = (current + step).rem_euclid(count) as usize;
    items[next].focus(window, cx);
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = Arc::new(self.id.clone());
        let child_id = |name: &str| ElementId::NamedChild(id.clone(), name.to_string().into());

        let open: Entity<bool> = window.use_keyed_state(child_id("open"), cx, |_, _| false);
        let is_open = *open.read(cx);
        let trigger_focus = keyed_focus(window, cx, &self.id);
        let item_focus: Vec<FocusHandle> = (0..self.items.len())
            .map(|index| keyed_focus(window, cx, &child_id(&format!("item {index}"))))
            .collect();
        // Where the trigger was drawn, so a press on it is left to its own click handler.
        let trigger_bounds: Rc<Cell<Bounds<Pixels>>> = Rc::default();
        let theme = cx.theme();
        let c = &theme.colors;

        let set_open = Rc::new(move |value: bool, cx: &mut App| {
            open.update(cx, |open, cx| {
                *open = value;
                cx.notify();
            });
        });
        let close = Rc::new({
            let set_open = set_open.clone();
            let trigger_focus = trigger_focus.clone();
            move |window: &mut Window, cx: &mut App| {
                set_open(false, cx);
                trigger_focus.focus(window, cx);
            }
        });

        let trigger = self
            .trigger
            .track_focus(&trigger_focus)
            .expanded(is_open)
            .on_click({
                let close = close.clone();
                let set_open = set_open.clone();
                let first = item_focus.first().cloned();
                move |window, cx| {
                    if is_open {
                        close(window, cx);
                    } else {
                        set_open(true, cx);
                        if let Some(first) = &first {
                            first.focus(window, cx);
                        }
                    }
                }
            });

        let on_key_down = {
            let close = close.clone();
            let items = item_focus.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let count = items.len();
                let current = focused_item(&items, window).unwrap_or(0);
                let next = match event.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        close(window, cx);
                        return;
                    }
                    "up" => (current + count - 1) % count,
                    "down" => (current + 1) % count,
                    "home" => 0,
                    "end" => count - 1,
                    _ => return,
                };
                cx.stop_propagation();
                items[next].focus(window, cx);
            }
        };

        let font = theme.type_scale.corner;
        let items = self
            .items
            .into_iter()
            .zip(&item_focus)
            .map(|((label, handler), focus)| {
                // Close first, so focus the handler sets (such as on a name field) is kept.
                let pick = {
                    let close = close.clone();
                    move |window: &mut Window, cx: &mut App| {
                        close(window, cx);
                        handler(window, cx);
                    }
                };
                let pick_a11y = pick.clone();
                div()
                    .id(child_id(&label))
                    .role(Role::MenuItem)
                    .aria_label(label.clone())
                    .track_focus(focus)
                    .h(px(ITEM_HEIGHT))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(theme.radii.sm)
                    .text_color(c.text)
                    .type_style(theme, font)
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|style| style.bg(c.on_bg).text_color(c.on_text))
                    .on_click(move |_, window, cx| pick(window, cx))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        pick_a11y(window, cx)
                    })
                    .child(font.case.apply(&label))
            })
            .collect::<Vec<_>>();

        // The ring is drawn by the panel, after every item, so no neighbour paints over it. The
        // trigger button draws the watcher that turns it off at a mouse press.
        let ring = focused_item(&item_focus, window)
            .filter(|_| keyboard_focus(window, cx))
            .map(|index| {
                let top = PANEL_PADDING + index as f32 * ITEM_HEIGHT;
                div()
                    .absolute()
                    .top(px(top))
                    .left(px(PANEL_PADDING))
                    .right(px(PANEL_PADDING))
                    .h(px(ITEM_HEIGHT))
                    .children(focus_ring(theme, theme.radii.sm, px(0.)))
            });

        let pop = theme.borders.pop;
        let panel = div()
            .id(child_id("panel"))
            .role(Role::Menu)
            .aria_label(self.label)
            .key_context(KEY_CONTEXT)
            .occlude()
            .on_key_down(on_key_down)
            .on_action({
                let items = item_focus.clone();
                move |_: &NextItem, window, cx| step_focus(&items, 1, window, cx)
            })
            .on_action({
                let items = item_focus.clone();
                move |_: &PreviousItem, window, cx| step_focus(&items, -1, window, cx)
            })
            .on_mouse_down_out({
                let close = close.clone();
                let trigger_bounds = trigger_bounds.clone();
                move |event, window, cx| {
                    // `close` puts focus back on the trigger; a focusable control under the
                    // press then takes it, since its own handler runs after this one.
                    if !trigger_bounds.get().contains(&event.position) {
                        close(window, cx);
                    }
                }
            })
            .relative()
            .flex()
            .flex_col()
            .min_w(px(160.))
            .p(px(PANEL_PADDING))
            .bg(c.surface)
            .border(pop.width)
            .border_color(pop.color)
            .rounded(theme.radii.lg)
            .shadow(theme.shadows.pop.clone())
            .children(items)
            .children(ring);

        let corner = if self.align_right {
            Anchor::TopRight
        } else {
            Anchor::TopLeft
        };
        div()
            .relative()
            .flex_none()
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(trigger) = bounds.first() {
                    trigger_bounds.set(*trigger);
                }
            })
            .child(trigger)
            .when(is_open, |this| {
                // A zero-size box at the trigger's bottom corner, which the panel hangs from.
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .when(self.align_right, |this| this.right_0())
                        .when(!self.align_right, |this| this.left_0())
                        .child(deferred(
                            anchored()
                                .anchor(corner)
                                .offset(point(px(0.), px(4.)))
                                .snap_to_window_with_margin(px(8.))
                                .child(panel),
                        )),
                )
            })
    }
}
