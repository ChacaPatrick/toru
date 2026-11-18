use gpui::{App, Bounds, FocusHandle, Pixels, Window, px};

use super::focus_ring::{FocusRing, RING_GAP, keyboard_focus, mouse_held};

/// GPUI's Tab and arrow keys only move focus, so without this a control below the fold takes focus
/// out of sight. It scrolls once per focus change, the least that shows the whole ring. A focus
/// taken while a mouse button is held is left alone: scrolling before the release would lose the
/// click. The container must `track_focus` the handle from `focus`; it is not a tab stop.
pub struct FocusInView {
    focus: FocusHandle,
    last_focused: Option<FocusHandle>,
    scroll_pending: bool,
}

impl FocusInView {
    pub fn new(cx: &mut App) -> FocusInView {
        FocusInView {
            focus: cx.focus_handle(),
            last_focused: None,
            scroll_pending: false,
        }
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Negative to scroll down. `viewport` is the container's bounds from the last frame. Call it
    /// in each render and add the result to the scroll offset.
    pub fn scroll_by(&mut self, viewport: Bounds<Pixels>, window: &Window, cx: &mut App) -> Pixels {
        let focused = window
            .focused(cx)
            .filter(|focused| self.focus.contains(focused, window));
        if focused != self.last_focused {
            // A place noted before the focus changed may predate the user's scrolling, so only a
            // fresh note counts; the control asks for another frame and the scroll happens then.
            // A key press counts even while a button is held, as Tab during a drag does.
            self.last_focused = focused.clone();
            self.scroll_pending =
                focused.is_some() && (keyboard_focus(window, cx) || !mouse_held(window, cx));
            cx.default_global::<FocusRing>().0 = None;
            return px(0.);
        }
        let Some(focused) = focused.filter(|_| self.scroll_pending) else {
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
        self.scroll_pending = false;
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
