//! The capture overlay (DESIGN.md §6) for Region and Window, built the way `docs/spikes.md` §1.2
//! proved: Toru's own windows are taken out of what captures see (§5.1), every monitor is
//! captured into memory on a background thread, then one borderless topmost window per monitor
//! shows its frozen frame, dimmed. In Region, a drag selects a region, with the size in physical
//! pixels and a magnifier in a panel beside the pointer, and releasing or Enter confirms. In
//! Window, the top-level windows are listed just before the screens freeze, the one under the
//! pointer is lit up with its size and title, and a click or Enter captures it. Esc on any
//! monitor closes them all, and so does an overlay closed any other way, such as Alt+F4. The
//! image comes from the original frames, so what the user gets is what was on screen, not a
//! readback of the overlay.
//!
//! Everything is in physical pixels and drawn at `physical / scale`, so edges land on device
//! pixels at any scale factor. Each monitor's overlay draws in its own frame; the desktop
//! coordinates monitors and windows share only enter for picking a window and stitching one that
//! spans monitors.

mod geometry;

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, Context, DispatchPhase, Div, FocusHandle, Global, HitboxBehavior, Hsla, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, PlatformDisplay,
    Point, Render, RenderImage, Rgba, SharedString, Subscription, Window, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, canvas, div, img, prelude::*, px,
};
use toru_core::capture::CaptureMode;
use xcap::{
    Monitor,
    image::{Frame, RgbaImage, imageops},
};

use geometry::{Pt, Rect, Selection};

use crate::platform;
use crate::theme::{ActiveTheme, ThemedText};

/// The magnifier shows this many frame pixels across, each `LOUPE_ZOOM` logical pixels wide.
const LOUPE_CELLS: u32 = 15;
const LOUPE_ZOOM: f32 = 8.;
/// The panel's distance from the pointer and its inner padding, in logical pixels.
const PANEL_GAP: f32 = 20.;
const PANEL_PADDING: f32 = 10.;
/// Between the magnifier and the text below it.
const PANEL_SPACING: f32 = 8.;
/// A window title longer than this is cut with an ellipsis, so the panel stays a panel.
const TITLE_MAX_WIDTH: f32 = 360.;

/// What the overlay hands back: the cropped region or window, and which it was.
pub type OnCapture = Rc<dyn Fn(RgbaImage, CaptureMode, &mut App)>;

/// One capture in progress, from capturing the monitors to handing over the image. Only one runs
/// at a time.
struct Session {
    mode: CaptureMode,
    overlays: Vec<WindowHandle<Overlay>>,
    /// Ends the capture when any overlay is closed from outside, such as by Alt+F4.
    _closed: Subscription,
    on_capture: OnCapture,
    /// Told when no overlay could be opened, which would otherwise look like nothing happening.
    on_failed: Box<dyn FnOnce(&mut App)>,
}

impl Global for Session {}

/// One monitor's frame, captured off the main thread: the original for cropping and the
/// magnifier, and a BGRA copy ready for GPUI to draw.
struct Captured {
    /// `xcap`'s monitor id, the low 32 bits of the `HMONITOR` GPUI uses as its `DisplayId`.
    monitor_id: u32,
    /// Where the monitor sits on the desktop, in physical pixels.
    bounds: Rect,
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
}

/// One monitor's frozen frame and its place on the desktop, shared by every overlay so a window
/// spanning monitors can be stitched from all of them.
struct MonitorFrame {
    bounds: Rect,
    frame: Arc<RgbaImage>,
}

/// A top-level window as it stood when the screens froze, in desktop pixels.
struct TopWindow {
    title: SharedString,
    bounds: Rect,
}

/// What the background thread brings back: the monitors, and in Window mode the windows.
struct Frozen {
    monitors: Vec<Captured>,
    windows: Vec<TopWindow>,
}

