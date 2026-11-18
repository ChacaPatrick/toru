use std::sync::Arc;

use gpui::{RenderImage, SharedString};
use toru_core::capture::CaptureMode;
use xcap::{
    Monitor,
    image::{Frame, RgbaImage},
};

use super::geometry::Rect;
use crate::platform;

/// One monitor's frame, captured off the main thread: the original for cropping and the
/// magnifier, and a BGRA copy ready for GPUI to draw.
pub(super) struct Captured {
    /// `xcap`'s monitor id, the low 32 bits of the `HMONITOR` GPUI uses as its `DisplayId`.
    pub(super) monitor_id: u32,
    /// Where the monitor sits on the desktop, in physical pixels.
    pub(super) bounds: Rect,
    pub(super) frame: Arc<RgbaImage>,
    pub(super) image: Arc<RenderImage>,
}

/// One monitor's frozen frame and its place on the desktop, shared by every overlay so a window
/// spanning monitors can be stitched from all of them.
pub(super) struct MonitorFrame {
    pub(super) bounds: Rect,
    pub(super) frame: Arc<RgbaImage>,
}

/// A top-level window as it stood when the screens froze, in desktop pixels.
pub(super) struct TopWindow {
    pub(super) title: SharedString,
    pub(super) bounds: Rect,
}

/// What the background thread brings back: the monitors, and in Window mode the windows.
pub(super) struct Frozen {
    pub(super) monitors: Vec<Captured>,
    pub(super) windows: Vec<TopWindow>,
}

/// Lists the windows, in Window mode, then captures every monitor into memory, so the list
/// matches the frames. Slow, so it runs on a background thread; `xcap`'s handles are made and
/// dropped there too, since they can't cross threads.
pub(super) fn freeze(mode: CaptureMode) -> Frozen {
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
