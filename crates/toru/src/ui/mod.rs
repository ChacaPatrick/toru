//! The shared controls, each drawn from the active `Theme` (`design/build-mockups.py` has the CSS
//! they follow) and each a tab stop with a role and a label for assistive technology.
//!
//! Keyboard focus shows the ring from `DESIGN.md` §7: 3px of `focus` 3px outside the control, with
//! `focus_halo` filling the gap. It is drawn as absolute children, so showing it never moves layout.
//! Like CSS `:focus-visible`, a key press turns the ring on and a mouse press turns it off; moving
//! the mouse changes nothing. (GPUI's own `last_input_was_keyboard` drops on a mouse move.) Each
//! ring also notes where it was drawn, so a scrolling container can bring it into view
//! (`FocusInView`). `init` binds the keys the controls handle as actions and starts watching for
//! key presses.

mod button;
mod chip;
mod fill;
mod flash;
mod icons;
mod input;
mod menu;
mod radio;
mod segmented;
mod toast;
mod toggle;
mod tooltip;

use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, Bounds, BoxShadow, ElementId, FocusHandle, Global, KeyBinding, MouseDownEvent,
    Pixels, Refineable, StyleRefinement, Window, WindowId, canvas, div, prelude::*, px,
};

use crate::theme::Theme;

#[cfg(test)]
pub use button::SUB_OPACITY;
pub use button::{Button, SMALL_HEIGHT as BUTTON_SMALL_HEIGHT};
pub use icons::Icon;
pub use menu::Menu;
pub use {
    chip::Chip, fill::fill, flash::Flash, input::TextField, radio::RadioGroup,
    segmented::Segmented, toast::Toast, toggle::Toggle, tooltip::Tooltip,
};

/// What a control does when pressed.
pub type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The smallest a click target may be (DESIGN.md §7). A control drawn smaller, such as a radio
/// mark, sits in a row this tall that takes the press.
pub const MIN_TARGET: f32 = 32.;

/// Binds the controls' keys. Call once, after `gpui_kit::init`, so these bindings are checked first.
pub fn init(cx: &mut App) {
    cx.intercept_keystrokes(|_, window, cx| set_keyboard_focus(true, window, cx))
        .detach();
    cx.bind_keys([
        // Inside an open menu, Tab steps through the items instead of leaving it.
        KeyBinding::new("tab", menu::NextItem, Some(menu::KEY_CONTEXT)),
        KeyBinding::new("shift-tab", menu::PreviousItem, Some(menu::KEY_CONTEXT)),
    ]);
}