/// Starts a Region or Window capture: takes Toru's windows out of what captures see, captures
/// every monitor, covers each one with an overlay and gives one of them the foreground, since
/// another app holds it after a delayed capture. `on_capture` runs on the main thread with the
/// image once the user confirms, and `on_failed` if no overlay could be opened.
pub fn start(
    mode: CaptureMode,
    cx: &mut App,
    on_capture: impl Fn(RgbaImage, CaptureMode, &mut App) + 'static,
    on_failed: impl FnOnce(&mut App) + 'static,
) {
    if cx.has_global::<Session>() {
        return;
    }
    cx.set_global(Session {
        mode,
        overlays: Vec::new(),
        _closed: cx.on_window_closed(|cx, closed| {
            let overlay_closed = cx
                .try_global::<Session>()
                .is_some_and(|session| session.overlays.iter().any(|o| o.window_id() == closed));
            if overlay_closed {
                // The other windows can't be touched while this one is being removed.
                cx.defer(|cx| finish(None, cx));
            }
        }),
        on_capture: Rc::new(on_capture),
        on_failed: Box::new(on_failed),
    });
    platform::exclude_windows_from_capture(true);
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(platform::EXCLUDE_SETTLE)
            .await;
        let frozen = cx.background_spawn(async move { freeze(mode) }).await;
        // Cleared as soon as the frames are in, whatever they hold, so Toru is never left out of
        // another app's capture.
        platform::exclude_windows_from_capture(false);
        cx.update(|cx| open_overlays(frozen, cx));
        platform::bring_overlay_to_foreground();
    })
    .detach();
}

/// Lists the windows, in Window mode, then captures every monitor into memory, so the list
/// matches the frames. Slow, so it runs on a background thread; `xcap`'s handles are made and
/// dropped there too, since they can't cross threads.
fn freeze(mode: CaptureMode) -> Frozen {
    let windows = match mode {
        CaptureMode::Window => list_windows(),
        _ => Vec::new(),
    };
    Frozen {
        monitors: capture_monitors(),
        windows,
    }
}

/// The visible top-level windows, topmost first, as `xcap` lists them: it already leaves out
/// Toru's own windows, cloaked ones and untitled tool windows. Minimized windows are off screen,
/// so they go too, and so do windows the user can't see or click (`platform::is_window_pickable`):
/// a click-through overlay the size of the desktop would otherwise sit over every real window and
/// be the only thing the picker finds. Bounds are `DWMWA_EXTENDED_FRAME_BOUNDS`, the frame
/// without its shadow, in the same physical desktop pixels as the monitors.
fn list_windows() -> Vec<TopWindow> {
    xcap::Window::all()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|window| {
            if window.is_minimized().ok()? || !platform::is_window_pickable(window.id().ok()?) {
                return None;
            }
            let bounds = Rect {
                x: window.x().ok()?,
                y: window.y().ok()?,
                width: window.width().ok()? as i32,
                height: window.height().ok()? as i32,
            };
            if bounds.width <= 0 || bounds.height <= 0 {
                return None;
            }
            let title = window.title().unwrap_or_default();
            let title = if title.is_empty() {
                "Untitled window".into()
            } else {
                title.into()
            };
            Some(TopWindow { title, bounds })
        })
        .collect()
}

fn capture_monitors() -> Vec<Captured> {
    let monitors = Monitor::all().unwrap_or_default();
    monitors
        .into_iter()
        .filter_map(|monitor| {
            let monitor_id = monitor.id().ok()?;
            let (x, y) = (monitor.x().ok()?, monitor.y().ok()?);
            let frame = monitor.capture_image().ok()?;
            let (width, height) = frame.dimensions();
            // GPUI's renderer takes BGRA; the original stays RGBA for the crop.
            let mut bgra = frame.clone();
            bgra.pixels_mut().for_each(|pixel| pixel.0.swap(0, 2));
            Some(Captured {
                monitor_id,
                bounds: Rect {
                    x,
                    y,
                    width: width as i32,
                    height: height as i32,
                },
                frame: Arc::new(frame),
                image: Arc::new(RenderImage::new([Frame::new(bgra)])),
            })
        })
        .collect()
}

/// Opens an overlay on each captured monitor GPUI also knows. With nothing to show, the capture
/// has failed.
fn open_overlays(frozen: Frozen, cx: &mut App) {
    let Some(mode) = cx.try_global::<Session>().map(|session| session.mode) else {
        return;
    };
    let frames: Rc<[MonitorFrame]> = frozen
        .monitors
        .iter()
        .map(|capture| MonitorFrame {
            bounds: capture.bounds,
            frame: capture.frame.clone(),
        })
        .collect();
    let windows: Rc<[TopWindow]> = frozen.windows.into();
    let displays = cx.displays();
    let mut overlays = Vec::new();
    for capture in frozen.monitors {
        let display = displays
            .iter()
            .find(|display| u64::from(display.id()) as u32 == capture.monitor_id);
        if let Some(display) = display {
            let shared = Shared {
                mode,
                frames: frames.clone(),
                windows: windows.clone(),
            };
            if let Some(handle) = open_overlay(capture, shared, display.as_ref(), cx) {
                overlays.push(handle);
            }
        }
    }
    if !cx.has_global::<Session>() {
        return;
    }
    if overlays.is_empty() {
        (cx.remove_global::<Session>().on_failed)(cx);
    } else {
        cx.global_mut::<Session>().overlays = overlays;
    }
}

