//! The main window (DESIGN.md §2): title bar, capture toolbar, then the body. The gallery,
//! settings column and bottom bar fill the body from section 4.2 on; until then it is empty.
//! Everything here is mock behaviour: the capture buttons capture nothing yet, but each choice
//! that is a setting already reads and writes `Settings`.

mod title_bar;
mod toolbar;

use gpui::{App, Context, DisplayId, FocusHandle, SharedString, Window, div, prelude::*};
use toru_core::settings::Settings;

use crate::platform;
use crate::theme::{ActiveTheme, ThemedText};

/// A display Full screen can capture, as the picker shows it.
struct Display {
    id: DisplayId,
    /// "Display 1 · 2560×1440": its number and its size in physical pixels, which a capture has.
    label: SharedString,
}

pub struct MainWindow {
    settings: Settings,
    /// The Region button, focused when the window opens so Tab works from the start (spike 1.5).
    first: FocusHandle,
    /// Mock data until Phase 5 starts real sessions.
    session_started: SharedString,
    /// The display Full screen captures. `None`, or a display that has since gone, means the
    /// primary one.
    display: Option<DisplayId>,
}

impl MainWindow {
    pub fn new(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let first = cx.focus_handle().tab_stop(true);
        first.focus(window, cx);
        MainWindow {
            settings,
            first,
            session_started: "2:14 PM".into(),
            display: None,
        }
    }

    /// The displays as they are right now, since monitors come and go while Toru runs, and the
    /// index of the chosen one among them.
    fn displays(&self, cx: &App) -> (Vec<Display>, usize) {
        let displays: Vec<Display> = cx
            .displays()
            .iter()
            .enumerate()
            .map(|(index, display)| {
                let (width, height) = platform::display_pixels(display.as_ref());
                Display {
                    id: display.id(),
                    label: format!("Display {} · {width}×{height}", index + 1).into(),
                }
            })
            .collect();
        let position = |id: Option<DisplayId>| displays.iter().position(|d| Some(d.id) == id);
        let chosen = position(self.display)
            .or_else(|| position(cx.primary_display().map(|d| d.id())))
            .unwrap_or(0);
        (displays, chosen)
    }

    fn title(&self) -> SharedString {
        format!("Toru — session started {}", self.session_started).into()
    }

    /// Runs `f` on this view from a control's handler.
    fn act(
        cx: &Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let view = cx.entity();
        move |_, cx| view.update(cx, |this, cx| f(this, cx))
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg)
            .text_color(c.text)
            .type_style(theme, theme.type_scale.body)
            .child(title_bar::title_bar(self.title(), window, cx))
            .child(toolbar::toolbar(self, cx))
            // The body: gallery, settings column and bottom bar, from 4.2 on.
            .child(div().flex_1())
    }
}
