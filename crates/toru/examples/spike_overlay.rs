//! Spike for `docs/spikes.md` section 1.2: capture every monitor, then cover each one with a
//! borderless topmost window showing the frozen image, dimmed, to select a region from.
//!
//! Set `TORU_SPIKE_AUTOCLOSE_MS` to close everything after that long and print a report.

use std::{
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, Context, Div, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, Pixels, PlatformDisplay, Point, Render, RenderImage, Window,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, black, div, img, prelude::*, px, white,
};
use gpui_platform::application;
use xcap::{
    Monitor,
    image::{Frame, Rgba, RgbaImage, imageops},
};

const DIM_OPACITY: f32 = 0.5;
/// A recaptured pixel further than this from the expected value counts as wrong in the report.
const OFF_THRESHOLD: u32 = 3;
const READOUT_SIZE: (u32, u32) = (120, 24);
/// The auto-close run selects this, in physical pixels, so the crop runs without mouse input.
const SCRIPTED_SELECTION: Selection = Selection {
    anchor: (100, 80),
    cursor: (499, 279),
};

struct Capture {
    name: String,
    monitor: Monitor,
    frame: Arc<RgbaImage>,
}

/// Both corners in physical pixels, each the pixel under the pointer, so the edges are inclusive.
#[derive(Clone, Copy)]
struct Selection {
    anchor: (u32, u32),
    cursor: (u32, u32),
}

#[derive(Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Selection {
    fn rect(self) -> Rect {
        Rect {
            x: self.anchor.0.min(self.cursor.0),
            y: self.anchor.1.min(self.cursor.1),
            width: self.anchor.0.abs_diff(self.cursor.0) + 1,
            height: self.anchor.1.abs_diff(self.cursor.1) + 1,
        }
    }
}

impl Rect {
    fn contains(self, x: u32, y: u32) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }

    /// Where the size readout goes: just below the selection, so the selection stays clear.
    fn readout(self, frame_height: u32) -> Rect {
        Rect {
            x: self.x,
            y: (self.y + self.height).min(frame_height - READOUT_SIZE.1),
            width: READOUT_SIZE.0,
            height: READOUT_SIZE.1,
        }
    }
}

struct Overlay {
    name: String,
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
    focus_handle: FocusHandle,
    selection: Option<Selection>,
    dragging: bool,
    started: Instant,
    rendered: bool,
}

impl Overlay {
    fn physical_point(&self, position: Point<Pixels>, scale: f32) -> (u32, u32) {
        let (width, height) = self.frame.dimensions();
        let to_physical = |value: Pixels| (f32::from(value) * scale).round() as u32;
        (
            to_physical(position.x).min(width - 1),
            to_physical(position.y).min(height - 1),
        )
    }

    fn print_crop(&self) {
        let Some(selection) = self.selection else {
            return;
        };
        let Rect {
            x,
            y,
            width,
            height,
        } = selection.rect();
        let crop = imageops::crop_imm(self.frame.as_ref(), x, y, width, height).to_image();
        println!(
            "{}: selected {width}x{height} physical px at ({x}, {y}), crop is {}x{}",
            self.name,
            crop.width(),
            crop.height(),
        );
    }

    fn finish_drag(&mut self) {
        if self.dragging {
            self.dragging = false;
            self.print_crop();
        }
    }
}

/// A div placed in physical pixels. Dividing by the scale keeps the edges on device pixels.
fn place(x: u32, y: u32, width: u32, height: u32, scale: f32) -> Div {
    let logical = |value: u32| px(value as f32 / scale);
    div()
        .absolute()
        .left(logical(x))
        .top(logical(y))
        .w(logical(width))
        .h(logical(height))
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.rendered {
            self.rendered = true;
            println!(
                "{}: first render {:?} after the capture started",
                self.name,
                self.started.elapsed()
            );
        }

        let scale = window.scale_factor();
        let (frame_width, frame_height) = self.frame.dimensions();
        let rect = self.selection.map(Selection::rect);

        // Four parts around the selection, so the image shows through undimmed inside it.
        let dimmed_parts = match rect {
            None => vec![(0, 0, frame_width, frame_height)],
            Some(Rect {
                x,
                y,
                width,
                height,
            }) => vec![
                (0, 0, frame_width, y),
                (0, y + height, frame_width, frame_height - (y + height)),
                (0, y, x, height),
                (x + width, y, frame_width - (x + width), height),
            ],
        };

        div()
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .cursor_crosshair()
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    println!("Esc pressed, closing every overlay");
                    cx.defer(close_overlays);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    let point = this.physical_point(event.position, window.scale_factor());
                    this.selection = Some(Selection {
                        anchor: point,
                        cursor: point,
                    });
                    this.dragging = true;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.dragging {
                    let point = this.physical_point(event.position, window.scale_factor());
                    if let Some(selection) = &mut this.selection {
                        selection.cursor = point;
                    }
                    cx.notify();
                }
            }))
            // GPUI keeps sending the drag to this window, but a release beyond its edge counts as "out".
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.finish_drag()),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.finish_drag()),
            )
            .child(
                img(self.image.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .children(dimmed_parts.into_iter().map(|(x, y, width, height)| {
                place(x, y, width, height, scale).bg(black().opacity(DIM_OPACITY))
            }))
            .children(rect.map(|rect| {
                let readout = rect.readout(frame_height);
                place(readout.x, readout.y, readout.width, readout.height, scale)
                    .bg(black())
                    .text_color(white())
                    .text_sm()
                    .child(format!("{} x {} px", rect.width, rect.height))
            }))
    }
}