/// What every overlay of one capture shares.
struct Shared {
    mode: CaptureMode,
    frames: Rc<[MonitorFrame]>,
    windows: Rc<[TopWindow]>,
}

fn open_overlay(
    capture: Captured,
    shared: Shared,
    display: &dyn PlatformDisplay,
    cx: &mut App,
) -> Option<WindowHandle<Overlay>> {
    let options = WindowOptions {
        // Fullscreen strips PopUp's frame and sizes the window to the monitor's physical bounds,
        // and PopUp keeps it topmost with no taskbar button (`docs/spikes.md` §1.2).
        window_bounds: Some(WindowBounds::Fullscreen(display.bounds())),
        display_id: Some(display.id()),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Overlay {
                mode: shared.mode,
                bounds: capture.bounds,
                frame: capture.frame,
                image: capture.image,
                frames: shared.frames,
                windows: shared.windows,
                focus,
                pointer: None,
                selection: None,
                picked: None,
            }
        })
    })
    .ok()
}

/// Ends the capture: closes every overlay and, with an image, hands it over. Windows only leave
/// the screen when the message loop next runs, so anything slow with the image belongs to the
/// receiver, off the main thread. Dropping the session also drops its window-closed watcher.
fn finish(image: Option<RgbaImage>, cx: &mut App) {
    if !cx.has_global::<Session>() {
        return;
    }
    let session = cx.remove_global::<Session>();
    for overlay in session.overlays {
        overlay
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
    if let Some(image) = image {
        (session.on_capture)(image, session.mode, cx);
    }
}

/// One monitor's overlay: its frozen frame, the drag or the picked window on it, and the pointer
/// for the panel.
struct Overlay {
    mode: CaptureMode,
    /// This monitor on the desktop.
    bounds: Rect,
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
    /// Every monitor's frame, for a window that reaches beyond this one.
    frames: Rc<[MonitorFrame]>,
    /// The windows to pick from, topmost first. Empty in Region.
    windows: Rc<[TopWindow]>,
    focus: FocusHandle,
    /// The frame pixel under the pointer, once it has moved over this monitor.
    pointer: Option<Pt>,
    /// The drag in progress, in Region. Releasing confirms it, so it never outlives the drag.
    selection: Option<Selection>,
    /// The window under the pointer, in Window, as an index into `windows`. Every overlay holds
    /// the same one, so a window spanning monitors lights up on each.
    picked: Option<usize>,
}

impl Overlay {
    fn frame_size(&self) -> (u32, u32) {
        self.frame.dimensions()
    }

    /// A window position as a pair of logical pixels.
    fn logical(position: Point<Pixels>) -> (f32, f32) {
        (f32::from(position.x), f32::from(position.y))
    }

    fn press(&mut self, position: Point<Pixels>, scale: f32, cx: &mut Context<Self>) {
        if self.mode == CaptureMode::Region {
            let edge = geometry::edge_at(Self::logical(position), scale, self.frame_size());
            self.selection = Some(Selection::new(edge));
            cx.notify();
        }
    }

    /// The pointer has moved, perhaps while dragging, and perhaps past this monitor's edge.
    fn moved(&mut self, position: Point<Pixels>, scale: f32, cx: &mut Context<Self>) {
        let position = Self::logical(position);
        let frame = self.frame_size();
        let pixel = geometry::pixel_at(position, scale, frame);
        self.pointer = Some(pixel);
        match self.mode {
            CaptureMode::Region => {
                if let Some(selection) = &mut self.selection {
                    selection.cursor = geometry::edge_at(position, scale, frame);
                }
            }
            _ => {
                let at = (pixel.0 + self.bounds.x, pixel.1 + self.bounds.y);
                let picked = geometry::window_at(self.windows.iter().map(|w| w.bounds), at);
                self.pick(picked, cx);
            }
        }
        cx.notify();
    }

    /// Makes `picked` the window under the pointer on every overlay, since it may stretch onto
    /// other monitors, whose overlays light up their part of it.
    fn pick(&mut self, picked: Option<usize>, cx: &mut Context<Self>) {
        if self.picked == picked {
            return;
        }
        self.picked = picked;
        cx.defer(move |cx| {
            let Some(session) = cx.try_global::<Session>() else {
                return;
            };
            for overlay in session.overlays.clone() {
                overlay
                    .update(cx, |this, _, cx| {
                        if this.picked != picked {
                            this.picked = picked;
                            cx.notify();
                        }
                    })
                    .ok();
            }
        });
    }

    /// The button came up, or Enter was pressed: captures what is selected or picked, if
    /// anything; a bare click in Region, or Enter over bare desktop in Window, waits.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let image = match self.mode {
            CaptureMode::Region => self.crop_selection(cx),
            _ => self.crop_picked(),
        };
        if let Some(image) = image {
            // A window can't be closed from inside its own update.
            cx.defer(move |cx| finish(Some(image), cx));
        }
    }

    fn crop_selection(&mut self, cx: &mut Context<Self>) -> Option<RgbaImage> {
        let selection = self.selection.take()?;
        if selection.is_empty() {
            cx.notify();
            return None;
        }
        let Rect {
            x,
            y,
            width,
            height,
        } = selection.rect();
        let crop = imageops::crop_imm(
            self.frame.as_ref(),
            x as u32,
            y as u32,
            width as u32,
            height as u32,
        )
        .to_image();
        Some(crop)
    }

    /// The picked window as the frozen monitors show it (`geometry::compose`): a window
    /// spanning two monitors is stitched from both frames.
    fn crop_picked(&self) -> Option<RgbaImage> {
        let window = self.windows.get(self.picked?)?;
        let monitors: Vec<(Rect, &RgbaImage)> = self
            .frames
            .iter()
            .map(|monitor| (monitor.bounds, monitor.frame.as_ref()))
            .collect();
        geometry::compose(window.bounds, &monitors)
    }

    fn cancel(&self, cx: &mut Context<Self>) {
        cx.defer(|cx| finish(None, cx));
    }

    /// The rect to light up on this monitor, in frame pixels: the drag, or the part of the
    /// picked window that is on this monitor.
    fn lit(&self, frame: Rect) -> Option<Rect> {
        match self.mode {
            CaptureMode::Region => self.selection.map(Selection::rect),
            _ => {
                let window = self.windows.get(self.picked?)?;
                window
                    .bounds
                    .relative_to((self.bounds.x, self.bounds.y))
                    .intersect(frame)
            }
        }
    }

    /// The mouse layer. A plain div's listeners stop once the pointer leaves its hitbox, which
    /// a drag does the moment it crosses onto the next monitor, although Windows keeps sending
    /// moves and the release to the window that holds the mouse (`docs/spikes.md` §1.2; GPUI
    /// calls `SetCapture` on the press). Capturing the pointer
    /// on the press keeps GPUI's capture set until the button comes up, and the geometry clamps
    /// positions past the edge to it.
    ///
    /// The capture is checked as `captured_hitbox().is_some()`, not through the hitbox's id:
    /// GPUI hands out a fresh `HitboxId` on every redraw (`next_hitbox_id` is never reset), the
    /// press redraws, and from then on the captured id is one no listener holds, so
    /// `is_hovered` would be back to plain hit testing and fail past the edge. This layer is the
    /// only thing in the window that captures, so any capture is this drag.
    fn mouse_layer(&self, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let id = hitbox.id;
                let on_down = view.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && id.is_hovered(window)
                    {
                        window.capture_pointer(id);
                        let scale = window.scale_factor();
                        on_down.update(cx, |this, cx| this.press(event.position, scale, cx));
                    }
                });
                let on_move = view.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && (window.captured_hitbox().is_some() || id.is_hovered(window))
                    {
                        let scale = window.scale_factor();
                        on_move.update(cx, |this, cx| this.moved(event.position, scale, cx));
                    }
                });
                // A release anywhere ends the drag. GPUI clears the capture after this event.
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && (window.captured_hitbox().is_some() || id.is_hovered(window))
                    {
                        view.update(cx, |this, cx| this.confirm(cx));
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }
}

