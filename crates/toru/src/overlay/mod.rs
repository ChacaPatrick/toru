//! The region capture overlay (DESIGN.md §6), built the way `docs/spikes.md` §1.2 proved: Toru's
//! own windows are taken out of what captures see (§5.1), every monitor is captured into memory
//! on a background thread, then one borderless topmost window per monitor shows its frozen
//! frame, dimmed. A drag selects a region, with the size in physical pixels and a magnifier in a
//! panel beside the pointer. Releasing or Enter confirms; Esc on any monitor closes them all, and
//! so does an overlay closed any other way, such as Alt+F4. The crop comes from the original
//! frame, so what the user gets is what was on screen, not a readback of the overlay.
//!
//! Everything is in physical pixels and drawn at `physical / scale`, so edges land on device
//! pixels at any scale factor. Each monitor's overlay works in its own frame, so negative monitor
//! origins never enter the arithmetic.

mod geometry;

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyWindowHandle, App, Context, Div, FocusHandle, Global, Hsla, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels,
    PlatformDisplay, Point, Render, RenderImage, Rgba, Subscription, Window, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, img, prelude::*, px,
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

/// What the overlay hands back: the cropped region.
pub type OnCapture = Rc<dyn Fn(RgbaImage, CaptureMode, &mut App)>;

/// One region capture in progress, from capturing the monitors to handing over the crop. Only
/// one runs at a time.
struct Session {
    overlays: Vec<AnyWindowHandle>,
    /// Ends the capture when any overlay is closed from outside, such as by Alt+F4.
    _closed: Subscription,
    on_capture: OnCapture,
}

impl Global for Session {}

/// One monitor's frame, captured off the main thread: the original for cropping and the
/// magnifier, and a BGRA copy ready for GPUI to draw.
struct Captured {
    /// `xcap`'s monitor id, the low 32 bits of the `HMONITOR` GPUI uses as its `DisplayId`.
    monitor_id: u32,
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
}

/// Starts a region capture: takes Toru's windows out of what captures see, captures every
/// monitor, and covers each one with an overlay. Toru's windows stay on screen, so Toru keeps the
/// foreground and the overlay gets the keyboard. `on_capture` runs on the main thread with the
/// crop once the user confirms.
pub fn start_region(cx: &mut App, on_capture: impl Fn(RgbaImage, CaptureMode, &mut App) + 'static) {
    if cx.has_global::<Session>() {
        return;
    }
    cx.set_global(Session {
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
    });
    platform::exclude_windows_from_capture(true);
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(platform::EXCLUDE_SETTLE)
            .await;
        let captured = cx.background_spawn(async { capture_monitors() }).await;
        // Cleared as soon as the frames are in, whatever they hold, so Toru is never left out of
        // another app's capture.
        platform::exclude_windows_from_capture(false);
        cx.update(|cx| open_overlays(captured, cx));
    })
    .detach();
}

/// Captures every monitor into memory. Slow, so it runs on a background thread; `xcap`'s monitor
/// handles are made and dropped there too, since they can't cross threads.
fn capture_monitors() -> Vec<Captured> {
    let monitors = Monitor::all().unwrap_or_default();
    monitors
        .into_iter()
        .filter_map(|monitor| {
            let monitor_id = monitor.id().ok()?;
            let frame = monitor.capture_image().ok()?;
            // GPUI's renderer takes BGRA; the original stays RGBA for the crop.
            let mut bgra = frame.clone();
            bgra.pixels_mut().for_each(|pixel| pixel.0.swap(0, 2));
            Some(Captured {
                monitor_id,
                frame: Arc::new(frame),
                image: Arc::new(RenderImage::new([Frame::new(bgra)])),
            })
        })
        .collect()
}

/// Opens an overlay on each captured monitor GPUI also knows. With nothing to show, the capture
/// ends as a cancel.
fn open_overlays(captured: Vec<Captured>, cx: &mut App) {
    let displays = cx.displays();
    let mut overlays = Vec::new();
    for capture in captured {
        let display = displays
            .iter()
            .find(|display| u64::from(display.id()) as u32 == capture.monitor_id);
        if let Some(display) = display
            && let Some(handle) = open_overlay(capture, display.as_ref(), cx)
        {
            overlays.push(handle.into());
        }
    }
    if overlays.is_empty() {
        finish(None, cx);
    } else if cx.has_global::<Session>() {
        cx.global_mut::<Session>().overlays = overlays;
    }
}

fn open_overlay(
    capture: Captured,
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
                frame: capture.frame,
                image: capture.image,
                focus,
                pointer: None,
                selection: None,
            }
        })
    })
    .ok()
}

