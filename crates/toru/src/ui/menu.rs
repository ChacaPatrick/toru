//! A popup menu under a button, such as a card's `⋯` or the toolbar's Delayed. The mockups show no
//! open menu, so it borrows the editor pill's surface, border and shadow, and the corner buttons'
//! item size. The arrow keys and Tab move between items, Enter or Space picks one, Esc closes.
//!
//! Besides plain items, which close the menu when picked, a menu can hold choices: one of a group
//! that stays open when picked, so several can be set in one visit, with a check mark on the
//! chosen one so the choice is told apart by more than colour. Headers name the groups.

use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, Anchor, AnyElement, App, Bounds, ElementId, Entity, FocusHandle,
    KeyDownEvent, Pixels, Role, SharedString, Toggled, Window, actions, anchored, deferred, div,
    point, prelude::*, px,
};

use super::{
    Button, Handler, Icon, Readable, button::MenuTrigger, focus_ring, keyboard_focus, keyed_focus,
    menu_opened,
};
use crate::theme::{ActiveTheme, ThemedText};

actions!(menu, [NextItem, PreviousItem]);

/// The key context of an open menu's panel, which `ui::init` binds Tab in.
pub const KEY_CONTEXT: &str = "Menu";

const ITEM_HEIGHT: f32 = 32.;
const HEADER_HEIGHT: f32 = 26.;
const DIVIDER_MARGIN: f32 = 4.;
const PANEL_PADDING: f32 = 4.;
/// The check mark's column, in front of every item of a menu that has choices.
const CHECK_SLOT: f32 = 20.;
/// The group an item and its check mark share, so the mark recolours with the hovered item.
const HOVER_GROUP: &str = "menu-item";

enum Row {
    Item {
        label: SharedString,
        /// `Some` for a choice, with whether it is the chosen one.
        selected: Option<bool>,
        handler: Handler,
    },
    Header(SharedString),
    Divider,
}

#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    label: SharedString,
    trigger: Button,
    rows: Vec<Row>,
    /// Line the panel up with the trigger's right edge, for buttons near the window's right side.
    align_right: bool,
}

