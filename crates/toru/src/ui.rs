//! The shared controls, following the CSS in `design/build-mockups.py`.

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

pub type PressHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// DESIGN.md §7. A control drawn smaller, such as a radio mark, sits in a row this tall that
/// takes the press.
pub const MIN_TARGET: f32 = 32.;

/// Call once, after `gpui_kit::init`, so these bindings are checked first.
pub fn init(cx: &mut App) {
    cx.intercept_keystrokes(|_, window, cx| set_keyboard_focus(true, window, cx))
        .detach();
    cx.bind_keys([
        KeyBinding::new("tab", menu::NextItem, Some(menu::KEY_CONTEXT)),
        KeyBinding::new("shift-tab", menu::PreviousItem, Some(menu::KEY_CONTEXT)),
        KeyBinding::new("tab", dialog::NextAction, Some(dialog::KEY_CONTEXT)),
        KeyBinding::new(
            "shift-tab",
            dialog::PreviousAction,
            Some(dialog::KEY_CONTEXT),
        ),
    ]);
}

/// Lives as long as the control with `id` keeps being rendered, so callers needn't own one.
fn keyed_focus(window: &mut Window, cx: &mut App, id: &ElementId) -> FocusHandle {
    let key = ElementId::NamedChild(Arc::new(id.clone()), "focus".into());
    window
        .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

/// GPUI 0.3.11 measures inset shadows from the border box and paints the border over them, hiding
/// a band no wider than the border, so inset shadows go on an overlay child inside the border.
/// Call it before adding the content.
pub fn shadowed<E: Styled + ParentElement>(element: E, shadows: &[BoxShadow], radius: Pixels) -> E {
    let (inset, drop): (Vec<BoxShadow>, Vec<BoxShadow>) =
        shadows.iter().cloned().partition(|shadow| shadow.inset);
    let mut element = element.shadow(drop);
    if !inset.is_empty() {
        element = element.child(div().absolute().inset_0().rounded(radius).shadow(inset));
    }
    element
}

fn arrow_target(key: &str, selected: usize, count: usize) -> Option<usize> {
    Some(match key {
        "left" | "up" => (selected + count - 1) % count,
        "right" | "down" => (selected + 1) % count,
        "home" => 0,
        "end" => count - 1,
        _ => return None,
    })
}

fn refined<E: Styled>(mut element: E, style: &StyleRefinement) -> E {
    element.style().refine(style);
    element
}
