//! The shared controls, each drawn from the active `Theme` (`design/build-mockups.py` has the CSS
//! they follow) and each a tab stop with a role and a label for assistive technology.
//!
//! Keyboard focus shows the ring from `DESIGN.md` §7: 3px of `focus` 3px outside the control, with
//! `focus_halo` filling the gap. It is drawn as absolute children, so showing it never moves layout.
//! Like CSS `:focus-visible`, a key press turns the ring on and a mouse press turns it off; moving
//! the mouse changes nothing. (GPUI's own `last_input_was_keyboard` drops on a mouse move.)
//! `init` binds the keys the controls handle as actions and starts watching for key presses.

mod button;
mod chip;
mod flash;
mod icons;
mod input;
mod menu;
mod segmented;
mod toast;
mod toggle;

use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, BoxShadow, Div, ElementId, FocusHandle, Global, KeyBinding, MouseDownEvent,
    Pixels, Refineable, StyleRefinement, Window, WindowId, canvas, div, prelude::*, px,
};

use crate::theme::Theme;

pub use button::Button;
#[cfg(test)]
pub use button::SUB_OPACITY;
pub use chip::Chip;
pub use flash::Flash;
pub use icons::Icon;
pub use input::TextField;
pub use menu::Menu;
pub use segmented::Segmented;
pub use toast::Toast;
pub use toggle::Toggle;

/// What a control does when pressed.
pub type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

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

/// Applies the sizing a caller set on a control through `Styled`.
fn refined<E: Styled>(mut element: E, style: &StyleRefinement) -> E {
    element.style().refine(style);
    element
}

/// The windows whose last press was a key rather than a mouse button, where focus shows the ring.
#[derive(Default)]
struct KeyboardFocus(HashSet<WindowId>);

impl Global for KeyboardFocus {}

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
        marks.extend(
            focus_ring(theme, radius, border)
                .into_iter()
                .map(IntoElement::into_any_element),
        );
    }
    marks
}

/// The ring and its halo, for a `relative()` control whose corners are rounded by `radius` and
/// whose own border is `border` wide. Absolute children are placed from inside that border, so it
/// is added back to keep the ring's gap measured from the control's outer edge.
fn focus_ring(theme: &Theme, radius: Pixels, border: Pixels) -> [Div; 2] {
    let ring = |offset: f32, color| {
        let inset = px(-offset) - border;
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
            .border(px(3.))
            .border_color(color)
    };
    [
        ring(6., theme.colors.focus),
        ring(3., theme.colors.focus_halo),
    ]
}