impl Menu {
    /// `label` names the menu for screen readers; `trigger` opens it and its click is replaced,
    /// unless it is a split button, whose sub opens the menu instead (`Button::split`).
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, trigger: Button) -> Menu {
        Menu {
            id: id.into(),
            label: label.into(),
            trigger,
            rows: Vec::new(),
            align_right: false,
        }
    }

    /// An action; picking it closes the menu.
    pub fn item(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Menu {
        self.rows.push(Row::Item {
            label: label.into(),
            selected: None,
            handler: Rc::new(handler),
        });
        self
    }

    /// One of a group of choices; picking it leaves the menu open.
    pub fn choice(
        mut self,
        label: impl Into<SharedString>,
        selected: bool,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Menu {
        self.rows.push(Row::Item {
            label: label.into(),
            selected: Some(selected),
            handler: Rc::new(handler),
        });
        self
    }

    /// Names the group of rows that follows.
    pub fn header(mut self, label: impl Into<SharedString>) -> Menu {
        self.rows.push(Row::Header(label.into()));
        self
    }

    pub fn divider(mut self) -> Menu {
        self.rows.push(Row::Divider);
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
        let mut trigger_focus = keyed_focus(window, cx, &self.id);
        if let Some(tab_stop) = self.trigger.tab_stop_setting() {
            trigger_focus = trigger_focus.tab_stop(tab_stop);
        }
        let item_count = self
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Item { .. }))
            .count();
        let item_focus: Vec<FocusHandle> = (0..item_count)
            .map(|index| keyed_focus(window, cx, &child_id(&format!("item {index}"))))
            .collect();
        let has_choices = self.rows.iter().any(|row| {
            matches!(
                row,
                Row::Item {
                    selected: Some(_),
                    ..
                }
            )
        });
        // Where the trigger was drawn, so a press on it is left to its own click handler.
        let trigger_bounds: Rc<Cell<Bounds<Pixels>>> = Rc::default();
        let theme = cx.theme();
        let c = &theme.colors;

        let set_open = Rc::new(move |value: bool, cx: &mut App| {
            open.update(cx, |open, cx| {
                *open = value;
                cx.notify();
            });
            if value {
                menu_opened(&open, cx);
            }
        });
        let close = Rc::new({
            let set_open = set_open.clone();
            let trigger_focus = trigger_focus.clone();
            move |window: &mut Window, cx: &mut App| {
                set_open(false, cx);
                trigger_focus.focus(window, cx);
            }
        });

        let toggle: Handler = Rc::new({
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
        let trigger = self.trigger.menu_trigger(MenuTrigger {
            focus: trigger_focus.clone(),
            open: is_open,
            toggle,
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
        let header_font = theme.type_scale.label;
        let hair = theme.borders.hair;
        // The ring is drawn by the panel, after every row, so no neighbour paints over it. The
        // trigger button draws the watcher that turns it off at a mouse press.
        let focused = focused_item(&item_focus, window).filter(|_| keyboard_focus(window, cx));
        let mut ring_top = None;
        let mut top = PANEL_PADDING;
        let mut item_index = 0;
        let mut item_focus_iter = item_focus.iter();
        let rows = self
            .rows
            .into_iter()
            .map(|row| match row {
                Row::Header(label) => {
                    top += HEADER_HEIGHT;
                    div()
                        .id(child_id(&format!("header {label}")))
                        .read_as_text(&label)
                        .h(px(HEADER_HEIGHT))
                        .px_2()
                        .flex()
                        .items_end()
                        .pb(px(4.))
                        .text_color(c.label)
                        .type_style(theme, header_font)
                        .whitespace_nowrap()
                        .child(header_font.case.apply(&label))
                        .into_any_element()
                }
                Row::Divider => {
                    top += DIVIDER_MARGIN * 2. + f32::from(hair.width);
                    div()
                        .my(px(DIVIDER_MARGIN))
                        .h(hair.width)
                        .bg(hair.color)
                        .into_any_element()
                }
                Row::Item {
                    label,
                    selected,
                    handler,
                } => {
                    if focused == Some(item_index) {
                        ring_top = Some(top);
                    }
                    top += ITEM_HEIGHT;
                    item_index += 1;
                    let focus = item_focus_iter.next().expect("one handle per item");
                    // A plain item closes first, so focus the handler sets (such as on a name
                    // field) is kept. A choice stays open for the next choice.
                    let pick = {
                        let close = close.clone();
                        move |window: &mut Window, cx: &mut App| {
                            if selected.is_none() {
                                close(window, cx);
                            }
                            handler(window, cx);
                        }
                    };
                    let pick_a11y = pick.clone();
                    let (role, toggled) = match selected {
                        Some(on) => (
                            Role::MenuItemRadio,
                            Some(if on { Toggled::True } else { Toggled::False }),
                        ),
                        None => (Role::MenuItem, None),
                    };
                    div()
                        .id(child_id(&label))
                        .role(role)
                        .aria_label(label.clone())
                        .when_some(toggled, |this, toggled| this.aria_toggled(toggled))
                        .track_focus(focus)
                        .h(px(ITEM_HEIGHT))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded(theme.radii.sm)
                        .text_color(c.text)
                        .type_style(theme, font)
                        .whitespace_nowrap()
                        .cursor_pointer()
                        // Not `surface_sel`: it is `surface` itself in some palettes.
                        .group(HOVER_GROUP)
                        .hover(|style| style.bg(c.on_bg).text_color(c.on_text))
                        .on_click(move |_, window, cx| pick(window, cx))
                        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                            pick_a11y(window, cx)
                        })
                        .when(has_choices, |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .w(px(CHECK_SLOT))
                                    .flex()
                                    .justify_center()
                                    .children((selected == Some(true)).then(|| {
                                        // The icon's colour is its own, so it follows the hover.
                                        Icon::Check
                                            .render(theme, c.text)
                                            .group_hover(HOVER_GROUP, |style| {
                                                style.text_color(c.on_text)
                                            })
                                    })),
                            )
                        })
                        .child(font.case.apply(&label))
                        .into_any_element()
                }
            })
            .collect::<Vec<AnyElement>>();

        let ring = ring_top.map(|top| {
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
            .children(rows)
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
