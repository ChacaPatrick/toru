//! The title bar as the mockups draw it: the look's dot, the title, and the minimize, maximize
//! and close controls. Windows does the work. The title is a `WindowControlArea::Drag`, so it
//! moves the window and a double-click maximizes it; the controls are the `Min`, `Max` and `Close`
//! areas, so they get the system's clicks and the snap layouts on hovering maximize. GPUI maps
//! the areas to the caption hit-test codes when the window is opened with
//! `TitlebarOptions::appears_transparent`.

use gpui::{
    AccessibleAction, App, Role, SharedString, Window, WindowControlArea, div, prelude::*, px,
};

use crate::platform;
use crate::theme::{ActiveTheme, Theme, ThemedText, hover_tint};

const HEIGHT: f32 = 38.;
const CONTROL_WIDTH: f32 = 46.;
const PADDING_LEFT: f32 = 14.;
const GAP: f32 = 10.;

pub fn title_bar(title: SharedString, window: &Window, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let style = theme.type_scale.title_bar;
    let maximized = window.is_maximized();
    let (maximize_label, maximize_glyph) = if maximized {
        ("Restore", "❐")
    } else {
        ("Maximize", "▢")
    };

    let dot = div()
        .flex_none()
        .size(theme.icons.dot)
        .rounded(theme.radii.dot)
        .bg(c.dot);

    // The drag area is a sibling of the controls, not their parent: GPUI answers the system's
    // hit test with the first control area painted under the mouse, and a parent paints first.
    div()
        .flex_none()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .bg(c.title_bg)
        .text_color(c.title_text)
        .border_b(hair.width)
        .border_color(hair.color)
        .child(
            div()
                .id("title-bar")
                .window_control_area(WindowControlArea::Drag)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap(px(GAP))
                .pl(px(PADDING_LEFT))
                .min_w_0()
                .child(dot)
                .child(
                    div()
                        .type_style(theme, style)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(style.case.apply(&title)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .h_full()
                .child(control(
                    "minimize",
                    "Minimize",
                    "–",
                    WindowControlArea::Min,
                    theme,
                    |window, _| window.minimize_window(),
                ))
                .child(control(
                    "maximize",
                    maximize_label,
                    maximize_glyph,
                    WindowControlArea::Max,
                    theme,
                    // GPUI's zoom only maximizes, so restoring is the platform module's.
                    |window, cx| {
                        if window.is_maximized() {
                            platform::restore_window(window, cx)
                        } else {
                            window.zoom_window()
                        }
                    },
                ))
                .child(control(
                    "close",
                    "Close",
                    "✕",
                    WindowControlArea::Close,
                    theme,
                    |window, _| window.remove_window(),
                )),
        )
}

/// One window control. The mouse reaches it through Windows, via its control area; `action` is
/// for assistive technology, whose click is simulated inside GPUI and never reaches the caption.
/// Hovered, it takes the bar's text colour, tinted behind it and on its glyph.
fn control(
    id: &'static str,
    label: &'static str,
    glyph: &'static str,
    area: WindowControlArea,
    theme: &Theme,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let divider = theme.borders.title_divider;
    let mark = match theme.sizes.window_control_square {
        Some(side) => div().size(side).bg(c.controls),
        None => div()
            .type_style(theme, theme.type_scale.controls)
            .child(glyph),
    };
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .window_control_area(area)
        .flex_none()
        .w(px(CONTROL_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .border_l(divider.width)
        .border_color(divider.color)
        .text_color(c.controls)
        .hover(|style| style.bg(hover_tint(c.title_text)).text_color(c.title_text))
        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            action(window, cx)
        })
        .child(mark)
}