/// A div placed over frame pixels. Dividing by the scale keeps its edges on device pixels.
fn place(rect: Rect, scale: f32) -> Div {
    let logical = |value: i32| px(value as f32 / scale);
    div()
        .absolute()
        .left(logical(rect.x))
        .top(logical(rect.y))
        .w(logical(rect.width))
        .h(logical(rect.height))
}

/// A line around `rect`, `width` logical pixels thick, drawn `outside` logical pixels beyond the
/// rect's edge so the pixels inside stay untouched.
fn outline(rect: Rect, scale: f32, outside: Pixels, width: Pixels, color: Hsla) -> Div {
    let logical = |value: i32| px(value as f32 / scale);
    div()
        .absolute()
        .left(logical(rect.x) - outside)
        .top(logical(rect.y) - outside)
        .w(logical(rect.width) + outside * 2.)
        .h(logical(rect.height) + outside * 2.)
        .border(width)
        .border_color(color)
}

fn pixel_color(pixel: xcap::image::Rgba<u8>) -> Hsla {
    Rgba {
        r: f32::from(pixel[0]) / 255.,
        g: f32::from(pixel[1]) / 255.,
        b: f32::from(pixel[2]) / 255.,
        a: 1.,
    }
    .into()
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let (frame_width, frame_height) = self.frame_size();
        let frame_rect = Rect {
            x: 0,
            y: 0,
            width: frame_width as i32,
            height: frame_height as i32,
        };
        let theme = cx.theme();
        let o = &theme.overlay;
        let rect = self.lit(frame_rect);
        let dimmed = match rect {
            Some(rect) => geometry::around(rect, frame_rect.width, frame_rect.height),
            None => vec![frame_rect],
        };
        // The panel follows the pointer, so it belongs to the monitor the pointer is on. The
        // platform never reports the pointer leaving through a mouse event here, but it does
        // clear the window's hovered flag; a drag keeps the panel, since the pointer may be past
        // this monitor's edge while the drag goes on.
        let panel = self
            .pointer
            .filter(|_| self.selection.is_some() || window.is_window_hovered())
            .map(|pointer| self.panel(pointer, rect, scale, frame_rect, window, cx));

        div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .map(|this| match self.mode {
                CaptureMode::Region => this.cursor_crosshair(),
                _ => this.cursor_default(),
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.cancel(cx),
                    "enter" => this.confirm(cx),
                    _ => {}
                }
            }))
            .child(
                img(self.image.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .children(dimmed.into_iter().map(|part| place(part, scale).bg(o.dim)))
            // The halo sits just outside the edge, so the edge reads on light and dark alike.
            .children(
                rect.filter(|rect| rect.width > 0 && rect.height > 0)
                    .map(|rect| {
                        let edge = o.edge.width;
                        div()
                            .child(outline(rect, scale, edge + px(1.), px(1.), o.edge_halo))
                            .child(outline(rect, scale, edge, edge, o.edge.color))
                    }),
            )
            .children(panel)
            .child(self.mouse_layer(cx))
    }
}

