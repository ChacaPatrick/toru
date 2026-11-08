//! The main window (DESIGN.md §2): title bar, capture toolbar, the body with the gallery and the
//! settings column, then the bottom bar, with the toast above it. This session's captures are
//! real (`captures`, `actions`); the earlier groups are mock until 7.1. Each choice that is a
//! setting reads and writes `Settings`, every write going through `change_settings` to the store.

mod actions;
mod bottom_bar;
mod captures;
mod card;
mod delayed;
mod gallery;
mod mock;
mod settings;
mod title_bar;
mod toolbar;

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{
    AnyWindowHandle, App, Context, DisplayId, Entity, FocusHandle, Pixels, ScrollHandle,
    SharedString, Task, Window, div, prelude::*, px,
};
use gpui_kit::component::input::{InputEvent, InputState};
use toru_core::{
    capture::CaptureMode,
    naming::{NameError, validate_pattern},
    settings::Settings,
    store::{SessionId, Store},
};

use crate::overlay;
use crate::platform;
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Flash, FocusInView, Toast};
use gallery::{Gallery, GalleryEvent};

/// The window's title in the taskbar. The title bar Toru draws has its own (`MainWindow::title`).
pub const WINDOW_TITLE: &str = "Toru";

const TOAST_FOR: Duration = Duration::from_secs(2);

/// A display Full screen can capture, as the picker shows it.
struct Display {
    id: DisplayId,
    /// "Display 1 · 2560×1440": its number and its size in physical pixels, which a capture has.
    label: SharedString,
}

pub struct MainWindow {
    store: Arc<Store>,
    session: SessionId,
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
    session_started: SharedString,
    /// The display Full screen captures. `None`, or a display that has since gone, means the
    /// primary one.
    display: Option<DisplayId>,
    gallery: Entity<Gallery>,
    toast: Flash<Toast>,
    /// Whether the window is the active one, so a toast shown now would be seen.
    window_active: bool,
    /// Failures from while it wasn't, shown when it next is.
    missed_failures: Vec<SharedString>,
    /// The window new cards' name fields are created in.
    window: AnyWindowHandle,
    /// The last job handed to `enqueue`, which the next one waits for.
    queue: Option<Task<()>>,
    /// A full-screen capture is under way, so a second press doesn't overlap it.
    capturing: bool,
    /// `Pictures\Screenshots`, where captures go unless the Save to setting says otherwise.
    default_save_folder: Option<PathBuf>,
    countdown: Option<delayed::RunningCountdown>,
}

impl MainWindow {
    pub fn new(
        store: Arc<Store>,
        session: SessionId,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let first = cx.focus_handle().tab_stop(true);
        first.focus(window, cx);
        // Something must hold focus for Tab to work (`docs/spikes.md` §1.5). This fires in the
        // draw where the focused element stops being rendered, as a collapsed group's card or a
        // name field scrolled out of the list does. Focus goes to the nearest focusable ancestor
        // still drawn, the gallery or the settings column, so Tab carries on from there, and to
        // the first control when there is none.
        cx.on_focus_lost(window, |this, window, cx| {
            let target = window
                .focus_lost_restore_target(cx)
                .unwrap_or_else(|| this.first.clone());
            target.focus(window, cx)
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            this.window_active = window.is_window_active();
            if this.window_active {
                this.show_missed_failures(cx);
            }
        })
        .detach();
        let now = platform::local_now();
        let gallery = cx.new(|cx| {
            let mut gallery = Gallery::new(now.date, window, cx);
            gallery.set_width(gallery_width(&settings, window), window.scale_factor());
            gallery
        });
        cx.subscribe(&gallery, |this, _, event: &GalleryEvent, cx| match event {
            GalleryEvent::Copied => this.report_copy(Ok(()), cx),
            GalleryEvent::Copy { card, capture } => this.copy_card(*card, *capture, cx),
            GalleryEvent::Renamed { capture, name } => {
                this.rename_capture(*capture, name.clone(), cx)
            }
            GalleryEvent::Save { capture } => this.save_card(*capture, cx),
            GalleryEvent::OpenFolder { file } => this.open_folder(file.clone(), cx),
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
            store,
            session,
            settings,
            first,
            gear: cx.focus_handle().tab_stop(true),
            pattern,
            pattern_error: None,
            settings_scroll: ScrollHandle::new(),
            settings_focus: FocusInView::new(cx),
            session_started: now.clock().into(),
            display: None,
            gallery,
            toast: Flash::new(),
            window_active: window.is_window_active(),
            missed_failures: Vec::new(),
            window: window.window_handle(),
            queue: None,
            capturing: false,
            default_save_folder: platform::default_save_folder(),
            countdown: None,
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

    /// Takes a capture now, whichever way it was asked for, in place of any countdown.
    fn capture(&mut self, mode: CaptureMode, cx: &mut Context<Self>) {
        self.cancel_countdown(cx);
        match mode {
            CaptureMode::Region | CaptureMode::Window => self.capture_with_overlay(mode, cx),
            CaptureMode::FullScreen => self.capture_full_screen(cx),
        }
    }

    /// Opens the Region or Window overlay (DESIGN.md §6); its image arrives in `add_capture`.
    fn capture_with_overlay(&mut self, mode: CaptureMode, cx: &mut Context<Self>) {
        let this = cx.entity();
        let failed = this.clone();
        overlay::start(
            mode,
            cx,
            move |image, mode, cx| {
                this.update(cx, |this, cx| this.add_capture(image, mode, cx));
            },
            move |cx| failed.update(cx, |this, cx| this.report_region_failed(cx)),
        );
    }

    /// Runs `f` on the window. Deferred, because the window can't be reached from inside one of
    /// its own handlers.
    fn on_window(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut Window) + 'static) {
        let window = self.window;
        cx.defer(move |cx| {
            window.update(cx, |_, window, _| f(window)).ok();
        });
    }

    fn title(&self) -> SharedString {
        format!("Toru — session started {}", self.session_started).into()
    }

    /// Every change to a setting goes through here, and on to the store.
    fn change_settings(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings);
        let settings = self.settings.clone();
        self.write_store("Couldn't save your settings", cx, move |store| {
            store.save_settings(&settings)
        });
        cx.notify();
    }

    fn show_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toast
            .show(toast, TOAST_FOR, cx, |this| &mut this.toast);
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

/// The width left for the gallery beside the settings column.
fn gallery_width(settings: &Settings, window: &Window) -> Pixels {
    let settings_width = if settings.settings_collapsed {
        px(0.)
    } else {
        px(settings::WIDTH)
    };
    window.viewport_size().width - settings_width
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = gallery_width(&self.settings, window);
        let show_previous = self.settings.show_previous;
        self.gallery.update(cx, |gallery, cx| {
            gallery.set_width(width, window.scale_factor());
            gallery.set_show_previous(show_previous, cx);
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
            .child(bottom_bar::bottom_bar(self, cx))
            // Absolute, so showing it moves nothing (DESIGN.md §2).
            .children(self.toast.get().cloned())
    }
}