/// A focus handle that lives as long as the control with `id` keeps being rendered, so callers
/// don't have to own one per control.
fn keyed_focus(window: &mut Window, cx: &mut App, id: &ElementId) -> FocusHandle {
    let key = ElementId::NamedChild(Arc::new(id.clone()), "focus".into());
    window
        .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

/// Gives `element` a theme shadow list. GPUI 0.3.11 measures inset shadows from the border box and
/// paints the border over them, so a band no wider than the border is hidden on any bordered
/// element. The drop shadows go on the element and the inset ones on an unfilled overlay child,
/// which sits inside the border, as a CSS inset shadow does. Call it before adding the content.
pub fn shadowed<E: Styled + ParentElement>(element: E, shadows: &[BoxShadow], radius: Pixels) -> E {
    let (inset, drop): (Vec<BoxShadow>, Vec<BoxShadow>) =
        shadows.iter().cloned().partition(|shadow| shadow.inset);
    let mut element = element.shadow(drop);
    if !inset.is_empty() {
        element = element.child(div().absolute().inset_0().rounded(radius).shadow(inset));
    }
    element
}

/// Where an arrow key moves a one-of-many choice such as a radio group: the neighbour, wrapping
/// at either end, or the first or last option. `None` for any other key.
fn arrow_target(key: &str, selected: usize, count: usize) -> Option<usize> {
    Some(match key {
        "left" | "up" => (selected + count - 1) % count,
        "right" | "down" => (selected + 1) % count,
        "home" => 0,
        "end" => count - 1,
        _ => return None,
    })
}

/// Applies the sizing a caller set on a control through `Styled`.
fn refined<E: Styled>(mut element: E, style: &StyleRefinement) -> E {
    element.style().refine(style);
    element
}

/// The windows whose last press was a key rather than a mouse button, where focus shows the ring.
#[derive(Default)]
struct KeyboardFocus(HashSet<WindowId>);

impl Global for KeyboardFocus {}

/// Which control last drew the focus ring and where the ring's outer edge was, so a scrolling
/// container can bring the control, ring and all, into view (`FocusInView`).
#[derive(Default)]
struct FocusRing(Option<(FocusHandle, Bounds<Pixels>)>);

impl Global for FocusRing {}

/// Keeps a scrolling container's keyboard-focused control in view: GPUI's Tab and the arrow keys
/// only move focus, so without this a control below the fold takes focus with no ring in sight.
/// It scrolls once per focus change, the least that shows the whole ring, and leaves the user's
/// own scrolling alone after that. The container's element must `track_focus` the handle from
/// `focus`, which is how a focus inside it is told from one elsewhere; it is not a tab stop.
pub struct FocusInView {
    focus: FocusHandle,
    /// What was focused inside the container at the last render, so a change of focus is told
    /// from the same focus seen again.
    last: Option<FocusHandle>,
    /// Whether the current focus still has to be scrolled to.
    pending: bool,
}

impl FocusInView {
    pub fn new(cx: &mut App) -> FocusInView {
        FocusInView {
            focus: cx.focus_handle(),
            last: None,
            pending: false,
        }
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// How far down the content must move so the focused control's ring is inside `viewport`,
    /// the container's bounds from the last frame, with the ring's gap as air between the two:
    /// negative to scroll down, zero when nothing is to be done. Call it in each render and add
    /// the result to the scroll offset.
    pub fn scroll_by(&mut self, viewport: Bounds<Pixels>, window: &Window, cx: &mut App) -> Pixels {
        let focused = window
            .focused(cx)
            .filter(|focused| self.focus.contains(focused, window));
        if focused != self.last {
            // A ring noted before the focus changed may be from before the user scrolled, so
            // only one painted from now on counts. The control that paints it asks for another
            // frame, and the scroll happens then.
            self.last = focused.clone();
            self.pending = focused.is_some();
            cx.default_global::<FocusRing>().0 = None;
            return px(0.);
        }
        let Some(focused) = focused.filter(|_| self.pending) else {
            return px(0.);
        };
        let ring = cx
            .try_global::<FocusRing>()
            .and_then(|ring| ring.0.as_ref())
            .filter(|(handle, _)| *handle == focused)
            .map(|(_, bounds)| *bounds);
        let Some(ring) = ring else {
            return px(0.);
        };
        self.pending = false;
        let air = px(RING_GAP);
        if ring.top() - air < viewport.top() {
            viewport.top() - (ring.top() - air)
        } else if ring.bottom() + air > viewport.bottom() {
            viewport.bottom() - (ring.bottom() + air)
        } else {
            px(0.)
        }
    }
}

/// Records where a focus ring was painted and, when the focus has moved since the last ring,
/// asks for another frame so the control's container can scroll to it.
fn note_focus_ring(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let Some(focused) = window.focused(cx) else {
        return;
    };
    let ring = cx.default_global::<FocusRing>();
    let moved = ring.0.as_ref().is_none_or(|(last, _)| *last != focused);
    ring.0 = Some((focused, bounds));
    if moved {
        window.request_animation_frame();
    }
}

fn keyboard_focus(window: &Window, cx: &App) -> bool {
    cx.try_global::<KeyboardFocus>()
        .is_some_and(|focus| focus.0.contains(&window.window_handle().window_id()))
}

fn set_keyboard_focus(on: bool, window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let windows = &mut cx.default_global::<KeyboardFocus>().0;
    let changed = if on {
        windows.insert(id)
    } else {
        windows.remove(&id)
    };
    if changed {
        window.refresh();
    }
}

/// What a control draws for keyboard focus: the ring when `focused` and the last press was a key,
/// and while the last press was a key, a zero-size watcher that turns the ring off at the next
/// mouse press anywhere in the window. Every control draws the watcher, so the rule holds whatever
/// view is the window's root.
fn focus_marks(
    focused: bool,
    theme: &Theme,
    radius: Pixels,
    border: Pixels,
    window: &Window,
    cx: &App,
) -> Vec<AnyElement> {
    if !keyboard_focus(window, cx) {
        return Vec::new();
    }
    let watcher = canvas(
        |_, _, _| (),
        |_, (), window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, window, cx| {
                if phase.capture() {
                    set_keyboard_focus(false, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_0();
    let mut marks = vec![watcher.into_any_element()];
    if focused {
        marks.extend(focus_ring(theme, radius, border));
    }
    marks
}

/// The ring's 3px width and how far its outer edge sits outside the control (DESIGN.md §7).
const RING_WIDTH: f32 = 3.;
const RING_GAP: f32 = RING_WIDTH * 2.;

/// Where an absolute child reaching `offset` outside a control sits: such children are placed
/// from inside the control's `border`, so it is added back.
fn ring_inset(offset: f32, border: Pixels) -> Pixels {
    px(-offset) - border
}

/// The ring and its halo, for a `relative()` control whose corners are rounded by `radius` and
/// whose own border is `border` wide, plus a zero-paint child the size of the outer ring that
/// notes where it was drawn (`FocusInView`). Every focused control draws all three.
fn focus_ring(theme: &Theme, radius: Pixels, border: Pixels) -> [AnyElement; 3] {
    let ring = |offset: f32, color| {
        let inset = ring_inset(offset, border);
        div()
            .absolute()
            .top(inset)
            .left(inset)
            .right(inset)
            .bottom(inset)
            // Square looks stay square, as a CSS outline does around a square box.
            .rounded(if radius > px(0.) {
                radius + px(offset)
            } else {
                px(0.)
            })
            .border(px(RING_WIDTH))
            .border_color(color)
    };
    // The same size as the outer ring, so what it records is what must be in view.
    let outer = ring_inset(RING_GAP, border);
    let note = canvas(
        |_, _, _| (),
        |bounds, (), window, cx| note_focus_ring(bounds, window, cx),
    )
    .absolute()
    .top(outer)
    .left(outer)
    .right(outer)
    .bottom(outer);
    [
        ring(RING_GAP, theme.colors.focus).into_any_element(),
        ring(RING_WIDTH, theme.colors.focus_halo).into_any_element(),
        note.into_any_element(),
    ]
}