/// Ends the capture: closes every overlay and, with a crop, hands it over. Windows only leave
/// the screen when the message loop next runs, so anything slow with the image belongs to the
/// receiver, off the main thread. Dropping the session also drops its window-closed watcher.
fn finish(crop: Option<RgbaImage>, cx: &mut App) {
    if !cx.has_global::<Session>() {
        return;
    }
    let session = cx.remove_global::<Session>();
    for overlay in session.overlays {
        overlay
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
    if let Some(crop) = crop {
        (session.on_capture)(crop, CaptureMode::Region, cx);
    }
}

/// One monitor's overlay: its frozen frame, the drag on it, and the pointer for the magnifier.
struct Overlay {
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
    focus: FocusHandle,
    /// The frame pixel under the pointer, once it has moved over this monitor.
    pointer: Option<Pt>,
    /// The drag in progress. Releasing confirms it, so it never outlives the drag.
    selection: Option<Selection>,
}

impl Overlay {
    fn frame_size(&self) -> (u32, u32) {
        self.frame.dimensions()
    }

    /// A window position as a pair of logical pixels.
    fn logical(position: Point<Pixels>) -> (f32, f32) {
        (f32::from(position.x), f32::from(position.y))
    }

    fn press(&mut self, position: Point<Pixels>, scale: f32) {
        let edge = geometry::edge_at(Self::logical(position), scale, self.frame_size());
        self.selection = Some(Selection::new(edge));
    }

    fn drag(&mut self, position: Point<Pixels>, scale: f32) {
        let position = Self::logical(position);
        let frame = self.frame_size();
        self.pointer = Some(geometry::pixel_at(position, scale, frame));
        if let Some(selection) = &mut self.selection {
            selection.cursor = geometry::edge_at(position, scale, frame);
        }
    }

    /// Confirms the drag, if it selected anything; a bare click selects nothing and waits.
    fn release(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.take() else {
            return;
        };
        if selection.is_empty() {
            cx.notify();
            return;
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
        // A window can't be closed from inside its own update.
        cx.defer(move |cx| finish(Some(crop), cx));
    }

    fn cancel(&self, cx: &mut Context<Self>) {
        cx.defer(|cx| finish(None, cx));
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
        let rect = self.selection.map(Selection::rect);
        let dimmed = match rect {
            Some(rect) => geometry::around(rect, frame_rect.width, frame_rect.height),
            None => vec![frame_rect],
        };

        div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .cursor_crosshair()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.cancel(cx),
                    "enter" => this.release(cx),
                    _ => {}
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.press(event.position, window.scale_factor());
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.drag(event.position, window.scale_factor());
                cx.notify();
            }))
            // The pointer has moved to another monitor, whose overlay shows the magnifier now.
            .on_mouse_exit(cx.listener(|this, _: &MouseExitEvent, _, cx| {
                if this.selection.is_none() {
                    this.pointer = None;
                    cx.notify();
                }
            }))
            // GPUI keeps the drag on this window, but a release past its edge arrives as "out".
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.release(cx)),
            )
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
            .children(
                self.pointer
                    .map(|pointer| self.panel(pointer, rect, scale, frame_rect, window, cx)),
            )
    }
}

impl Overlay {
    /// The panel beside the pointer: the magnifier over the readout line and a hint.
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
        let hint_style = theme.type_scale.hint;

        let readout = match rect {
            Some(rect) => geometry::size_text(rect),
            None => geometry::position_text(pointer),
        };
        let hint = if rect.is_some() {
            "Release or Enter to capture · Esc cancels"
        } else {
            "Drag to select · Esc cancels"
        };

        let loupe_size = LOUPE_CELLS as f32 * LOUPE_ZOOM + f32::from(o.loupe.width) * 2.;
        let readout_width = f32::from(readout_style.text_width(&readout, theme, window));
        let hint_width = f32::from(hint_style.text_width(hint, theme, window));
        let readout_height = f32::from(readout_style.natural_line_height(theme, cx));
        let hint_height = f32::from(hint_style.natural_line_height(theme, cx));
        let border = f32::from(theme.borders.toast.width);
        let panel_width =
            loupe_size.max(readout_width).max(hint_width) + 2. * (PANEL_PADDING + border);
        let panel_height = loupe_size
            + PANEL_SPACING
            + readout_height
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
        // The centre cell is outlined, and one-pixel hairlines cross the loupe through it.
        let centre = px((LOUPE_CELLS / 2) as f32 * LOUPE_ZOOM);
        let middle = centre + cell / 2. - px(0.5);
        let hairline = Hsla {
            a: 0.5,
            ..o.crosshair
        };
        let inner = px(LOUPE_CELLS as f32 * LOUPE_ZOOM);
        let loupe = div()
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
            .child(div().w_full().flex().justify_center().child(loupe))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .type_style(theme, readout_style)
                            .line_height(px(readout_height))
                            .whitespace_nowrap()
                            .child(readout_style.case.apply(&readout)),
                    )
                    .child(
                        div()
                            .type_style(theme, hint_style)
                            .line_height(px(hint_height))
                            .whitespace_nowrap()
                            .child(hint_style.case.apply(hint)),
                    ),
            )
    }
}
