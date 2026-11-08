//! `.btn` and its variants: `.primary`, `.sm`, `.icon`, the `.sub` secondary label, and the Copy
//! button's check mark, which is laid over the hidden label so the button keeps its width.

use std::rc::Rc;

use gpui::{
    AccessibleAction, App, ElementId, FocusHandle, Role, SharedString, StyleRefinement, Window,
    div, prelude::*, px,
};

use super::{Handler, Icon, focus_marks, keyed_focus, refined};
use crate::theme::{ActiveTheme, ThemedText};

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    sub: Option<SharedString>,
    aria_label: Option<SharedString>,
    primary: bool,
    small: bool,
    /// As wide as it is tall, with the content centred: icon-only and glyph-only buttons.
    square: bool,
    check_mark: bool,
    expanded: Option<bool>,
    focus: Option<FocusHandle>,
    on_click: Option<Handler>,
    style: StyleRefinement,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
        Button {
            id: id.into(),
            label: Some(label.into()),
            icon: None,
            sub: None,
            aria_label: None,
            primary: false,
            small: false,
            square: false,
            check_mark: false,
            expanded: None,
            focus: None,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    /// A square button showing only `icon`; `aria_label` is what it is called.
    pub fn icon_only(
        id: impl Into<ElementId>,
        icon: Icon,
        aria_label: impl Into<SharedString>,
    ) -> Button {
        let mut button = Button::new(id, "");
        button.label = None;
        button.icon = Some(icon);
        button.aria_label = Some(aria_label.into());
        button.square = true;
        button
    }

    /// A square button showing only `glyph`, such as `⋯`; `aria_label` is what it is called.
    pub fn glyph_only(
        id: impl Into<ElementId>,
        glyph: impl Into<SharedString>,
        aria_label: impl Into<SharedString>,
    ) -> Button {
        let mut button = Button::new(id, glyph);
        button.aria_label = Some(aria_label.into());
        button.square = true;
        button
    }

    pub fn icon(mut self, icon: Icon) -> Button {
        self.icon = Some(icon);
        self
    }

    /// The secondary part of the label, such as `5 s ▾`.
    pub fn sub(mut self, sub: impl Into<SharedString>) -> Button {
        self.sub = Some(sub.into());
        self
    }

    pub fn primary(mut self) -> Button {
        self.primary = true;
        self
    }

    pub fn small(mut self) -> Button {
        self.small = true;
        self
    }

    /// Shows a check mark instead of the label, as a Copy button does just after copying.
    pub fn check_mark(mut self, check_mark: bool) -> Button {
        self.check_mark = check_mark;
        self
    }

    /// Whether the menu this button opens is open.
    pub(super) fn expanded(mut self, expanded: bool) -> Button {
        self.expanded = Some(expanded);
        self
    }

    /// Use `focus` instead of the button's own handle, to focus it from elsewhere.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Button {
        self.focus = Some(focus.clone());
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Button {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self
            .focus
            .unwrap_or_else(|| keyed_focus(window, cx, &self.id));
        let theme = cx.theme();
        let c = &theme.colors;
        let (bg, text, border, shadow) = if self.primary {
            (
                c.accent_bg,
                c.accent_text,
                theme.borders.accent,
                &theme.shadows.accent,
            )
        } else {
            (
                c.button_bg,
                c.button_text,
                theme.borders.button,
                &theme.shadows.button,
            )
        };
        let scale = &theme.type_scale;
        let font = match (self.primary, self.small) {
            (true, true) => scale.primary_button_sm,
            (true, false) => scale.primary_button,
            (false, true) => scale.button_sm,
            (false, false) => scale.button,
        };
        let (height, padding) = if self.small {
            (px(32.), theme.sizes.button_sm_padding)
        } else {
            (px(40.), px(16.))
        };
        let square = self.square;
        // The sub is part of the name, as a `<button>`'s text would be: "Delayed 5 s".
        let aria_label: SharedString = if self.check_mark {
            "Copied".into()
        } else {
            let name = self.aria_label.or(self.label.clone()).unwrap_or_default();
            match &self.sub {
                Some(sub) => format!("{name} {}", sub.trim_end_matches(" ▾")).into(),
                None => name,
            }
        };
        let radius = theme.radii.md;
        let sub_font = theme.type_scale.button_sub;

        let content = div()
            .flex()
            .items_center()
            .gap_2()
            .when(self.check_mark, |this| this.invisible())
            .children(self.icon.map(|icon| icon.render(theme, text)))
            .children(self.label.map(|label| font.case.apply(&label)))
            .children(self.sub.map(|sub| {
                div()
                    .pl_2()
                    .border_l_1()
                    .border_color(text)
                    .opacity(0.75)
                    .type_style(theme, sub_font)
                    .child(sub_font.case.apply(&sub))
            }));

        let button = div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(aria_label)
            .track_focus(&focus)
            .when_some(self.expanded, |this, expanded| this.aria_expanded(expanded))
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .h(height)
            .when(square, |this| this.w(height))
            .when(!square, |this| this.px(padding))
            .bg(bg)
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .shadow(shadow.clone())
            .type_style(theme, font)
            .whitespace_nowrap()
            .cursor_pointer()
            .when_some(self.on_click, |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |_, window, cx| handler(window, cx))
                    // A screen reader's click lands at the control's centre, which can be covered.
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        for_a11y(window, cx)
                    })
            })
            .child(content)
            .when(self.check_mark, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::Check.render(theme, text)),
                )
            })
            .children(focus_marks(
                focus.is_focused(window),
                theme,
                radius,
                border.width,
                window,
                cx,
            ));
        refined(button, &self.style)
    }
}
