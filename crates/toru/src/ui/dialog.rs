//! A modal dialog: a backdrop over the whole window and a panel with a title, a line of
//! explanation, any content the caller adds, and a row of actions. The mockups show none, so the
//! panel borrows the menu's surface, border and shadow. While it is open the window behind takes
//! no clicks and no keys, so its own shortcuts rest; Tab and Shift+Tab stay among the content's
//! controls and the actions, Esc cancels, and Enter or Space presses the focused control. The
//! caller owns the first action's focus
//! handle, and the content's, and focuses one when the dialog opens, so a keyboard user lands in
//! the dialog straight away (DESIGN.md §7). Focus stays in the dialog when the control holding it
//! goes, as a Clear button does with what it cleared: the panel itself can take focus, so GPUI
//! restores focus to it, and from there it moves to the first action. Any menu open when the
//! dialog comes up is closed,
//! since it would paint above the backdrop and take focus from the dialog; a dialog holds no
//! menus of its own.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, ElementId, FocusHandle, KeyDownEvent, Pixels, Role, SharedString, Window,
    actions, div, prelude::*, px,
};

use super::{Button, Handler, close_menus, keyed_focus, menus_open};
use crate::theme::{ActiveTheme, ThemedText, scrim};

actions!(dialog, [NextAction, PreviousAction]);

/// The key context of an open dialog's panel, which `ui::init` binds Tab in.
pub const KEY_CONTEXT: &str = "Dialog";

const WIDTH: f32 = 440.;
const PADDING: f32 = 22.;
/// Between the title, the body, the content and the actions.
const GAP: f32 = 14.;
const ACTION_GAP: f32 = 8.;
/// `.hint`'s `line-height: 1.45`, so a two-line body reads easily.
const BODY_LEADING: f32 = 1.45;

/// A key press in the dialog, before Esc is looked at. `true` means it was handled.
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App) -> bool>;

struct Action {
    label: SharedString,
    primary: bool,
    handler: Handler,
}

#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    body: SharedString,
    width: Pixels,
    /// Between the body and the actions.
    content: Option<AnyElement>,
    /// The content's tab stops, in order, which Tab visits before the actions.
    content_focus: Vec<FocusHandle>,
    /// The first action's focus, owned by the caller.
    focus: FocusHandle,
    actions: Vec<Action>,
    on_key: Option<KeyHandler>,
    /// What Esc does.
    cancel: Handler,
}