fn close_overlays(cx: &mut App) {
    for window in cx.windows() {
        window
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
}

fn capture_monitors(started: Instant) -> Vec<Capture> {
    let monitors = Monitor::all().expect("failed to list monitors");
    println!(
        "listed {} monitors in {:?}",
        monitors.len(),
        started.elapsed()
    );

    let captures = monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let monitor_started = Instant::now();
            let frame = monitor
                .capture_image()
                .expect("failed to capture a monitor");
            let capture_time = monitor_started.elapsed();
            // GPUI blends images with their alpha, so anything not opaque would be drawn darker.
            let translucent = frame.pixels().filter(|pixel| pixel[3] != 255).count();
            let name = format!("monitor {index}");
            println!(
                "{name}: captured {}x{} in {capture_time:?}, {translucent} pixels not opaque \
                 ({:.0} Hz, xcap scale {}, primary {})",
                frame.width(),
                frame.height(),
                monitor.frequency().unwrap_or(0.0),
                monitor.scale_factor().unwrap_or(0.0),
                monitor.is_primary().unwrap_or(false),
            );
            Capture {
                name,
                monitor,
                frame: Arc::new(frame),
            }
        })
        .collect();
    println!("captured everything in {:?}", started.elapsed());
    captures
}

fn open_overlay(
    capture: &Capture,
    display: &dyn PlatformDisplay,
    started: Instant,
    cx: &mut App,
) -> WindowHandle<Overlay> {
    // GPUI's renderer takes BGRA, and the original frame stays RGBA for cropping.
    let mut bgra = capture.frame.as_ref().clone();
    bgra.pixels_mut().for_each(|pixel| pixel.0.swap(0, 2));
    let image = Arc::new(RenderImage::new([Frame::new(bgra)]));

    let options = WindowOptions {
        // Fullscreen strips PopUp's frame and sizes the window to the monitor's physical bounds.
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
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Overlay {
                name: capture.name.clone(),
                frame: capture.frame.clone(),
                image,
                focus_handle,
                selection: None,
                dragging: false,
                started,
                rendered: false,
            }
        })
    })
    .expect("failed to open an overlay window")
}

fn print_bounds(capture: &Capture, window: &Window) {
    let monitor = &capture.monitor;
    let scale = window.scale_factor();
    let bounds = window.bounds().to_device_pixels(scale);
    let actual = (
        bounds.origin.x.0,
        bounds.origin.y.0,
        bounds.size.width.0,
        bounds.size.height.0,
    );
    let expected = (
        monitor.x().unwrap_or(i32::MIN),
        monitor.y().unwrap_or(i32::MIN),
        monitor.width().unwrap_or(0) as i32,
        monitor.height().unwrap_or(0) as i32,
    );
    println!(
        "{}: window {actual:?} at scale {scale}, monitor {expected:?}, match {}, fullscreen {}",
        capture.name,
        actual == expected,
        window.is_fullscreen(),
    );
}

/// How far one zone of a recapture is from what the overlay should be showing.
#[derive(Default)]
struct Tally {
    /// Pixels by their largest channel error: 0, 1, 2 to `OFF_THRESHOLD`, and anything above.
    errors: [u64; 4],
    worst: u32,
    /// The smallest box around the wrong pixels, as inclusive (left, top, right, bottom).
    off_box: Option<(u32, u32, u32, u32)>,
}

impl Tally {
    fn add(&mut self, x: u32, y: u32, error: u32) {
        self.worst = self.worst.max(error);
        let bucket = match error {
            0 | 1 => error as usize,
            _ if error <= OFF_THRESHOLD => 2,
            _ => 3,
        };
        self.errors[bucket] += 1;
        if bucket == 3 {
            let (left, top, right, bottom) = self.off_box.unwrap_or((x, y, x, y));
            self.off_box = Some((left.min(x), top.min(y), right.max(x), bottom.max(y)));
        }
    }
}

