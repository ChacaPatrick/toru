//! `.btn` and its variants, the bulk bar's `.bulk button` among them.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, App, ElementId, FocusHandle, Hsla, Pixels, Role, SharedString,
    StyleRefinement, Window, div, prelude::*, px,
};

use super::{
    Icon, PressHandler, RingFocus, Tooltip, focus_marks, focus_marks_in, keyed_focus, refined,
    shadowed,
};
use crate::theme::{ActiveTheme, Border, Theme, ThemedText, TypeStyle, contrast, hover_tint};

/// The mockups' CSS says .75, which leaves Game Boy green's sub at 4.4:1; .8 is 4.8:1 there and
/// reads the same (DESIGN.md §7). Blended into the background rather than drawn translucent, so a
/// split sub can go to the full text colour when hovered.
pub const SUB_OPACITY: f32 = 0.8;

/// `.btn` and `.btn.sm`.
pub const HEIGHT: f32 = 40.;
pub const SMALL_HEIGHT: f32 = 32.;
/// `.bulk button`.
pub const BULK_HEIGHT: f32 = 36.;
const BULK_PADDING: f32 = 12.;
/// `.btn{padding:0 16px}`.
const PADDING: f32 = 16.;
/// Also the sub's padding after its rule.
const CONTENT_GAP: f32 = 8.;
const SUB_RULE: f32 = 1.;
/// `.bulk .n{opacity:.7}`.
const NOTE_OPACITY: f32 = 0.7;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Primary,
    /// `main` is the bulk bar's one filled action.
    Bulk {
        main: bool,
    },
}

pub(super) struct MenuTrigger {
    pub focus: FocusHandle,
    pub open: bool,
    pub toggle: PressHandler,
}

