//! The main window (DESIGN.md §2): title bar, capture toolbar, the body with the gallery and the
//! settings column, then the bottom bar, with the toast above it. While anything is selected the
//! bulk bar (§4) floats over the gallery's foot in the bottom bar's place. Opening a card in the
//! editor (§5) replaces everything under the title bar with it. Captures come in through
//! `captures`, the gallery's buttons act on them in `actions`, and the gallery pages through
//! earlier days itself (`history`). Each choice that is a setting reads and writes `Settings`,
//! every write going through `change_settings` to the store. Old unsaved captures are deleted by
//! the retention setting (`retention`) at launch, at midnight and when the setting changes.

mod actions;
mod backdrop;
mod bottom_bar;
mod bulk_bar;
mod captures;
mod card;
mod closing;
mod custom_palette;
mod delayed;
mod gallery;
mod history;
mod retention;
mod selection;
mod settings;
mod shortcuts_dialog;
mod title_bar;
mod toolbar;

use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyWindowHandle, App, Context, DisplayId, Entity, FocusHandle, Focusable, KeyDownEvent, Pixels,
    RenderImage, ScrollHandle, SharedString, Task, Window, div, prelude::*, px,
};
use gpui_kit::component::input::{InputEvent, InputState};
use tiny_skia::Pixmap;
use toru_core::{
    capture::{Capture, CaptureMode},
    naming::{NameError, validate_pattern},
    settings::Settings,
    store::{SessionId, Store},
    time::{LocalDate, LocalDateTime},
};

use crate::editor::{Editor, EditorEvent, document};
use crate::overlay;
use crate::platform;
use crate::shortcuts::{Hotkeys, Planned};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Flash, FocusInView, Toast};
use backdrop::Backdrop;
use custom_palette::{BackgroundPicker, CustomEditor};
use gallery::{Gallery, GalleryEvent};
use shortcuts_dialog::ShortcutsDialog;

/// The window's title in the taskbar. The title bar Toru draws has its own (`MainWindow::title`).
pub const WINDOW_TITLE: &str = "Toru";

const TOAST_FOR: Duration = Duration::from_secs(2);
/// `.hint`'s `line-height: 1.45`.
const HINT_LEADING: f32 = 1.45;
/// How often the date is looked at. A timer set for midnight would be wrong after a sleep or a
/// changed clock, so the date itself is checked often, whatever the window is doing.
const DAY_CHECK_EVERY: Duration = Duration::from_secs(10);

/// A display Full screen can capture, as the picker shows it.
struct Display {
    id: DisplayId,
    /// What a per-display shortcut is stored under (`platform::display_key`): the monitor's own
    /// identity, which survives restarts and unplugging, where `id` doesn't.
    key: String,
    /// "Display 1 · 2560×1440": its number and its size in physical pixels, which a capture has.
    label: SharedString,
}

pub struct MainWindow {
    store: Arc<Store>,
    session: SessionId,
    /// The date this session is for. It moves on as soon as a change of date is seen, ahead of
    /// `session`, which follows on the queue.
    day: LocalDate,
    settings: Settings,
    /// The Region button, focused when the window opens so Tab works from the start (spike 1.5).
    first: FocusHandle,
    /// The toolbar's gear, which shows and hides the settings column.
    gear: FocusHandle,
    /// The name pattern as typed. `settings.name_pattern` only follows it while it is valid;
    /// otherwise `pattern_error` says what is wrong.
    pattern: Entity<InputState>,
    pattern_error: Option<NameError>,
    /// The Delete unsaved after control's state (`retention`).
    retention: retention::RetentionControl,
    settings_scroll: ScrollHandle,
    /// Scrolls the settings column to whichever of its controls takes keyboard focus.
    settings_focus: FocusInView,
    session_started: SharedString,
    /// The display Full screen captures. `None`, or a display that has since gone, means the
    /// primary one.
    display: Option<DisplayId>,
    gallery: Entity<Gallery>,
    /// The editor, while a capture is open in it, in place of the toolbar and gallery.
    editor: Option<Entity<Editor>>,
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
    /// How many captures and store jobs are under way, which quitting waits for (`closing`).
    pending: Rc<Cell<usize>>,
    /// `Pictures\Screenshots`, where captures go unless the Save to setting says otherwise.
    default_save_folder: Option<PathBuf>,
    countdown: Option<delayed::RunningCountdown>,
    /// Quitting is waiting for `pending` work before it applies the On close setting.
    quitting: bool,
    /// The tray icon is there to bring the window back from, so closing may hide it.
    has_tray: bool,
    /// The quit dialog is up, asking about this many unsaved captures (`closing`).
    quit_prompt: Option<usize>,
    /// The open dialog's first action, focused when the quit dialog opens.
    dialog_focus: FocusHandle,
    /// The custom palette editor, while it is open over the window.
    custom_editor: Option<CustomEditor>,
    /// The editor's file picker while it is open, one per window whatever the editor does.
    background_picker: BackgroundPicker,
    /// The theme's background photo, once asked for (`backdrop`).
    backdrop: Option<Backdrop>,
    /// The photo being read right now, so one change starts one read.
    backdrop_loading: Option<PathBuf>,
    /// Photos no longer shown, taken out of the GPU at the next frame.
    stale_backdrops: Vec<Arc<RenderImage>>,
    /// The global shortcut registry (`shortcuts`), and the rows it was last given.
    hotkeys: Hotkeys,
    shortcuts: Vec<Planned>,
    /// The Edit shortcuts dialog, while it is open (`shortcuts_dialog`).
    shortcuts_dialog: Option<ShortcutsDialog>,
}

