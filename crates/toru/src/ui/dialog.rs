//! A modal confirmation: a backdrop over the whole window and a panel with a title, a line of
//! explanation and a row of actions. The mockups show none, so the panel borrows the menu's
//! surface, border and shadow. While it is open the window behind takes no clicks, Tab and
//! Shift+Tab stay among its actions, Esc cancels, and Enter or Space presses the focused action.
//! The caller owns the first action's focus handle and focuses it when the dialog opens, so a
//! keyboard user lands on it straight away (DESIGN.md §7). Any menu open when the dialog comes up
//! is closed, since it would paint above the backdrop and take focus from the dialog; a dialog
//! holds no menus of its own.

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, ElementId, FocusHandle, KeyDownEvent, Role, SharedString, Window, actions, div,
    prelude::*, px,
};

use super::{Button, Handler, close_menus, keyed_focus, menus_open};
use crate::theme::{ActiveTheme, ThemedText, scrim};

actions!(dialog, [NextAction, PreviousAction]);

/// The key context of an open dialog's panel, which `ui::init` binds Tab in.
pub const KEY_CONTEXT: &str = "Dialog";

const WIDTH: f32 = 440.;
const PADDING: f32 = 22.;
/// Between the title, the body and the actions.
const GAP: f32 = 14.;
const ACTION_GAP: f32 = 8.;
/// `.hint`'s `line-height: 1.45`, so a two-line body reads easily.
const BODY_LEADING: f32 = 1.45;

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
    /// The first action's focus, owned by the caller.
    focus: FocusHandle,
    actions: Vec<Action>,
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
            focus: focus.clone(),
            actions: Vec::new(),
            cancel: Rc::new(cancel),
        }
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

/// Tab and Shift+Tab: moves focus `step` actions on from the focused one, wrapping at either end.
fn step_focus(actions: &[FocusHandle], step: isize, window: &mut Window, cx: &mut App) {
    let count = actions.len() as isize;
    let current = actions
        .iter()
        .position(|focus| focus.is_focused(window))
        .unwrap_or(0) as isize;
    let next = (current + step).rem_euclid(count) as usize;
    actions[next].focus(window, cx);
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
        let focus: Vec<FocusHandle> = (0..self.actions.len())
            .map(|index| match index {
                0 => self.focus.clone(),
                _ => keyed_focus(window, cx, &child_id(&format!("action {index}"))),
            })
            .collect();

        let theme = cx.theme();
        let c = &theme.colors;
        let pop = theme.borders.pop;
        let body_font = theme.type_scale.body;

        let cancel = self.cancel;
        let on_key_down = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                cancel(window, cx);
            }
        };

        let actions = self.actions.into_iter().zip(&focus).map(|(action, focus)| {
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
            .key_context(KEY_CONTEXT)
            .on_key_down(on_key_down)
            .on_action({
                let focus = focus.clone();
                move |_: &NextAction, window, cx| step_focus(&focus, 1, window, cx)
            })
            .on_action({
                let focus = focus.clone();
                move |_: &PreviousAction, window, cx| step_focus(&focus, -1, window, cx)
            })
            .flex()
            .flex_col()
            .gap(px(GAP))
            .w(px(WIDTH))
            .max_w_full()
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
