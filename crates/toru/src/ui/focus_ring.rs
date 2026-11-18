use std::collections::HashSet;

use gpui::{
    AccessibleAction, AnyElement, App, Bounds, FocusHandle, Global, Hsla, MouseDownEvent,
    MouseUpEvent, Pixels, Window, WindowId, canvas, div, prelude::*, px,
};

use crate::theme::Theme;

/// The windows whose last press was a key rather than a mouse button, where focus shows the ring.
#[derive(Default)]
struct KeyboardFocus(HashSet<WindowId>);

impl Global for KeyboardFocus {}

/// The windows where a mouse button is held down right now. Focus taken during a press came
/// from the press, so `FocusInView` leaves it where it is.
#[derive(Default)]
struct MouseHeld(HashSet<WindowId>);

impl Global for MouseHeld {}

/// Which focused control last noted its place and where its ring's outer edge is, so a scrolling
/// container can bring the control, ring and all, into view (`FocusInView`).
#[derive(Default)]
pub(super) struct FocusRing(pub(super) Option<(FocusHandle, Bounds<Pixels>)>);

impl Global for FocusRing {}

/// Records where the focused control's ring is or would be and, when the focus has moved since
/// the last note, asks for another frame so the control's container can scroll to it.
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

/// `track_focus` for a control that shows the ring. Focus that assistive technology asks for
/// shows it as a key press does: a screen reader's own navigation keys, voice control and a
/// switch reach the app as a request to focus a control, never as a key, so without this their
/// users would not see where focus is.
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

/// What a control draws for focus: a zero-paint note of where it is whenever it is focused, so a
/// scrolling container can bring it into view (`FocusInView`); a zero-size watcher that notes a
/// mouse press anywhere in the window, which turns the ring off and, until the release, marks
/// any focus taken as the press's own; and the ring when `focused` and the last press was a key.
/// Every control draws the watcher, so the rules hold whatever view is the window's root. Public
/// for the gallery's cards, which are focusable themselves.
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

/// `focus_marks` with the ring in `ring`: the bulk bar's controls use `bulk_focus`, since the
/// bar's own colour can swallow `focus`.
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

/// The ring's 3px width and how far its outer edge sits outside the control (DESIGN.md §7).
pub(super) const RING_WIDTH: f32 = 3.;
pub(super) const RING_GAP: f32 = RING_WIDTH * 2.;

/// Where an absolute child reaching `offset` outside a control sits: such children are placed
/// from inside the control's `border`, so it is added back.
pub(super) fn ring_inset(offset: f32, border: Pixels) -> Pixels {
    px(-offset) - border
}

/// The size of the outer ring, which is what must be in view, noted where it was drawn
/// (`FocusInView`) without painting anything.
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

/// The ring and its halo, for a `relative()` control whose corners are rounded by `radius` and
/// whose own border is `border` wide. `focus_marks` adds the note; a text field, which always
/// shows its ring, draws these alone.
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

/// A selected card's outline (`.card.sel`): the accent where the focus ring goes, with no halo.
/// A focused card draws its ring over it.
pub fn selection_ring(theme: &Theme, radius: Pixels, border: Pixels) -> AnyElement {
    ring(RING_GAP, theme.colors.accent, radius, border)
}

/// A 3px line whose outer edge is `offset` outside a control with `border`, following its
/// rounding, as a CSS outline does. Square looks stay square.
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
