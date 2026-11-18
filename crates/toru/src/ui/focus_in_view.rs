use gpui::{App, Bounds, FocusHandle, Pixels, Window, px};

use super::focus_ring::{FocusRing, RING_GAP, keyboard_focus, mouse_held};

/// Keeps a scrolling container's keyboard-focused control in view: GPUI's Tab and the arrow keys
/// only move focus, so without this a control below the fold takes focus with no ring in sight.
/// It scrolls once per focus change, the least that shows the whole ring, and leaves the user's
/// own scrolling alone after that. A focus taken while a mouse button is held came from the
/// press, so it is left alone too: the control is under the pointer already, and scrolling it
/// away before the release would lose the click. The container's element must `track_focus` the
/// handle from `focus`, which is how a focus inside it is told from one elsewhere; it is not a
/// tab stop.
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
            // A place noted before the focus changed may be from before the user scrolled, so
            // only one noted from now on counts. The control that notes it asks for another
            // frame, and the scroll happens then. A key press counts even while a button is
            // held, as Tab during a drag does; a press with no key counts for nothing.
            self.last = focused.clone();
            self.pending =
                focused.is_some() && (keyboard_focus(window, cx) || !mouse_held(window, cx));
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
