use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, ElementId, FocusHandle, KeyDownEvent, Pixels, Role, SharedString, Window,
    actions, div, prelude::*, px,
};

use super::{Button, Checkbox, PressHandler, Readable, close_menus, keyed_focus, menus_open};
use crate::theme::{ActiveTheme, ThemedText, scrim};

actions!(dialog, [NextAction, PreviousAction]);

/// `ui::init` binds Tab in it.
pub const KEY_CONTEXT: &str = "Dialog";

const WIDTH: f32 = 440.;
const PADDING: f32 = 22.;
const GAP: f32 = 14.;
const ACTION_GAP: f32 = 8.;
/// `.hint`'s `line-height: 1.45`.
const BODY_LEADING: f32 = 1.45;

const TICK_LABEL_GAP: f32 = 4.;

/// Runs before Esc is looked at; `true` means it was handled.
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App) -> bool>;

struct Action {
    label: SharedString,
    primary: bool,
    handler: PressHandler,
}

struct Tick {
    label: SharedString,
    on: bool,
    handler: PressHandler,
}

#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    body: SharedString,
    width: Pixels,
    content: Option<AnyElement>,
    content_focus: Vec<FocusHandle>,
    first_action_focus: FocusHandle,
    actions: Vec<Action>,
    on_key: Option<KeyHandler>,
    tick: Option<Tick>,
    cancel: PressHandler,
}

impl Dialog {
    /// `focus` goes on the first action added, the leftmost; focus it on opening so a keyboard
    /// user lands in the dialog (DESIGN.md §7). `cancel` runs on Esc, and should only close the
    /// dialog.
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
            first_action_focus: focus.clone(),
            actions: Vec::new(),
            on_key: None,
            tick: None,
            cancel: Rc::new(cancel),
        }
    }

    pub fn width(mut self, width: Pixels) -> Dialog {
        self.width = width;
        self
    }

    /// `focus` lists its controls' focus handles in tab order; each control must `track_focus`
    /// its own.
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

    pub fn tick(
        mut self,
        label: impl Into<SharedString>,
        on: bool,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        self.tick = Some(Tick {
            label: label.into(),
            on,
            handler: Rc::new(handler),
        });
        self
    }

    pub fn action(
        self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        self.push(label, false, handler)
    }

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
        // An open menu would paint above the backdrop and take focus from the dialog. The menus
        // drawn before the dialog in this frame have already read their open flags, and nothing
        // invalidated during a draw is redrawn, so they are closed as the next frame begins.
        if menus_open(cx) {
            window.on_next_frame(|_, cx| close_menus(cx));
        }
        let id = Arc::new(self.id.clone());
        let child_id = |name: &str| ElementId::NamedChild(id.clone(), name.to_string().into());
        let action_focus: Vec<FocusHandle> = (0..self.actions.len())
            .map(|index| match index {
                0 => self.first_action_focus.clone(),
                _ => keyed_focus(window, cx, &child_id(&format!("action {index}"))),
            })
            .collect();
        let tick_focus = self
            .tick
            .as_ref()
            .map(|_| keyed_focus(window, cx, &child_id("tick focus")));
        let mut stops = self.content_focus;
        stops.extend(tick_focus.iter().cloned());
        stops.extend(action_focus.iter().cloned());
        // Not a tab stop. When the focused control goes, as a Clear button does, GPUI restores
        // focus to the panel, and from there it moves to the first action.
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
        // The mockups show no dialog, so the panel borrows the menu's surface, border and shadow.
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

        let tick = self.tick.zip(tick_focus).map(|(tick, focus)| {
            let press = tick.handler.clone();
            let press_label = tick.handler;
            div()
                .flex()
                .items_center()
                .gap(px(TICK_LABEL_GAP))
                .child(
                    Checkbox::new(child_id("tick"), tick.label.clone(), tick.on)
                        .track_focus(&focus)
                        .on_press(move |_, window, cx| press(window, cx)),
                )
                .child(
                    div()
                        .id(child_id("tick-label"))
                        .type_style(theme, body_font)
                        .cursor_pointer()
                        .on_click(move |_, window, cx| press_label(window, cx))
                        .child(tick.label),
                )
        });

        let panel = div()
            .id(child_id("panel"))
            .role(Role::AlertDialog)
            .aria_label(self.title.clone())
            .aria_description(self.body.clone())
            .modal_dialog()
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
            .children(tick)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(ACTION_GAP))
                    .pt(px(GAP - ACTION_GAP))
                    .children(actions),
            );

        // Must be the window's last child, so it paints above the rest; it occludes them so they
        // take no clicks.
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
