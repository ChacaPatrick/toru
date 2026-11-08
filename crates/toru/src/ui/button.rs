//! `.btn` and its variants: `.primary`, `.sm`, `.icon`, the `.sub` secondary label, and the Copy
//! button's check mark, which is laid over the hidden label so the button keeps its width. The
//! bulk bar's buttons (`.bulk button`) are a variant too: borderless on the bar's colour, with a
//! small `.n` note such as a count after the label, and the ring in `bulk_focus`.
//!
//! A button with a sub can be split: it keeps the look of one button, but the sub is a second
//! control, so pressing the label does the button's own work and pressing the sub opens a `Menu`.

use std::{rc::Rc, sync::Arc};

use gpui::{
    AccessibleAction, App, ElementId, FocusHandle, Hsla, Role, SharedString, StyleRefinement,
    Window, div, prelude::*, px,
};

use super::{Handler, Icon, focus_marks, focus_marks_in, keyed_focus, refined, shadowed};
use crate::theme::{ActiveTheme, Border, ThemedText, contrast, hover_tint};

/// How much of the button's text colour the `.sub` label keeps. The mockups' CSS says .75, which
/// leaves Game Boy green's sub at 4.4:1; .8 is 4.8:1 there and reads the same (DESIGN.md §7).
/// It is blended into the button's background rather than drawn translucent, so a split sub can
/// go to the full text colour when hovered.
pub const SUB_OPACITY: f32 = 0.8;

/// `.btn` and `.btn.sm` heights, for layouts that must know a row's height before it is laid out.
pub const HEIGHT: f32 = 40.;
pub const SMALL_HEIGHT: f32 = 32.;
/// `.bulk button`'s height and horizontal padding.
pub const BULK_HEIGHT: f32 = 36.;
const BULK_PADDING: f32 = 12.;
/// How much of the bar's text colour a bulk note keeps (`.bulk .n{opacity:.7}`).
const NOTE_OPACITY: f32 = 0.7;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Primary,
    /// On the bulk bar; `main` is the bar's one filled action.
    Bulk {
        main: bool,
    },
}

