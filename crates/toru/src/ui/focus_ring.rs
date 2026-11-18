use std::collections::HashSet;

use gpui::{
    AccessibleAction, AnyElement, App, Bounds, FocusHandle, Global, Hsla, MouseDownEvent,
    MouseUpEvent, Pixels, Window, WindowId, canvas, div, prelude::*, px,
};

use crate::theme::Theme;

/// Windows whose last press was a key, where focus shows the ring, as with CSS `:focus-visible`.
/// GPUI's own `last_input_was_keyboard` won't do: it drops on a mere mouse move.
#[derive(Default)]
struct KeyboardFocus(HashSet<WindowId>);

impl Global for KeyboardFocus {}

/// Focus taken during a press came from the press, so `FocusInView` leaves it where it is.
#[derive(Default)]
struct MouseHeld(HashSet<WindowId>);

impl Global for MouseHeld {}

/// The focused control and its ring's outer edge, for `FocusInView`.
#[derive(Default)]
pub(super) struct FocusRing(pub(super) Option<(FocusHandle, Bounds<Pixels>)>);

impl Global for FocusRing {}

/// A new focus asks for another frame, so its container can scroll to it then.
pub(super) fn note_focus_ring(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
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

pub(super) fn keyboard_focus(window: &Window, cx: &App) -> bool {
    cx.try_global::<KeyboardFocus>()
        .is_some_and(|focus| focus.0.contains(&window.window_handle().window_id()))
}

pub(super) fn set_keyboard_focus(on: bool, window: &mut Window, cx: &mut App) {
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

/// Focus that assistive technology asks for shows the ring as a key press does: a screen reader's
/// navigation keys, voice control and switches reach the app as focus requests, never as keys.
pub trait RingFocus: StatefulInteractiveElement {
    fn track_ring_focus(self, focus: &FocusHandle) -> Self {
        let requested = focus.clone();
        self.track_focus(focus)
            .on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                set_keyboard_focus(true, window, cx);
                requested.focus(window, cx);
            })
    }
}

impl<E: StatefulInteractiveElement> RingFocus for E {}

pub(super) fn mouse_held(window: &Window, cx: &App) -> bool {
    cx.try_global::<MouseHeld>()
        .is_some_and(|held| held.0.contains(&window.window_handle().window_id()))
}

pub(super) fn set_mouse_held(on: bool, window: &Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let windows = &mut cx.default_global::<MouseHeld>().0;
    if on {
        windows.insert(id);
    } else {
        windows.remove(&id);
    }
}

/// The focus note, a watcher for mouse presses anywhere in the window, and the ring after a key
/// press. Every control draws the watcher, so the rules hold whatever view is the window's root.
pub fn focus_marks(
    focused: bool,
    theme: &Theme,
    radius: Pixels,
    border: Pixels,
    window: &Window,
    cx: &App,
) -> Vec<AnyElement> {
    focus_marks_in(
        focused,
        theme.colors.focus,
        theme,
        radius,
        border,
        window,
        cx,
    )
}

/// For the bulk bar, whose colour can swallow `focus`.
pub fn focus_marks_in(
    focused: bool,
    ring: Hsla,
    theme: &Theme,
    radius: Pixels,
    border: Pixels,
    window: &Window,
    cx: &App,
) -> Vec<AnyElement> {
    let mut marks = Vec::new();
    if focused {
        marks.push(focus_note(border));
    }
    let watcher = canvas(
        |_, _, _| (),
        |_, (), window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, window, cx| {
                if phase.capture() {
                    set_keyboard_focus(false, window, cx);
                    set_mouse_held(true, window, cx);
                }
            });
            window.on_mouse_event(|_: &MouseUpEvent, phase, window, cx| {
                if phase.capture() {
                    set_mouse_held(false, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_0();
    marks.push(watcher.into_any_element());
    if focused && keyboard_focus(window, cx) {
        marks.extend(focus_ring_in(ring, theme, radius, border));
    }
    marks
}

/// DESIGN.md §7. `RING_GAP` is how far the ring's outer edge sits outside the control.
pub(super) const RING_WIDTH: f32 = 3.;
pub(super) const RING_GAP: f32 = RING_WIDTH * 2.;

/// Absolute children are placed from inside the control's `border`, so it is added back.
pub(super) fn ring_inset(offset: f32, border: Pixels) -> Pixels {
    px(-offset) - border
}

/// Notes the outer ring's bounds, which is what must be in view, without painting.
pub(super) fn focus_note(border: Pixels) -> AnyElement {
    let outer = ring_inset(RING_GAP, border);
    canvas(
        |_, _, _| (),
        |bounds, (), window, cx| note_focus_ring(bounds, window, cx),
    )
    .absolute()
    .top(outer)
    .left(outer)
    .right(outer)
    .bottom(outer)
    .into_any_element()
}

/// For a `relative()` control. Without the note, for a text field, which always shows its ring.
pub(super) fn focus_ring(theme: &Theme, radius: Pixels, border: Pixels) -> [AnyElement; 2] {
    focus_ring_in(theme.colors.focus, theme, radius, border)
}

pub(super) fn focus_ring_in(
    color: Hsla,
    theme: &Theme,
    radius: Pixels,
    border: Pixels,
) -> [AnyElement; 2] {
    [
        ring(RING_GAP, color, radius, border),
        ring(RING_WIDTH, theme.colors.focus_halo, radius, border),
    ]
}

/// `.card.sel`: the accent where the focus ring goes. A focused card draws its ring over it.
pub fn selection_ring(theme: &Theme, radius: Pixels, border: Pixels) -> AnyElement {
    ring(RING_GAP, theme.colors.accent, radius, border)
}

pub(super) fn ring(offset: f32, color: Hsla, radius: Pixels, border: Pixels) -> AnyElement {
    let inset = ring_inset(offset, border);
    div()
        .absolute()
        .top(inset)
        .left(inset)
        .right(inset)
        .bottom(inset)
        .rounded(if radius > px(0.) {
            radius + px(offset)
        } else {
            px(0.)
        })
        .border(px(RING_WIDTH))
        .border_color(color)
        .into_any_element()
}
