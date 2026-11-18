use std::rc::Rc;

use gpui::{
    App, Global, PlatformDisplay, Subscription, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, prelude::*,
};
use toru_core::capture::CaptureMode;
use xcap::image::RgbaImage;

use super::freeze::{Captured, Frozen, freeze};
use super::{MonitorFrame, Overlay, TopWindow};
use crate::platform;

pub type OnCapture = Rc<dyn Fn(RgbaImage, CaptureMode, &mut App)>;

/// Only one runs at a time.
pub(super) struct Session {
    mode: CaptureMode,
    pub(super) overlays: Vec<WindowHandle<Overlay>>,
    /// Ends the capture when any overlay is closed from outside, such as by Alt+F4.
    _closed: Subscription,
    on_capture: OnCapture,
    /// Told when no overlay could be opened, which would otherwise look like nothing happening.
    on_failed: Box<dyn FnOnce(&mut App)>,
}

impl Global for Session {}

/// One overlay is given the foreground, since another app holds it after a delayed capture.
/// `on_capture` runs on the main thread once the user confirms.
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

/// Only on captured monitors GPUI also knows.
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

pub(super) struct Shared {
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

/// Windows only leave the screen when the message loop next runs, so anything slow with the image
/// belongs to the receiver, off the main thread.
pub(super) fn finish(image: Option<RgbaImage>, cx: &mut App) {
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
