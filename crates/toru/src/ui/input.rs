//! `.name` and `.field`: `gpui-kit`'s `Input` restyled to the theme. The kit's own focus border
//! is off (it is a 1px edge; `docs/spikes.md` §1.3), and the §7 ring is drawn around it instead.
//! A text field shows the ring whenever it has focus, as a browser does, and a click anywhere in
//! it puts the caret in the text: the kit's frame is focusable on its own, so a click on the
//! padding would otherwise focus the frame and leave the text deaf to typing.

use gpui::{App, Entity, Focusable, SharedString, StyleRefinement, Window, div, prelude::*, px};
use gpui_kit::component::input::{Input, InputState};

use super::{focus_note, focus_ring, refined};
use crate::theme::{ActiveTheme, ThemedText};

#[derive(IntoElement)]
pub struct TextField {
    state: Entity<InputState>,
    aria_label: SharedString,
    height: gpui::Pixels,
    tab_stop: bool,
    /// Sizing for the field as a whole, such as a width or `flex_1`.
    style: StyleRefinement,
}

impl TextField {
    /// `aria_label` is what the field is called, such as "Screenshot name".
    pub fn new(state: &Entity<InputState>, aria_label: impl Into<SharedString>) -> TextField {
        TextField {
            state: state.clone(),
            aria_label: aria_label.into(),
            height: px(34.),
            tab_stop: true,
            style: StyleRefinement::default(),
        }
    }

    /// The mockups use 34px for names, 36px for the folder field and 32px for search.
    pub fn height(mut self, height: gpui::Pixels) -> TextField {
        self.height = height;
        self
    }

    /// Whether Tab reaches the field; see `Button::tab_stop`.
    pub fn tab_stop(mut self, tab_stop: bool) -> TextField {
        self.tab_stop = tab_stop;
        self
    }
}

impl Styled for TextField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let border = theme.borders.input;
        let radius = theme.radii.md;
        // The kit's text element clones the state's handle when it renders, so the flag set on
        // the shared entry here is what it registers.
        let focused = self
            .state
            .focus_handle(cx)
            .tab_stop(self.tab_stop)
            .is_focused(window);

        let mut input = Input::new(&self.state)
            .aria_label(self.aria_label)
            .focus_bordered(false)
            .w_full()
            .px(px(10.))
            .bg(c.input_bg)
            .text_color(c.text)
            .border(border.width)
            .border_color(border.color)
            .rounded(radius)
            .type_style(theme, theme.type_scale.field);
        // `Input::h` is the kit's own method for multi-line fields; the frame's height is a style.
        Styled::style(&mut input).size.height = Some(self.height.into());

        let state = self.state;
        let field = div()
            .relative()
            .flex_none()
            // Runs after the frame has taken focus for itself, and hands it to the text.
            .on_any_mouse_down(move |_, window, cx| {
                state.update(cx, |state, cx| state.focus(window, cx))
            })
            .child(input)
            .when(focused, |this| {
                this.children(focus_ring(theme, radius, px(0.)))
                    .child(focus_note(px(0.)))
            });
        refined(field, &self.style)
    }
}