fn pixel_error(shown: &Rgba<u8>, original: &Rgba<u8>, factor: f32) -> u32 {
    (0..3)
        .map(|channel| {
            let expected = f32::from(original[channel]) * factor;
            (f32::from(shown[channel]) - expected).abs() as u32
        })
        .max()
        .unwrap_or(0)
}

/// Compares a capture taken while the overlay was up against what the overlay should be showing.
/// Prints only counts and positions, so nothing about the screen's content leaves this function.
fn print_pixel_check(
    name: &str,
    screen: &RgbaImage,
    frame: &RgbaImage,
    selection: Option<Selection>,
) {
    if screen.dimensions() != frame.dimensions() {
        println!("{name}: pixel check skipped, the recapture has a different size");
        return;
    }

    let rect = selection.map(Selection::rect);
    let readout = rect.map(|rect| rect.readout(frame.height()));
    let is_clear = |x, y| rect.is_some_and(|rect| rect.contains(x, y));
    let is_readout = |x, y| readout.is_some_and(|readout| readout.contains(x, y));
    let factor = |x, y| {
        if is_clear(x, y) {
            1.0
        } else {
            1.0 - DIM_OPACITY
        }
    };

    let (mut dimmed, mut clear) = (Tally::default(), Tally::default());
    for (x, y, shown) in screen
        .enumerate_pixels()
        .filter(|&(x, y, _)| !is_readout(x, y))
    {
        let tally = if is_clear(x, y) {
            &mut clear
        } else {
            &mut dimmed
        };
        tally.add(
            x,
            y,
            pixel_error(shown, frame.get_pixel(x, y), factor(x, y)),
        );
    }
    for (zone, tally) in [("dimmed", &dimmed), ("selection", &clear)] {
        println!(
            "{name}: {zone} errors [0, 1, 2-{OFF_THRESHOLD}, more] = {:?}, worst {}, wrong pixels in {:?}",
            tally.errors, tally.worst, tally.off_box,
        );
    }

    // A misaligned image would make one of the shifted comparisons beat the unshifted one.
    for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
        let wrong = screen
            .enumerate_pixels()
            .filter(|&(x, y, shown)| {
                let original =
                    frame.get_pixel_checked((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                !is_readout(x, y)
                    && original.is_some_and(|original| {
                        pixel_error(shown, original, factor(x, y)) > OFF_THRESHOLD
                    })
            })
            .count();
        println!("{name}: frame shifted by ({dx}, {dy}) gives {wrong} wrong pixels");
    }
}

fn report_and_close(
    captures: &[Capture],
    overlays: &[WindowHandle<Overlay>],
    started: Instant,
    cx: &mut App,
) {
    println!(
        "auto-close fired {:?} after the capture started",
        started.elapsed()
    );

    let mut recaptures = Vec::new();
    for (capture, overlay) in captures.iter().zip(overlays) {
        overlay
            .update(cx, |overlay, window, _| {
                print_bounds(capture, window);
                let recapture_started = Instant::now();
                let screen = capture.monitor.capture_image();
                println!(
                    "{}: recapture took {:?}",
                    capture.name,
                    recapture_started.elapsed()
                );
                recaptures.push((capture, screen, overlay.selection));
            })
            .ok();
    }

    // Dropped windows only hide when the message loop next runs, so the overlays stay up during this slow comparison.
    close_overlays(cx);
    for (capture, screen, selection) in recaptures {
        match screen {
            Ok(screen) => print_pixel_check(&capture.name, &screen, &capture.frame, selection),
            Err(error) => println!("{}: recapture failed: {error}", capture.name),
        }
    }
}

fn main() {
    let started = Instant::now();
    let captures = capture_monitors(started);
    let autoclose = env::var("TORU_SPIKE_AUTOCLOSE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis);

    application().run(move |cx: &mut App| {
        let displays = cx.displays();
        let overlays: Vec<_> = captures
            .iter()
            .map(|capture| {
                // Both libraries identify a monitor by its HMONITOR, and xcap truncates it to 32 bits.
                let monitor_id = capture.monitor.id().expect("failed to read the monitor id");
                let display = displays
                    .iter()
                    .find(|display| u64::from(display.id()) as u32 == monitor_id)
                    .expect("GPUI doesn't list a monitor that xcap captured");
                open_overlay(capture, display.as_ref(), started, cx)
            })
            .collect();
        println!(
            "opened {} windows {:?} after the capture started",
            overlays.len(),
            started.elapsed()
        );

        let Some(delay) = autoclose else {
            return;
        };
        for overlay in &overlays {
            overlay
                .update(cx, |overlay, _, cx| {
                    overlay.selection = Some(SCRIPTED_SELECTION);
                    overlay.print_crop();
                    cx.notify();
                })
                .ok();
        }
        cx.spawn(async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|cx| report_and_close(&captures, &overlays, started, cx));
        })
        .detach();
    });
}
