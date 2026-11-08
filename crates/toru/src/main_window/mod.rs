//! The main window (DESIGN.md §2): title bar, capture toolbar, then the body with the gallery
//! and the settings column; the bottom bar follows in 4.4. Everything here is mock behaviour:
//! the capture buttons capture nothing yet, but each choice that is a setting already reads and
//! writes `Settings`, every write going through `change_settings`.

mod card;
mod gallery;
mod mock;
mod settings;
mod title_bar;
mod toolbar;

use std::time::Duration;

use gpui::{
    App, Context, DisplayId, Entity, FocusHandle, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};
use gpui_kit::component::input::{InputEvent, InputState};
use toru_core::{
    naming::{NameError, validate_pattern},
    settings::Settings,
};

use crate::platform;
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Flash, FocusInView, Toast};
use gallery::{Gallery, GalleryEvent};

const TOAST_FOR: Duration = Duration::from_secs(2);

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
    /// The toolbar's gear, which shows and hides the settings column.
    gear: FocusHandle,
    /// The name pattern as typed. `settings.name_pattern` only follows it while it is valid;
    /// otherwise `pattern_error` says what is wrong.
    pattern: Entity<InputState>,
    pattern_error: Option<NameError>,
    settings_scroll: ScrollHandle,
    /// Scrolls the settings column to whichever of its controls takes keyboard focus.
    settings_focus: FocusInView,
    /// Mock data until Phase 5 starts real sessions.
    session_started: SharedString,
    /// The display Full screen captures. `None`, or a display that has since gone, means the
    /// primary one.
    display: Option<DisplayId>,
    gallery: Entity<Gallery>,
    toast: Flash<SharedString>,
}

impl MainWindow {
    pub fn new(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let first = cx.focus_handle().tab_stop(true);
        first.focus(window, cx);
        // Something must hold focus for Tab to work (`docs/spikes.md` §1.5). This fires in the
        // draw where the focused element stops being rendered, as a collapsed group's card or a
        // name field scrolled out of the list does, so focus returns to the first control at once.
        cx.on_focus_lost(window, |this, window, cx| this.first.focus(window, cx))
            .detach();
        let gallery = cx.new(|cx| Gallery::new(window, cx));
        cx.subscribe(&gallery, |this, _, event: &GalleryEvent, cx| match event {
            GalleryEvent::Copied => {
                let message = cx.theme().strings.copied.into();
                this.toast
                    .show(message, TOAST_FOR, cx, |this| &mut this.toast);
            }
        })
        .detach();
        let pattern =
            cx.new(|cx| InputState::new(window, cx).default_value(settings.name_pattern.clone()));
        cx.subscribe(&pattern, |this, pattern, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                let typed = pattern.read(cx).value().to_string();
                this.pattern_error = validate_pattern(&typed).err();
                if this.pattern_error.is_none() {
                    this.change_settings(cx, |settings| settings.name_pattern = typed);
                }
                cx.notify();
            }
        })
        .detach();
        MainWindow {
            settings,
            first,
            gear: cx.focus_handle().tab_stop(true),
            pattern,
            pattern_error: None,
            settings_scroll: ScrollHandle::new(),
            settings_focus: FocusInView::new(cx),
            session_started: "2:14 PM".into(),
            display: None,
            gallery,
            toast: Flash::new(),
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

    /// Every change to a setting goes through here, so Phase 5's store has one place to write
    /// it to disk from.
    fn change_settings(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings);
        cx.notify();
    }
}

/// Runs `f` on a view from one of its controls' handlers.
fn act<V: 'static>(
    cx: &Context<V>,
    f: impl Fn(&mut V, &mut Context<V>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let view = cx.entity();
    move |_, cx| view.update(cx, |this, cx| f(this, cx))
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_width = if self.settings.settings_collapsed {
            px(0.)
        } else {
            px(settings::WIDTH)
        };
        let show_previous = self.settings.show_previous;
        self.gallery.update(cx, |gallery, _| {
            gallery.set_width(window.viewport_size().width - settings_width);
            gallery.set_show_previous(show_previous);
        });
        let theme = cx.theme();
        let c = &theme.colors;
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg)
            .text_color(c.text)
            .type_style(theme, theme.type_scale.body)
            .child(title_bar::title_bar(self.title(), window, cx))
            .child(toolbar::toolbar(self, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.gallery.clone()),
                    )
                    .when(!self.settings.settings_collapsed, |body| {
                        body.child(settings::settings_column(self, window, cx))
                    }),
            )
            // The bottom bar arrives in 4.4; until then the toast sits where it will be above it.
            .children(self.toast.get().map(|message| Toast::new(message.clone())))
    }
}
