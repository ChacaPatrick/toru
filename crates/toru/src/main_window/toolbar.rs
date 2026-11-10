//! The capture toolbar (DESIGN.md §2): Region, Window, Full screen with its display picker,
//! Delayed with its seconds and mode, the shortcut hint, New session and the settings gear.
//!
//! The toolbar is one row at every window width. When the row has no room for everything, the
//! shortcut hint goes first, since the settings column lists the shortcuts too, and then New
//! session's label, leaving its plus with the name as a tooltip.

use gpui::{AnyElement, Context, Div, Pixels, Window, div, prelude::*, px};
use toru_core::{capture::CaptureMode, settings::DELAY_SECONDS};

use super::{MainWindow, act};
use crate::theme::{ActiveTheme, Theme, ThemedText};
use crate::ui::{Button, Icon, Menu, Readable};

const PADDING_X: f32 = 16.;
const GAP: f32 = 10.;
/// The hint's distance from Delayed, past the gap.
const HINT_INSET: f32 = 4.;
/// The least room between the hint and New session.
const HINT_AIR: f32 = 16.;

const MODES: [CaptureMode; 3] = [
    CaptureMode::Region,
    CaptureMode::Window,
    CaptureMode::FullScreen,
];

/// The Delayed menu or, while a countdown runs, the time left on a button that cancels it, and
/// how wide either is.
fn delayed(
    this: &MainWindow,
    theme: &Theme,
    window: &Window,
    cx: &Context<MainWindow>,
) -> (AnyElement, Pixels) {
    if let Some(left) = this.countdown_sub() {
        let cancel = Button::new("delayed", "Cancel").icon(Icon::Clock).sub(left);
        let width = cancel.width(theme, window);
        let cancel = cancel.on_click(act(cx, |this, cx| this.cancel_countdown(cx)));
        return (cancel.into_any_element(), width);
    }
    let seconds = this.settings.delay_seconds;
    let mode = this.settings.delay_mode;
    let trigger = Button::new("delayed", "Delayed")
        .icon(Icon::Clock)
        .sub(format!("{seconds} s ▾"));
    let width = trigger.width(theme, window);
    let mut menu = Menu::new("delayed", "Delayed capture", trigger).header("Delay");
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
    let menu = menu.divider().item(
        format!("Capture {} in {seconds} s", mode.label().to_lowercase()),
        act(cx, |this, cx| this.start_countdown(cx)),
    );
    (menu.into_any_element(), width)
}

/// What the row has room for beside the capture buttons and the gear: New session's label,
/// and the hint with it. The label outranks the hint.
#[derive(Debug, PartialEq, Eq)]
struct Fit {
    label: bool,
    hint: bool,
}

/// `room` is the row inside its padding, `fixed` the buttons that always show with the gaps
/// between all the row's items, `label` New session with its label and `hint` the hint with
/// the air around it, or `None` with no shortcut to show.
fn fit(room: Pixels, fixed: Pixels, label: Pixels, hint: Option<Pixels>) -> Fit {
    let with_label = fixed + label;
    Fit {
        label: with_label <= room,
        hint: hint.is_some_and(|hint| with_label + hint <= room),
    }
}

pub fn toolbar(this: &MainWindow, window: &Window, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let settings = &this.settings;

    let region = Button::new("region", "Region").icon(Icon::Region).primary();
    let window_button = Button::new("window", "Window").icon(Icon::Window);
    // A split button: the label captures, the sub opens the display picker.
    let (displays, chosen) = this.displays(cx);
    let full_screen = Button::new("full-screen", "Full screen")
        .icon(Icon::Screen)
        .sub(format!("Display {} ▾", chosen + 1))
        .split(format!("Display {}, choose display", chosen + 1));
    let (delayed, delayed_width) = delayed(this, theme, window, cx);
    let new_session = Button::new("new-session", "New session").icon(Icon::Plus);
    let gear = Button::icon_only(
        "settings",
        Icon::Gear,
        if settings.settings_collapsed {
            "Show settings"
        } else {
            "Hide settings"
        },
    );

    let hint = this.toolbar_hint();
    let hint_style = theme.type_scale.hint;
    // Seven items in the row: four capture buttons, the hint's space, New session and the gear.
    let fixed = [&region, &window_button, &full_screen, &gear]
        .iter()
        .fold(delayed_width + px(GAP * 6.), |sum, button| {
            sum + button.width(theme, window)
        });
    let fit = fit(
        window.viewport_size().width - px(PADDING_X * 2.),
        fixed,
        new_session.width(theme, window),
        (!hint.is_empty())
            .then(|| px(HINT_INSET + HINT_AIR) + hint_style.text_width(&hint, theme, window)),
    );

    let full_screen = displays.into_iter().enumerate().fold(
        Menu::new(
            "display",
            "Display to capture",
            full_screen.on_click(act(cx, |this, cx| {
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

    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(GAP))
        .px(px(PADDING_X))
        .py(px(12.))
        .bg(c.surface_2)
        .border_b(hair.width)
        .border_color(hair.color)
        .child(
            region
                .track_focus(&this.first)
                .on_click(act(cx, |this, cx| this.capture(CaptureMode::Region, cx))),
        )
        .child(window_button.on_click(act(cx, |this, cx| this.capture(CaptureMode::Window, cx))))
        .child(full_screen)
        .child(delayed)
        .child(
            div()
                .id("toolbar-hint")
                // Read whether or not the row has room to show it.
                .when(!hint.is_empty(), |space| {
                    space.read_as_text(format!("Shortcuts: {hint}"))
                })
                .flex_1()
                .min_w_0()
                .pl(px(HINT_INSET))
                .type_style(theme, hint_style)
                .text_color(c.muted)
                .whitespace_nowrap()
                .overflow_hidden()
                .when(fit.hint, |space| space.child(hint)),
        )
        .child(
            new_session
                .compact(!fit.label)
                .on_click(act(cx, |this, cx| this.start_new_session(cx))),
        )
        .child(gear.track_focus(&this.gear).on_click(act(cx, |this, cx| {
            this.change_settings(cx, |settings| {
                settings.settings_collapsed = !settings.settings_collapsed
            })
        })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_goes_before_the_label_as_the_row_narrows() {
        let (fixed, label, hint) = (px(600.), px(140.), Some(px(160.)));
        let both = Fit {
            label: true,
            hint: true,
        };
        let label_only = Fit {
            label: true,
            hint: false,
        };
        let neither = Fit {
            label: false,
            hint: false,
        };
        assert_eq!(fit(px(900.), fixed, label, hint), both);
        assert_eq!(fit(px(899.), fixed, label, hint), label_only);
        assert_eq!(fit(px(740.), fixed, label, hint), label_only);
        // The hint never shows beside the bare plus: it went first.
        assert_eq!(fit(px(739.), fixed, label, hint), neither);
        // No shortcut is set, so there is no hint to show however wide the row.
        assert_eq!(fit(px(2000.), fixed, label, None), label_only);
    }
}