/// UIA's ExpandCollapse pattern. Only the action that changes the state is offered; AccessKit asks
/// for no other.
fn expand_collapse<E: StatefulInteractiveElement>(
    element: E,
    open: bool,
    toggle: PressHandler,
) -> E {
    let action = if open {
        AccessibleAction::Collapse
    } else {
        AccessibleAction::Expand
    };
    element.on_a11y_action(action, move |_, window, cx| toggle(window, cx))
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    sub: Option<SharedString>,
    aria_label: Option<SharedString>,
    description: Option<SharedString>,
    kind: Kind,
    note: Option<SharedString>,
    small: bool,
    square: bool,
    check_mark: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    expanded: Option<bool>,
    focus: Option<FocusHandle>,
    tab_stop: Option<bool>,
    on_click: Option<PressHandler>,
    /// `Some` makes it a split button.
    sub_name: Option<SharedString>,
    sub_menu: Option<MenuTrigger>,
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
            description: None,
            kind: Kind::Plain,
            note: None,
            small: false,
            square: false,
            check_mark: false,
            disabled: false,
            tooltip: None,
            expanded: None,
            focus: None,
            tab_stop: None,
            on_click: None,
            sub_name: None,
            sub_menu: None,
            style: StyleRefinement::default(),
        }
    }

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

    pub fn named(mut self, name: impl Into<SharedString>) -> Button {
        self.aria_label = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Button {
        self.description = Some(description.into());
        self
    }

    pub fn icon(mut self, icon: Icon) -> Button {
        self.icon = Some(icon);
        self
    }

    pub fn sub(mut self, sub: impl Into<SharedString>) -> Button {
        self.sub = Some(sub.into());
        self
    }

    pub fn primary(mut self) -> Button {
        self.kind = Kind::Primary;
        self
    }

    pub fn bulk(mut self, main: bool) -> Button {
        self.kind = Kind::Bulk { main };
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Button {
        self.note = Some(note.into());
        self
    }

    pub fn small(mut self) -> Button {
        self.small = true;
        self
    }

    /// Laid over the hidden label, so the button keeps its width.
    pub fn check_mark(mut self, check_mark: bool) -> Button {
        self.check_mark = check_mark;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Button {
        self.disabled = disabled;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Button {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The icon alone, with any sub or note; the label becomes the name and tooltip.
    pub fn compact(mut self, compact: bool) -> Button {
        if compact && let Some(label) = self.label.take() {
            self.square = self.sub.is_none() && self.note.is_none();
            self.tooltip.get_or_insert(label.clone());
            self.aria_label.get_or_insert(label);
        }
        self
    }

    pub(super) fn expanded(mut self, expanded: bool) -> Button {
        self.expanded = Some(expanded);
        self
    }

    /// The sub becomes its own control, called `name`, and opens the `Menu` the button triggers,
    /// while `on_click` stays the button's own.
    pub fn split(mut self, name: impl Into<SharedString>) -> Button {
        self.sub_name = Some(name.into());
        self
    }

    /// A split button puts the menu on its sub, and pressing its label while the menu is open
    /// closes the menu too; any other button's click is replaced by the menu's.
    pub(super) fn menu_trigger(mut self, menu: MenuTrigger) -> Button {
        if self.sub_name.is_none() {
            let toggle = menu.toggle;
            return self
                .track_focus(&menu.focus)
                .expanded(menu.open)
                .on_click(move |window, cx| toggle(window, cx));
        }
        if menu.open
            && let Some(own) = self.on_click.take()
        {
            let toggle = menu.toggle.clone();
            self.on_click = Some(Rc::new(move |window, cx| {
                toggle(window, cx);
                own(window, cx);
            }));
        }
        self.sub_menu = Some(menu);
        self
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Button {
        self.tab_stop = Some(tab_stop);
        self
    }

    /// A `Menu` applies it to the trigger it focuses.
    pub(super) fn tab_stop_setting(&self) -> Option<bool> {
        self.tab_stop
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Button {
        self.focus = Some(focus.clone());
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Button {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn label_style(&self, theme: &Theme) -> TypeStyle {
        let scale = &theme.type_scale;
        match (self.kind, self.small) {
            (Kind::Bulk { .. }, _) => scale.bulk_button,
            (Kind::Primary, true) => scale.primary_button_sm,
            (Kind::Primary, false) => scale.primary_button,
            (Kind::Plain, true) => scale.button_sm,
            (Kind::Plain, false) => scale.button,
        }
    }

    /// Height and horizontal padding.
    fn box_size(&self, theme: &Theme) -> (Pixels, Pixels) {
        match self.kind {
            Kind::Bulk { .. } => (px(BULK_HEIGHT), px(BULK_PADDING)),
            _ if self.small => (px(SMALL_HEIGHT), theme.sizes.button_sm_padding),
            _ => (px(HEIGHT), px(PADDING)),
        }
    }

    fn border_width(&self, theme: &Theme) -> Pixels {
        match self.kind {
            Kind::Plain => theme.borders.button.width,
            Kind::Primary => theme.borders.accent.width,
            Kind::Bulk { .. } => px(0.),
        }
    }

    /// Before layout, for rows that choose what to show by what fits.
    pub fn width(&self, theme: &Theme, window: &Window) -> Pixels {
        let (height, padding) = self.box_size(theme);
        if self.square {
            return height;
        }
        let scale = &theme.type_scale;
        let text = |text: &SharedString, style: TypeStyle| style.text_width(text, theme, window);
        let parts = [
            self.icon.map(Icon::size),
            self.label
                .as_ref()
                .map(|label| text(label, self.label_style(theme))),
            self.sub
                .as_ref()
                .map(|sub| px(CONTENT_GAP + SUB_RULE) + text(sub, scale.button_sub)),
            self.note.as_ref().map(|note| text(note, scale.bulk_count)),
        ];
        let shown = parts.iter().flatten().count();
        let content = parts.iter().flatten().fold(px(0.), |sum, part| sum + *part);
        (self.border_width(theme) + padding) * 2.
            + content
            + px(CONTENT_GAP) * shown.saturating_sub(1) as f32
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut focus = self
            .focus
            .clone()
            .unwrap_or_else(|| keyed_focus(window, cx, &self.id));
        if let Some(tab_stop) = self.tab_stop {
            focus = focus.tab_stop(tab_stop);
        }
        let disabled = self.disabled;
        if disabled {
            focus = focus.tab_stop(false);
        }
        // One that held focus when it was turned off keeps it until the user moves on, so a
        // button that turns itself off, as Save all does, keeps the keyboard's place.
        let focusable = !disabled || focus.is_focused(window);
        let theme = cx.theme();
        let c = &theme.colors;
        let font = self.label_style(theme);
        let (height, padding) = self.box_size(theme);
        let none = Vec::new();
        let (bg, text, border, shadow, ring) = match self.kind {
            Kind::Plain => (
                c.button_bg,
                c.button_text,
                theme.borders.button,
                &theme.shadows.button,
                c.focus,
            ),
            Kind::Primary => (
                c.accent_bg,
                c.accent_text,
                theme.borders.accent,
                &theme.shadows.accent,
                c.focus,
            ),
            Kind::Bulk { main: true } => (
                c.bulk_main_bg,
                c.bulk_main_text,
                Border::none(),
                &none,
                c.bulk_focus,
            ),
            Kind::Bulk { main: false } => (
                gpui::transparent_black(),
                c.bulk_text,
                Border::none(),
                &none,
                c.bulk_focus,
            ),
        };
        // An empty slot in the look's placeholder outline, at the button's own border width so
        // turning off moves nothing.
        let (bg, text, border, shadow) = if disabled {
            let outline = Border {
                width: border.width,
                ..theme.borders.load
            };
            (gpui::transparent_black(), c.muted, outline, &none)
        } else {
            (bg, text, border, shadow)
        };
        // What lies under the text: the bar for an unfilled bulk button.
        let under = if bg.a == 0. { c.bulk_bg } else { bg };
        // The mockups' CSS gives the bulk bar's buttons no hover, and a borderless button would
        // otherwise show none.
        let bulk = matches!(self.kind, Kind::Bulk { .. });
        let hover_bg = contrast::over(hover_tint(text), under);
        let square = self.square;
        let split = self.sub_name.is_some();
        // The sub is part of the name, as a `<button>`'s text would be: "Delayed 5 s".
        let aria_label: SharedString = if self.check_mark {
            "Copied".into()
        } else {
            let name = self.aria_label.or(self.label.clone()).unwrap_or_default();
            match (&self.sub, &self.note) {
                (Some(sub), _) if !split => format!("{name} {}", sub.trim_end_matches(" ▾")).into(),
                (_, Some(note)) => format!("{name} {note}").into(),
                _ => name,
            }
        };
        let radius = theme.radii.md;
        let sub_font = theme.type_scale.button_sub;
        let sub_id = ElementId::NamedChild(Arc::new(self.id.clone()), "sub".into());

        let sub = self.sub.map(|sub| {
            // A disabled button has no fill to blend into.
            let sub_text = if disabled {
                text
            } else {
                contrast::over(
                    Hsla {
                        a: SUB_OPACITY,
                        ..text
                    },
                    bg,
                )
            };
            let sub = div()
                .pl(px(CONTENT_GAP))
                .border_l(px(SUB_RULE))
                .border_color(sub_text)
                .type_style(theme, sub_font)
                .child(sub_font.case.apply(&sub));
            let Some((name, menu)) = self.sub_name.zip(self.sub_menu) else {
                return sub.text_color(sub_text).into_any_element();
            };
            // As tall as the whole button and with its right padding, so its ring lines up with
            // the button's and its tint reaches the edge.
            let toggle = menu.toggle;
            let toggle_a11y = toggle.clone();
            let sub_button = div().id(sub_id);
            expand_collapse(sub_button, menu.open, toggle.clone())
                .role(Role::Button)
                .aria_label(name)
                .aria_expanded(menu.open)
                .track_ring_focus(&menu.focus)
                .relative()
                .h_full()
                .flex()
                .items_center()
                .pr(padding)
                .rounded_r(radius - border.width)
                .text_color(sub_text)
                .cursor_pointer()
                .hover(|style| style.bg(hover_tint(text)).text_color(text))
                .on_click(move |_, window, cx| {
                    // Not the button's press too.
                    cx.stop_propagation();
                    toggle(window, cx)
                })
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    toggle_a11y(window, cx)
                })
                .child(sub)
                .children(focus_marks(
                    menu.focus.is_focused(window),
                    theme,
                    radius,
                    border.width,
                    window,
                    cx,
                ))
                .into_any_element()
        });

        let note = self.note.map(|note| {
            let note_font = theme.type_scale.bulk_count;
            let note_text = if disabled {
                text
            } else {
                contrast::over(
                    Hsla {
                        a: NOTE_OPACITY,
                        ..text
                    },
                    under,
                )
            };
            div()
                .type_style(theme, note_font)
                .text_color(note_text)
                .child(note_font.case.apply(&note))
        });
        let content = div()
            .h_full()
            .flex()
            .items_center()
            .gap(px(CONTENT_GAP))
            .when(self.check_mark, |this| this.invisible())
            .children(self.icon.map(|icon| icon.render(theme, text)))
            .children(self.label.map(|label| font.case.apply(&label)))
            .children(sub)
            .children(note);

        // A menu trigger's click is the menu's toggle (`menu_trigger`).
        let menu_toggle = self
            .expanded
            .zip(self.on_click.clone())
            .filter(|_| !disabled);
        let button = div().id(self.id);
        let button = shadowed(button, shadow, radius)
            .role(Role::Button)
            .aria_label(aria_label)
            .when_some(self.description, |this, text| this.aria_description(text))
            .when(focusable, |this| this.track_ring_focus(&focus))
            .when_some(self.expanded, |this, expanded| this.aria_expanded(expanded))
            .when_some(menu_toggle, |this, (open, toggle)| {
                expand_collapse(this, open, toggle)
            })
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .h(height)
            .when(square, |this| this.w(height))
            .when(!square, |this| this.pl(padding))
            .when(!square && !split, |this| this.pr(padding))
            .bg(bg)
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .type_style(theme, font)
            .whitespace_nowrap()
            .when(border.dashed, |this| this.border_dashed())
            .when_some(self.tooltip, |this, tooltip| {
                this.tooltip(Tooltip::build(tooltip))
            })
            .when(disabled, |this| {
                this.a11y_synthetic_children(|builder| {
                    builder.parent_node().set_disabled();
                })
            })
            .when(!disabled, |this| this.cursor_pointer())
            .when(bulk && !disabled, |this| {
                this.hover(|style| style.bg(hover_bg))
            })
            .when_some(self.on_click.filter(|_| !disabled), |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |_, window, cx| {
                    // Not the card's beneath it too.
                    cx.stop_propagation();
                    handler(window, cx)
                })
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
            .children(focus_marks_in(
                focus.is_focused(window),
                ring,
                theme,
                radius,
                border.width,
                window,
                cx,
            ));
        refined(button, &self.style)
    }
}
