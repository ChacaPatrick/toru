//! The shared controls, each drawn from the active `Theme` (`design/build-mockups.py` has the CSS
//! they follow) and each a tab stop with a role and a label for assistive technology.
//!
//! Keyboard focus shows the ring from `DESIGN.md` §7: 3px of `focus` 3px outside the control, with
//! `focus_halo` filling the gap. It is drawn as absolute children, so showing it never moves layout.
//! Like CSS `:focus-visible`, a key press turns the ring on and a mouse press turns it off; moving
//! the mouse changes nothing. (GPUI's own `last_input_was_keyboard` drops on a mouse move.) Focus
//! that assistive technology asks for turns it on as a key press does (`RingFocus`). A
//! focused control also notes where its ring is, shown or not, so a scrolling container can bring
//! it into view (`FocusInView`) when a key or assistive technology moved the focus; a mouse press
//! already has the control under the pointer, and scrolling then would move it away before the
//! release. `init` binds the keys the controls handle as actions and starts watching for key
//! presses.

mod a11y;
mod button;
mod checkbox;
mod chip;
pub mod dialog;
mod fill;
mod flash;
mod focus_in_view;
mod focus_ring;
mod icons;
mod input;
mod jump;
mod menu;
mod open_menus;
mod pressable;
mod radio;
mod segmented;
mod slider;
mod toast;
mod toggle;
mod tooltip;

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, BoxShadow, ElementId, FocusHandle, KeyBinding, Pixels, Refineable, StyleRefinement,
    Window, div, prelude::*,
};

pub use a11y::{Readable, spoken};
pub use button::{
    BULK_HEIGHT as BULK_BUTTON_HEIGHT, Button, SMALL_HEIGHT as BUTTON_SMALL_HEIGHT, SUB_OPACITY,
};
pub use checkbox::SIZE as CHECKBOX_SIZE;
pub use dialog::Dialog;
pub use focus_in_view::FocusInView;
pub use focus_ring::{RingFocus, focus_marks, focus_marks_in, selection_ring};
use focus_ring::{focus_note, focus_ring, keyboard_focus, set_keyboard_focus};
pub use icons::Icon;
pub use jump::{HEIGHT as JUMP_HEIGHT, Jump};
pub use menu::Menu;
use open_menus::menu_opened;
pub use open_menus::{close_menus, menus_open};
pub use {
    checkbox::Checkbox,
    chip::Chip,
    fill::fill,
    flash::Flash,
    input::TextField,
    pressable::{Choices, Pressable},
    radio::RadioGroup,
    segmented::Segmented,
    slider::Slider,
    toast::Toast,
    toggle::Toggle,
    tooltip::Tooltip,
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
        // Inside an open dialog, Tab stays among its actions.
        KeyBinding::new("tab", dialog::NextAction, Some(dialog::KEY_CONTEXT)),
        KeyBinding::new(
            "shift-tab",
            dialog::PreviousAction,
            Some(dialog::KEY_CONTEXT),
        ),
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
