//! The little the window needs from the operating system that GPUI 0.3.11 doesn't offer:
//! restoring a maximized window and a display's size in physical pixels. Windows is done here;
//! the other platforms get the fallbacks at the bottom until they are.

use gpui::PlatformDisplay;

#[cfg(windows)]
mod windows {
    use gpui::{App, PlatformDisplay, Window};

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    /// `MONITORINFO`.
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    const SW_NORMAL: i32 = 1;
    const MONITOR_DEFAULTTONULL: u32 = 0;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumThreadWindows(
            thread: u32,
            callback: unsafe extern "system" fn(isize, isize) -> i32,
            param: isize,
        ) -> i32;
        fn IsZoomed(hwnd: isize) -> i32;
        fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
        fn ShowWindowAsync(hwnd: isize, cmd: i32) -> i32;
        fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    /// What `find_maximized` looks for and what it found.
    struct Search {
        monitor: isize,
        found: isize,
    }

    unsafe extern "system" fn find_maximized(hwnd: isize, param: isize) -> i32 {
        // SAFETY: `param` is the `Search` that `restore_window` passed and still holds.
        let search = unsafe { &mut *(param as *mut Search) };
        // SAFETY: plain user32 calls on a handle the enumeration just handed over.
        let matches = unsafe {
            IsZoomed(hwnd) != 0 && MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) == search.monitor
        };
        if matches {
            search.found = hwnd;
        }
        i32::from(!matches)
    }

    /// Takes `window` out of maximized. GPUI's `zoom_window` only maximizes, and the caption
    /// button's restore runs inside Windows' own click handling, which a click simulated by
    /// assistive technology never reaches. GPUI doesn't hand out the `HWND` without
    /// `raw-window-handle`, so the window is found among this thread's top-level windows (GPUI
    /// opens all of them on this thread) as the maximized one on `window`'s display: a display
    /// holds at most one maximized window of Toru's.
    pub fn restore_window(window: &Window, cx: &App) {
        let Some(display) = window.display(cx) else {
            return;
        };
        let mut search = Search {
            monitor: u64::from(display.id()) as isize,
            found: 0,
        };
        // SAFETY: `search` outlives the enumeration, which runs to completion before returning.
        unsafe {
            EnumThreadWindows(
                GetCurrentThreadId(),
                find_maximized,
                &mut search as *mut Search as isize,
            );
            if search.found != 0 {
                ShowWindowAsync(search.found, SW_NORMAL);
            }
        }
    }

    /// The display's size in physical pixels, which is what a capture of it has. GPUI's
    /// `DisplayId` is the `HMONITOR`, and `GetMonitorInfoW` answers in physical pixels for a
    /// per-monitor-DPI-aware process, which GPUI's are.
    pub fn display_pixels(display: &dyn PlatformDisplay) -> Option<(u32, u32)> {
        let mut info = MonitorInfo {
            size: size_of::<MonitorInfo>() as u32,
            monitor: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            work: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            flags: 0,
        };
        let monitor = u64::from(display.id()) as isize;
        // SAFETY: `info` is a writable MONITORINFO with its size filled in.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return None;
        }
        let width = u32::try_from(info.monitor.right - info.monitor.left).ok()?;
        let height = u32::try_from(info.monitor.bottom - info.monitor.top).ok()?;
        Some((width, height))
    }
}

#[cfg(windows)]
pub use windows::restore_window;

#[cfg(not(windows))]
pub fn restore_window(_window: &gpui::Window, _cx: &gpui::App) {}

/// The display's size in physical pixels, falling back to its logical size where the platform
/// code isn't written yet.
pub fn display_pixels(display: &dyn PlatformDisplay) -> (u32, u32) {
    #[cfg(windows)]
    if let Some(pixels) = windows::display_pixels(display) {
        return pixels;
    }
    let size = display.bounds().size;
    (
        f32::from(size.width).round() as u32,
        f32::from(size.height).round() as u32,
    )
}