/// What a `Menu` hangs on its trigger: the trigger's focus, whether the menu is open, and the
/// press that opens or closes it.
pub(super) struct MenuTrigger {
    pub focus: FocusHandle,
    pub open: bool,
    pub toggle: Handler,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    sub: Option<SharedString>,
    aria_label: Option<SharedString>,
    kind: Kind,
    /// The small note after the label, such as the count a bulk action covers.
    note: Option<SharedString>,
    small: bool,
    /// As wide as it is tall, with the content centred: icon-only and glyph-only buttons.
    square: bool,
    check_mark: bool,
    expanded: Option<bool>,
    focus: Option<FocusHandle>,
    /// `Some(false)` keeps the button out of the tab order (`tab_stop`).
    tab_stop: Option<bool>,
    on_click: Option<Handler>,
    /// `Some` for a split button: what the sub is called for assistive technology.
    sub_name: Option<SharedString>,
    /// The menu a split button's sub opens.
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
            kind: Kind::Plain,
            note: None,
            small: false,
            square: false,
            check_mark: false,
            expanded: None,
            focus: None,
            tab_stop: None,
            on_click: None,
            sub_name: None,
            sub_menu: None,
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
        self.kind = Kind::Primary;
        self
    }

    /// A bulk bar button; `main` fills it as the bar's one main action.
    pub fn bulk(mut self, main: bool) -> Button {
        self.kind = Kind::Bulk { main };
        self
    }

    /// The small note after the label: "(2)" or "(1 new)".
    pub fn note(mut self, note: impl Into<SharedString>) -> Button {
        self.note = Some(note.into());
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

    /// Splits the button: the sub becomes its own control, called `name` for assistive
    /// technology, and opens the `Menu` the button is the trigger of, while `on_click` stays the
    /// button's own.
    pub fn split(mut self, name: impl Into<SharedString>) -> Button {
        self.sub_name = Some(name.into());
        self
    }

    /// Makes the button a `Menu`'s trigger. A split button puts the menu on its sub, and pressing
    /// its label while the menu is open closes the menu too; any other button's click is
    /// replaced by the menu's.
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

    /// Whether Tab reaches the button. A card's buttons are tab stops only while the card is the
    /// gallery's current one, so Tab doesn't walk every control of every card (DESIGN.md §7).
    pub fn tab_stop(mut self, tab_stop: bool) -> Button {
        self.tab_stop = Some(tab_stop);
        self
    }

    /// Whether `tab_stop` was set, and to what; a `Menu` applies it to the trigger it focuses.
    pub(super) fn tab_stop_setting(&self) -> Option<bool> {
        self.tab_stop
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
        let mut focus = self
            .focus
            .unwrap_or_else(|| keyed_focus(window, cx, &self.id));
        if let Some(tab_stop) = self.tab_stop {
            focus = focus.tab_stop(tab_stop);
        }
        let theme = cx.theme();
        let c = &theme.colors;
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
        let scale = &theme.type_scale;
        let font = match (self.kind, self.small) {
            (Kind::Bulk { .. }, _) => scale.bulk_button,
            (Kind::Primary, true) => scale.primary_button_sm,
            (Kind::Primary, false) => scale.primary_button,
            (Kind::Plain, true) => scale.button_sm,
            (Kind::Plain, false) => scale.button,
        };
        let (height, padding) = match self.kind {
            Kind::Bulk { .. } => (px(BULK_HEIGHT), px(BULK_PADDING)),
            _ if self.small => (px(SMALL_HEIGHT), theme.sizes.button_sm_padding),
            _ => (px(HEIGHT), px(16.)),
        };
        // What lies under the text: the bar for an unfilled bulk button.
        let under = if bg.a == 0. { c.bulk_bg } else { bg };
        // A bulk button's hover is a tint of its text, as the jump pill's is: the mockups' CSS
        // gives the bar's buttons no hover, and a borderless button would otherwise show none.
        let bulk = matches!(self.kind, Kind::Bulk { .. });
        let hover_bg = contrast::over(hover_tint(text), under);
        let square = self.square;
        let split = self.sub_name.is_some();
        // The sub is part of the name, as a `<button>`'s text would be: "Delayed 5 s". A split
        // button's sub has its own name.
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
            let sub_text = contrast::over(
                Hsla {
                    a: SUB_OPACITY,
                    ..text
                },
                bg,
            );
            let sub = div()
                .pl_2()
                .border_l_1()
                .border_color(sub_text)
                .type_style(theme, sub_font)
                .child(sub_font.case.apply(&sub));
            let Some((name, menu)) = self.sub_name.zip(self.sub_menu) else {
                return sub.text_color(sub_text).into_any_element();
            };
            // The split sub: a button of its own, as tall as the whole one, with the button's
            // right padding, so its ring lines up with the button's and its tint reaches the edge,
            // rounded like the inside of the button's border so it stays within the outline.
            // Hovered, it shows as its own target: its text at full strength over a tint of it.
            let toggle = menu.toggle;
            let toggle_a11y = toggle.clone();
            div()
                .id(sub_id)
                .role(Role::Button)
                .aria_label(name)
                .aria_expanded(menu.open)
                .track_focus(&menu.focus)
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
                    // The press is the sub's alone, not the button's.
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
            let note_text = contrast::over(
                Hsla {
                    a: NOTE_OPACITY,
                    ..text
                },
                under,
            );
            div()
                .type_style(theme, note_font)
                .text_color(note_text)
                .child(note_font.case.apply(&note))
        });
        let content = div()
            .h_full()
            .flex()
            .items_center()
            .gap_2()
            .when(self.check_mark, |this| this.invisible())
            .children(self.icon.map(|icon| icon.render(theme, text)))
            .children(self.label.map(|label| font.case.apply(&label)))
            .children(sub)
            .children(note);

        let button = div().id(self.id);
        let button = shadowed(button, shadow, radius)
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
            .when(!square, |this| this.pl(padding))
            .when(!square && !split, |this| this.pr(padding))
            .bg(bg)
            .text_color(text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .type_style(theme, font)
            .whitespace_nowrap()
            .cursor_pointer()
            .when(bulk, |this| this.hover(|style| style.bg(hover_bg)))
            .when_some(self.on_click, |this, handler| {
                let for_a11y = handler.clone();
                this.on_click(move |_, window, cx| {
                    // The press is the button's alone, not the card's beneath it.
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