impl Dialog {
    /// `focus` goes on the first action added, which is also the leftmost; focus it on opening.
    /// `cancel` runs on Esc, and should close the dialog without doing anything else.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        focus: &FocusHandle,
        cancel: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        Dialog {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            width: px(WIDTH),
            content: None,
            content_focus: Vec::new(),
            focus: focus.clone(),
            actions: Vec::new(),
            on_key: None,
            cancel: Rc::new(cancel),
        }
    }

    pub fn width(mut self, width: Pixels) -> Dialog {
        self.width = width;
        self
    }

    /// Content between the body and the actions. `focus` lists its controls' focus handles in
    /// tab order; each control must `track_focus` its own.
    pub fn content(mut self, content: impl IntoElement, focus: Vec<FocusHandle>) -> Dialog {
        self.content = Some(content.into_any_element());
        self.content_focus = focus;
        self
    }

    /// Sees every key pressed in the dialog first. Returning `true` ends the key's handling, so
    /// a dialog recording a key combination can take Esc and Tab too.
    pub fn on_key(
        mut self,
        handler: impl Fn(&KeyDownEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Dialog {
        self.on_key = Some(Rc::new(handler));
        self
    }

    pub fn action(
        self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        self.push(label, false, handler)
    }

    /// The action drawn as the primary button.
    pub fn primary_action(
        self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        self.push(label, true, handler)
    }

    fn push(
        mut self,
        label: impl Into<SharedString>,
        primary: bool,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        self.actions.push(Action {
            label: label.into(),
            primary,
            handler: Rc::new(handler),
        });
        self
    }
}

/// Tab and Shift+Tab: moves focus `step` stops on from the focused one, wrapping at either end.
fn step_focus(stops: &[FocusHandle], step: isize, window: &mut Window, cx: &mut App) {
    let count = stops.len() as isize;
    let current = stops
        .iter()
        .position(|focus| focus.is_focused(window))
        .unwrap_or(0) as isize;
    let next = (current + step).rem_euclid(count) as usize;
    stops[next].focus(window, cx);
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The menus drawn before the dialog in this frame have already read their open flags,
        // and nothing invalidated during a draw is redrawn, so they are closed as the next frame
        // begins, which then draws the dialog without them. Once closed, nothing is scheduled.
        if menus_open(cx) {
            window.on_next_frame(|_, cx| close_menus(cx));
        }
        let id = Arc::new(self.id.clone());
        let child_id = |name: &str| ElementId::NamedChild(id.clone(), name.to_string().into());
        let action_focus: Vec<FocusHandle> = (0..self.actions.len())
            .map(|index| match index {
                0 => self.focus.clone(),
                _ => keyed_focus(window, cx, &child_id(&format!("action {index}"))),
            })
            .collect();
        let mut stops = self.content_focus;
        stops.extend(action_focus.iter().cloned());
        // Not a tab stop: it only holds focus for the moment between a control going and the
        // first action taking over.
        let panel_focus = window
            .use_keyed_state(child_id("panel focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        if panel_focus.is_focused(window) {
            let first = action_focus[0].clone();
            window.defer(cx, move |window, cx| first.focus(window, cx));
        }

        let theme = cx.theme();
        let c = &theme.colors;
        let pop = theme.borders.pop;
        let body_font = theme.type_scale.body;

        // Every key stops here: the controls inside the panel have had theirs by now, and the
        // window's own key handling, Ctrl+A or Esc in select mode, must not act behind a modal.
        // The dialog's Tab bindings run before key-down listeners, so they are unaffected.
        let (cancel, on_key) = (self.cancel, self.on_key);
        let on_key_down = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            let taken = on_key
                .as_ref()
                .is_some_and(|on_key| on_key(event, window, cx));
            if !taken && event.keystroke.key == "escape" {
                cancel(window, cx);
            }
        };

        let actions = self
            .actions
            .into_iter()
            .zip(&action_focus)
            .map(|(action, focus)| {
                let handler = action.handler;
                Button::new(child_id(&action.label), action.label)
                    .when(action.primary, Button::primary)
                    .track_focus(focus)
                    .on_click(move |window, cx| handler(window, cx))
            });

        let panel = div()
            .id(child_id("panel"))
            .role(Role::AlertDialog)
            .aria_label(self.title.clone())
            .track_focus(&panel_focus)
            .key_context(KEY_CONTEXT)
            .on_key_down(on_key_down)
            .on_action({
                let stops = stops.clone();
                move |_: &NextAction, window, cx| step_focus(&stops, 1, window, cx)
            })
            .on_action({
                let stops = stops.clone();
                move |_: &PreviousAction, window, cx| step_focus(&stops, -1, window, cx)
            })
            .flex()
            .flex_col()
            .gap(px(GAP))
            .w(self.width)
            .max_w_full()
            .max_h_full()
            .p(px(PADDING))
            .bg(c.surface)
            .text_color(c.text)
            .border(pop.width)
            .border_color(pop.color)
            .rounded(theme.radii.lg)
            .shadow(theme.shadows.pop.clone())
            .child(
                div()
                    .type_style(theme, theme.type_scale.section)
                    .child(theme.type_scale.section.case.apply(&self.title)),
            )
            .child(
                div()
                    .type_style(theme, body_font)
                    .line_height(body_font.size * BODY_LEADING)
                    .child(self.body),
            )
            .children(self.content)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(ACTION_GAP))
                    .pt(px(GAP - ACTION_GAP))
                    .children(actions),
            );

        // Laid over the whole window, last among its children so it paints above them, and
        // occluding them so they take no clicks.
        div()
            .id(child_id("scrim"))
            .absolute()
            .inset_0()
            .occlude()
            .bg(scrim(c.text))
            .flex()
            .items_center()
            .justify_center()
            .p(px(PADDING))
            .child(panel)
    }
}