impl MainWindow {
    pub fn new(
        store: Arc<Store>,
        session: SessionId,
        settings: Settings,
        hotkeys: Hotkeys,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let first = cx.focus_handle().tab_stop(true);
        first.focus(window, cx);
        // Something must hold focus for Tab to work (`docs/spikes.md` §1.5). This fires in the
        // draw where the focused element stops being rendered, as a collapsed group's card or a
        // name field scrolled out of the list does. Focus goes to the nearest focusable ancestor
        // still drawn, the gallery or the settings column, so Tab carries on from there. When
        // there is none, it goes to the editor's Back while the editor is open; otherwise, as when
        // the bottom bar gives way to the bulk bar or the bulk bar goes with the selection while a
        // button on it has focus, it goes into the gallery, where Tab reaches the current card; the
        // toolbar's Region button would start a capture on the Enter or Space that follows.
        // With no such ancestor and the Edit shortcuts dialog open, focus goes to its Done: the
        // quit dialog's Cancel takes its buttons away while one has focus, and the two dialogs
        // are siblings, so nothing of the one that went lies inside the one that stays.
        cx.on_focus_lost(window, |this, window, cx| {
            let target = window.focus_lost_restore_target(cx).unwrap_or_else(|| {
                match (&this.shortcuts_dialog, &this.editor) {
                    (Some(dialog), _) => dialog.done.clone(),
                    (None, Some(editor)) => editor.focus_handle(cx),
                    (None, None) => this.gallery.read(cx).focus_handle(),
                }
            });
            target.focus(window, cx)
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            this.window_active = window.is_window_active();
            if this.window_active {
                this.show_missed_failures(cx);
            } else {
                // A key pressed in another app is that app's, not a new shortcut.
                this.stop_recording(cx);
            }
        })
        .detach();
        // A display plugged in or unplugged, the window hidden in the tray or not, changes which
        // per-display shortcuts can be registered.
        let this = cx.weak_entity();
        cx.observe_displays(move |_, cx| {
            this.update(cx, |this, cx| this.refresh_hotkeys(cx)).ok();
        })
        .detach();
        // Closing the window hides it in the tray or quits, by the settings (`closing`); GPUI
        // itself never closes it.
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            this.update(cx, |this, cx| this.on_close_requested(cx))
                .unwrap_or(true)
        });
        let now = platform::local_now();
        let gallery = cx.new(|cx| {
            let mut gallery = Gallery::new(store.clone(), session, now.date, window, cx);
            gallery.set_width(gallery_width(&settings, window), window.scale_factor());
            gallery
        });
        cx.subscribe(&gallery, |this, _, event: &GalleryEvent, cx| match event {
            GalleryEvent::Copy { card, capture } => this.copy_card(*card, *capture, cx),
            GalleryEvent::Renamed { capture, name } => {
                this.rename_capture(*capture, name.clone(), cx)
            }
            GalleryEvent::Save { capture } => this.save_card(*capture, cx),
            GalleryEvent::OpenFolder { file } => this.open_folder(file.clone(), cx),
            GalleryEvent::OpenEditor(id) => this.open_editor(*id, cx),
        })
        .detach();
        // The card says why a name was refused; while that card is in the editor, the editor's
        // top bar says it too.
        cx.observe(&gallery, |this, gallery, cx| {
            if let Some(editor) = &this.editor {
                let name = editor.read(cx).name_field();
                let error = gallery.read(cx).name_error_of(&name);
                editor.update(cx, |editor, cx| editor.set_name_error(error, cx));
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
        let retention = retention::RetentionControl::new(settings.retention, window, cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DAY_CHECK_EVERY).await;
                let checked = this.update(cx, |this, cx| this.check_day(platform::local_now(), cx));
                if checked.is_err() {
                    break;
                }
            }
        })
        .detach();
        let mut this = MainWindow {
            store,
            session,
            day: now.date,
            settings,
            first,
            gear: cx.focus_handle().tab_stop(true),
            pattern,
            pattern_error: None,
            retention,
            settings_scroll: ScrollHandle::new(),
            settings_focus: FocusInView::new(cx),
            session_started: now.clock().into(),
            display: None,
            gallery,
            editor: None,
            toast: Flash::new(),
            window_active: window.is_window_active(),
            missed_failures: Vec::new(),
            window: window.window_handle(),
            queue: None,
            capturing: false,
            pending: closing::pending_counter(),
            default_save_folder: platform::default_save_folder(),
            countdown: None,
            quitting: false,
            has_tray: true,
            quit_prompt: None,
            dialog_focus: cx.focus_handle().tab_stop(true),
            custom_editor: None,
            background_picker: BackgroundPicker::default(),
            backdrop: None,
            backdrop_loading: None,
            stale_backdrops: Vec::new(),
            hotkeys,
            shortcuts: Vec::new(),
            shortcuts_dialog: None,
        };
        this.refresh_hotkeys(cx);
        this.announce_shortcut_problems(cx);
        this
    }

    /// Ends the session and starts another if the date is no longer the session's. It goes on the
    /// queue so that captures already on their way land in the session they were taken in.
    fn check_day(&mut self, now: LocalDateTime, cx: &mut Context<Self>) {
        if now.date == self.day {
            return;
        }
        let previous = std::mem::replace(&mut self.day, now.date);
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let session = cx
                .background_spawn(async move { store.new_session() })
                .await;
            this.update(cx, |this, cx| match session {
                Ok(session) => this.start_session(session, now, cx),
                // The next check tries again.
                Err(_) => this.day = previous,
            })
            .ok();
        });
    }

    fn start_session(&mut self, session: SessionId, now: LocalDateTime, cx: &mut Context<Self>) {
        self.session = session;
        self.session_started = now.clock().into();
        let gallery = self.gallery.clone();
        self.window
            .update(cx, |_, window, cx| {
                gallery.update(cx, |gallery, cx| {
                    gallery.end_session(session, now.date, window, cx)
                })
            })
            .ok();
        // A new day may put more captures past the retention period.
        self.clean_up_history(cx);
        cx.notify();
    }

    /// Opens the card with `id` in the editor. Its picture is read back from where the store
    /// keeps it, on the capture queue, since a Save queued ahead moves it; if it can't be read
    /// the editor doesn't open and a toast says so, as Copy does. "Open editor after capture"
    /// calls this with the new card's id.
    pub fn open_editor(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some((capture, name, stored)) = self
            .gallery
            .read(cx)
            .card(id)
            .map(|card| (card.capture.clone(), card.name.clone(), card.files.id))
        else {
            return;
        };
        let store = self.store.clone();
        let window = self.window;
        self.enqueue(cx, async move |this, cx| {
            let picture = cx
                .background_spawn(async move {
                    document::load_picture(&store.full_image_path(stored).ok()?)
                })
                .await;
            cx.update_window(window, |_, window, cx| {
                this.update(cx, |this, cx| match picture {
                    Some(base) => this.show_editor(capture, name, base, window, cx),
                    None => this.report_failure(captures::FILE_GONE, cx),
                })
            })
            .ok();
        });
    }

    fn show_editor(
        &mut self,
        capture: Capture,
        name: Entity<InputState>,
        base: Pixmap,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_editor(window, cx);
        let editor = cx.new(|cx| Editor::new(capture, name, base, cx));
        cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &EditorEvent, window, cx| match event {
                EditorEvent::Back => this.close_editor(window, cx),
                EditorEvent::Copied => this.report_copy(Ok(()), cx),
            },
        )
        .detach();
        editor.focus_handle(cx).focus(window, cx);
        self.editor = Some(editor);
        cx.notify();
    }

    /// The `editor` start-up argument: straight into the editor on the newest card.
    pub fn open_newest_in_editor(&mut self, cx: &mut Context<Self>) {
        let newest = self
            .gallery
            .read(cx)
            .groups()
            .flat_map(|group| group.cards.first())
            .next()
            .map(|card| card.id);
        if let Some(id) = newest {
            self.open_editor(id, cx);
        }
    }

    /// Whether the editor is open with annotations that closing it would lose.
    fn editor_has_edits(&self, cx: &App) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_edited())
    }

    /// Back to session. Focus returns to the first control through `on_focus_lost`, and the
    /// picture leaves the GPU.
    fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.take() {
            editor.update(cx, |editor, _| editor.release_images(window));
        }
        cx.notify();
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
                    key: platform::display_key(display.as_ref()),
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
            CaptureMode::FullScreen => self.capture_full_screen(None, cx),
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

    fn title(&self, cx: &App) -> SharedString {
        match &self.editor {
            Some(editor) => editor.read(cx).title(cx),
            None => format!("Toru — session started {}", self.session_started).into(),
        }
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

    /// Select mode's keys, from anywhere in the window (DESIGN.md §4 and §7): Esc leaves it and
    /// Ctrl+A selects every listed card. A control that handles the key itself, as a name field
    /// does with both, keeps it.
    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let select_mode = self.gallery.read(cx).in_select_mode();
        match keystroke.key.as_str() {
            "escape" if select_mode => {
                cx.stop_propagation();
                self.gallery
                    .update(cx, |gallery, cx| gallery.leave_select_mode(cx));
            }
            "a" if selection::selects_all(&keystroke.modifiers) => {
                cx.stop_propagation();
                self.gallery
                    .update(cx, |gallery, cx| gallery.select_all(cx));
            }
            _ => {}
        }
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
        self.sync_backdrop(window, cx);
        let theme = cx.theme();
        let c = &theme.colors;
        let frame = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg)
            .text_color(c.text)
            .type_style(theme, theme.type_scale.body)
            // First, so everything else paints over them.
            .children(self.backdrop_layers(cx))
            .child(title_bar::title_bar(self.title(cx), window, cx));
        let frame = match self.editor.clone() {
            Some(editor) => frame.child(div().flex_1().min_h_0().child(editor)),
            None => {
                let width = gallery_width(&self.settings, window);
                let show_previous = self.settings.show_previous;
                self.gallery.update(cx, |gallery, cx| {
                    gallery.set_width(width, window.scale_factor());
                    gallery.set_show_previous(show_previous, cx);
                });
                let counts = self.gallery.read(cx).selection_counts();
                // While anything is selected the bulk bar floats over the foot of the gallery
                // and, at widths under about 1450px, of the settings column too; both keep a
                // focused control clear of it (§7).
                let bar = (counts.selected > 0)
                    .then(|| px(bulk_bar::BOTTOM) + bulk_bar::height(cx.theme()));
                frame
                    .on_key_down(cx.listener(|this, event, _, cx| this.key_down(event, cx)))
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
                                body.child(settings::settings_column(self, bar, window, cx))
                            }),
                    )
                    // The bulk bar takes the bottom bar's place while anything is selected (§4).
                    .when(counts.selected == 0, |this| {
                        this.child(bottom_bar::bottom_bar(self, cx))
                    })
                    .when(counts.selected > 0, |this| {
                        this.child(bulk_bar::bulk_bar(&counts, cx))
                    })
            }
        };
        // Absolute, so showing them moves nothing (DESIGN.md §2). The dialogs lie over the
        // editor as much as the gallery: quitting is asked about wherever the user is.
        frame
            .children(self.toast.get().cloned())
            .children(self.retention_dialog(cx))
            .children(self.custom_editor_view(window, cx))
            .children(self.shortcuts_dialog(cx))
            .children(
                self.quit_prompt
                    .map(|unsaved| self.quit_dialog(unsaved, cx)),
            )
    }
}
