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
mod icons;
mod input;
mod jump;
mod menu;
mod pressable;
mod radio;
mod segmented;
mod slider;
mod toast;
mod toggle;
mod tooltip;

use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, AnyElement, App, Bounds, BoxShadow, ElementId, Entity, FocusHandle, Global,
    Hsla, KeyBinding, MouseDownEvent, MouseUpEvent, Pixels, Refineable, StyleRefinement,
    WeakEntity, Window, WindowId, canvas, div, prelude::*, px,
};

use crate::theme::Theme;

pub use a11y::{Readable, spoken};
pub use button::{
    BULK_HEIGHT as BULK_BUTTON_HEIGHT, Button, SMALL_HEIGHT as BUTTON_SMALL_HEIGHT, SUB_OPACITY,
};
pub use checkbox::SIZE as CHECKBOX_SIZE;
pub use dialog::Dialog;
pub use icons::Icon;
pub use jump::{HEIGHT as JUMP_HEIGHT, Jump};
pub use menu::Menu;
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

/// The open menus' open flags, so a dialog can close them all (`close_menus`).
#[derive(Default)]
struct OpenMenus(Vec<WeakEntity<bool>>);

impl Global for OpenMenus {}

/// Notes that the menu whose open flag this is has opened.
fn menu_opened(open: &Entity<bool>, cx: &mut App) {
    let mut menus = std::mem::take(&mut cx.default_global::<OpenMenus>().0);
    // Menus closed by their own means leave their entry behind until now.
    menus.retain(|menu| menu.upgrade().is_some_and(|menu| *menu.read(cx)));
    menus.push(open.downgrade());
    cx.default_global::<OpenMenus>().0 = menus;
}

/// Whether any menu is open.
pub fn menus_open(cx: &App) -> bool {
    cx.try_global::<OpenMenus>().is_some_and(|menus| {
        menus
            .0
            .iter()
            .any(|menu| menu.upgrade().is_some_and(|menu| *menu.read(cx)))
    })
}

/// Closes every open menu, without moving focus. A dialog does this as it opens: a menu would
/// paint above its backdrop, still take clicks, and put focus behind the dialog on a click
/// outside itself. Called between frames, not during a draw, so the menus are redrawn closed.
pub fn close_menus(cx: &mut App) {
    let menus = std::mem::take(&mut cx.default_global::<OpenMenus>().0);
    for menu in menus.iter().filter_map(WeakEntity::upgrade) {
        menu.update(cx, |open, cx| {
            if *open {
                *open = false;
                cx.notify();
            }
        });
    }
}

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
struct FocusRing(Option<(FocusHandle, Bounds<Pixels>)>);

impl Global for FocusRing {}

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

/// Records where the focused control's ring is or would be and, when the focus has moved since
/// the last note, asks for another frame so the control's container can scroll to it.
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

fn mouse_held(window: &Window, cx: &App) -> bool {
    cx.try_global::<MouseHeld>()
        .is_some_and(|held| held.0.contains(&window.window_handle().window_id()))
}

fn set_mouse_held(on: bool, window: &Window, cx: &mut App) {
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
const RING_WIDTH: f32 = 3.;
const RING_GAP: f32 = RING_WIDTH * 2.;

/// Where an absolute child reaching `offset` outside a control sits: such children are placed
/// from inside the control's `border`, so it is added back.
fn ring_inset(offset: f32, border: Pixels) -> Pixels {
    px(-offset) - border
}

/// The size of the outer ring, which is what must be in view, noted where it was drawn
/// (`FocusInView`) without painting anything.
fn focus_note(border: Pixels) -> AnyElement {
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
fn focus_ring(theme: &Theme, radius: Pixels, border: Pixels) -> [AnyElement; 2] {
    focus_ring_in(theme.colors.focus, theme, radius, border)
}

fn focus_ring_in(color: Hsla, theme: &Theme, radius: Pixels, border: Pixels) -> [AnyElement; 2] {
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
fn ring(offset: f32, color: Hsla, radius: Pixels, border: Pixels) -> AnyElement {
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