impl Overlay {
    /// The panel beside the pointer: in Region the magnifier over the readout line and a hint,
    /// in Window the picked window's size and title over the hint.
    fn panel(
        &self,
        pointer: Pt,
        rect: Option<Rect>,
        scale: f32,
        frame: Rect,
        window: &Window,
        cx: &App,
    ) -> Div {
        let theme = cx.theme();
        let c = &theme.colors;
        let o = &theme.overlay;
        let readout_style = theme.type_scale.readout;
        let title_style = theme.type_scale.toast;
        let hint_style = theme.type_scale.hint;
        let region = self.mode == CaptureMode::Region;
        let picked = self.picked.and_then(|index| self.windows.get(index));

        // In Window the whole window's size, not just the part on this monitor.
        let readout = match (picked, rect) {
            (Some(window), _) if !region => geometry::size_text(window.bounds),
            (_, Some(rect)) => geometry::size_text(rect),
            (_, None) => geometry::position_text(pointer),
        };
        let title = picked
            .filter(|_| !region)
            .map(|window| window.title.clone());
        let hint = match (region, rect.is_some()) {
            (true, true) => "Release or Enter to capture · Esc cancels",
            (true, false) => "Drag to select · Esc cancels",
            (false, true) => "Click or Enter to capture · Esc cancels",
            (false, false) => "Point at a window · Esc cancels",
        };

        let loupe_size = if region {
            LOUPE_CELLS as f32 * LOUPE_ZOOM + f32::from(o.loupe.width) * 2.
        } else {
            0.
        };
        let readout_width = f32::from(readout_style.text_width(&readout, theme, window));
        let title_width = title.as_ref().map_or(0., |title| {
            f32::from(title_style.text_width(title, theme, window)).min(TITLE_MAX_WIDTH)
        });
        let hint_width = f32::from(hint_style.text_width(hint, theme, window));
        let readout_height = f32::from(readout_style.natural_line_height(theme, cx));
        let title_height = title.as_ref().map_or(0., |_| {
            f32::from(title_style.natural_line_height(theme, cx))
        });
        let hint_height = f32::from(hint_style.natural_line_height(theme, cx));
        let border = f32::from(theme.borders.toast.width);
        let panel_width = loupe_size
            .max(readout_width)
            .max(title_width)
            .max(hint_width)
            + 2. * (PANEL_PADDING + border);
        let panel_height = loupe_size
            + if region { PANEL_SPACING } else { 0. }
            + readout_height
            + title_height
            + hint_height
            + 2. * (PANEL_PADDING + border);

        // The pointer and the frame in logical pixels, for placing the panel.
        let pointer_logical = (pointer.0 as f32 / scale, pointer.1 as f32 / scale);
        let frame_logical = (frame.width as f32 / scale, frame.height as f32 / scale);
        let (left, top) = geometry::panel_origin(
            pointer_logical,
            (panel_width, panel_height),
            frame_logical,
            PANEL_GAP,
        );

        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(panel_width))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(PANEL_SPACING))
            .p(px(PANEL_PADDING))
            .bg(c.toast_bg)
            .text_color(c.toast_text)
            .border(theme.borders.toast.width)
            .border_color(theme.borders.toast.color)
            .rounded(theme.radii.md)
            .shadow(theme.shadows.pop.clone())
            .when(region, |panel| {
                panel.child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .child(self.loupe(pointer, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .child(
                        div()
                            .type_style(theme, readout_style)
                            .line_height(px(readout_height))
                            .whitespace_nowrap()
                            .child(readout_style.case.apply(&readout)),
                    )
                    .children(title.map(|title| {
                        div()
                            .max_w(px(TITLE_MAX_WIDTH))
                            .type_style(theme, title_style)
                            .line_height(px(title_height))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title_style.case.apply(&title))
                    }))
                    .child(
                        div()
                            .type_style(theme, hint_style)
                            .line_height(px(hint_height))
                            .whitespace_nowrap()
                            .child(hint_style.case.apply(hint)),
                    ),
            )
    }

    /// The magnifier: the frame pixels around the pointer, blown up, with the centre cell
    /// outlined and one-pixel hairlines crossing through it.
    fn loupe(&self, pointer: Pt, cx: &App) -> Div {
        let theme = cx.theme();
        let c = &theme.colors;
        let o = &theme.overlay;
        let pixels = geometry::loupe_pixels(&self.frame, pointer, LOUPE_CELLS);
        let cell = px(LOUPE_ZOOM);
        let rows = pixels.chunks(LOUPE_CELLS as usize).map(|row| {
            div().flex().children(row.iter().map(|pixel| {
                div()
                    .w(cell)
                    .h(cell)
                    .bg(pixel.map(pixel_color).unwrap_or(c.toast_bg))
            }))
        });
        let centre = px((LOUPE_CELLS / 2) as f32 * LOUPE_ZOOM);
        let middle = centre + cell / 2. - px(0.5);
        let hairline = Hsla {
            a: 0.5,
            ..o.crosshair
        };
        let inner = px(LOUPE_CELLS as f32 * LOUPE_ZOOM);
        div()
            .relative()
            .flex()
            .flex_col()
            .border(o.loupe.width)
            .border_color(o.loupe.color)
            .children(rows)
            .child(
                div()
                    .absolute()
                    .left(middle)
                    .top_0()
                    .w_px()
                    .h(inner)
                    .bg(hairline),
            )
            .child(
                div()
                    .absolute()
                    .top(middle)
                    .left_0()
                    .h_px()
                    .w(inner)
                    .bg(hairline),
            )
            .child(
                div()
                    .absolute()
                    .left(centre)
                    .top(centre)
                    .w(cell)
                    .h(cell)
                    .border_1()
                    .border_color(o.crosshair),
            )
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Lists the real windows in memory. Nothing is shown or printed.
    #[test]
    fn listed_windows_are_pickable_and_on_screen() {
        for window in list_windows() {
            assert!(window.bounds.width > 0 && window.bounds.height > 0);
            assert!(!window.title.is_empty());
        }
    }
}
