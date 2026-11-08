//! The capture toolbar (DESIGN.md §2): Region, Window, Full screen with its display picker,
//! Delayed with its seconds and mode, the shortcut hint, New session and the settings gear.

use gpui::{AnyElement, Context, Div, div, prelude::*, px};
use toru_core::{capture::CaptureMode, settings::DELAY_SECONDS};

use super::{MainWindow, act};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Button, Icon, Menu};

const MODES: [CaptureMode; 3] = [
    CaptureMode::Region,
    CaptureMode::Window,
    CaptureMode::FullScreen,
];

/// The Delayed menu or, while a countdown runs, the time left on a button that cancels it.
fn delayed(this: &MainWindow, cx: &Context<MainWindow>) -> AnyElement {
    if let Some(left) = this.countdown_sub() {
        return Button::new("delayed", "Cancel")
            .icon(Icon::Clock)
            .sub(left)
            .on_click(act(cx, |this, cx| this.cancel_countdown(cx)))
            .into_any_element();
    }
    let seconds = this.settings.delay_seconds;
    let mode = this.settings.delay_mode;
    let mut menu = Menu::new(
        "delayed",
        "Delayed capture",
        Button::new("delayed", "Delayed")
            .icon(Icon::Clock)
            .sub(format!("{seconds} s ▾")),
    )
    .header("Delay");
    for choice in DELAY_SECONDS {
        menu = menu.choice(
            format!("{choice} s"),
            choice == seconds,
            act(cx, move |this, cx| {
                this.change_settings(cx, |settings| settings.delay_seconds = choice)
            }),
        );
    }
    menu = menu.header("Capture");
    for choice in MODES {
        menu = menu.choice(
            choice.label(),
            choice == mode,
            act(cx, move |this, cx| {
                this.change_settings(cx, |settings| settings.delay_mode = choice)
            }),
        );
    }
    menu.divider()
        .item(
            format!("Capture {} in {seconds} s", mode.label().to_lowercase()),
            act(cx, |this, cx| this.start_countdown(cx)),
        )
        .into_any_element()
}

pub fn toolbar(this: &MainWindow, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let settings = &this.settings;

    // A split button: the label captures, the sub opens the display picker.
    let (displays, chosen) = this.displays(cx);
    let full_screen = displays.into_iter().enumerate().fold(
        Menu::new(
            "display",
            "Display to capture",
            Button::new("full-screen", "Full screen")
                .icon(Icon::Screen)
                .sub(format!("Display {} ▾", chosen + 1))
                .split(format!("Display {}, choose display", chosen + 1))
                .on_click(act(cx, |this, cx| {
                    this.capture(CaptureMode::FullScreen, cx)
                })),
        ),
        |menu, (index, display)| {
            menu.choice(
                display.label,
                index == chosen,
                act(cx, move |this, cx| {
                    this.display = Some(display.id);
                    cx.notify();
                }),
            )
        },
    );

    let shortcuts: Vec<&str> = [&settings.region_shortcut, &settings.window_shortcut]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();

    div()
        .flex_none()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(12.))
        .bg(c.surface_2)
        .border_b(hair.width)
        .border_color(hair.color)
        .child(
            Button::new("region", "Region")
                .icon(Icon::Region)
                .primary()
                .track_focus(&this.first)
                .on_click(act(cx, |this, cx| this.capture(CaptureMode::Region, cx))),
        )
        .child(
            Button::new("window", "Window")
                .icon(Icon::Window)
                .on_click(act(cx, |this, cx| this.capture(CaptureMode::Window, cx))),
        )
        .child(full_screen)
        .child(delayed(this, cx))
        .child(
            div()
                .ml(px(4.))
                .type_style(theme, theme.type_scale.hint)
                .text_color(c.muted)
                .whitespace_nowrap()
                .child(shortcuts.join(" · ")),
        )
        .child(div().flex_1().min_w(px(40.)))
        .child(
            Button::new("new-session", "New session")
                .icon(Icon::Plus)
                .on_click(|_, _| {}),
        )
        .child(
            Button::icon_only(
                "settings",
                Icon::Gear,
                if settings.settings_collapsed {
                    "Show settings"
                } else {
                    "Hide settings"
                },
            )
            .track_focus(&this.gear)
            .on_click(act(cx, |this, cx| {
                this.change_settings(cx, |settings| {
                    settings.settings_collapsed = !settings.settings_collapsed
                })
            })),
        )
}
