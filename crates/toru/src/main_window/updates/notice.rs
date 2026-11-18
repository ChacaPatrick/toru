use gpui::{Context, Div, Role, accesskit::Live, div, prelude::*, px};

use super::NOTICE_HINT;
use crate::main_window::{MainWindow, act, settings::hint};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::Button;

const NOTICE_PADDING_X: f32 = 16.;

const NOTICE_PADDING_Y: f32 = 8.;

const NOTICE_GAP: f32 = 10.;

const NOTICE_BUTTON_GAP: f32 = 8.;

/// A row in the toolbar's colours, so it covers nothing and the gallery only starts lower.
/// Announced like a toast when it appears.
pub(in crate::main_window) fn notice(this: &MainWindow, cx: &Context<MainWindow>) -> Option<Div> {
    let update = this
        .updates
        .ready
        .as_ref()
        .filter(|_| !this.updates.dismissed)?;
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let message = format!("Toru {} is ready", update.version);
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(NOTICE_GAP))
            .px(px(NOTICE_PADDING_X))
            .py(px(NOTICE_PADDING_Y))
            .bg(c.surface_2)
            .border_b(hair.width)
            .border_color(hair.color)
            .child(
                div()
                    .flex_none()
                    .size(theme.icons.dot)
                    .rounded(theme.radii.dot)
                    .bg(c.accent),
            )
            .child(
                div()
                    .id("update-ready")
                    .role(Role::Status)
                    .aria_label(format!("{message}. {NOTICE_HINT}"))
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(Live::Polite);
                    })
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_baseline()
                    .gap(px(NOTICE_GAP))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .type_style(theme, theme.type_scale.setting)
                            .child(message),
                    )
                    .child(
                        hint(theme, NOTICE_HINT)
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis(),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(NOTICE_BUTTON_GAP))
                    .child(
                        Button::new("whats-new", "What's new")
                            .small()
                            .on_click(act(cx, |this, cx| this.open_whats_new(cx))),
                    )
                    .child(
                        Button::new("update-later", "Later")
                            .named("Update later")
                            .small()
                            .on_click(act(cx, |this, cx| this.dismiss_update(cx))),
                    )
                    .child(
                        Button::new("update-now", "Update")
                            .named(format!("Update to Toru {}", update.version))
                            .small()
                            .primary()
                            .on_click(act(cx, |this, cx| this.install_update(cx))),
                    ),
            ),
    )
}
