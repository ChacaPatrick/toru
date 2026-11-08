//! The little the app needs from the operating system that GPUI 0.3.11 doesn't offer: restoring
//! a maximized window, a display's size in physical pixels, the local time, the app-data folder
//! and keeping Toru's own windows out of a capture. Windows is done here; the other platforms get
//! the fallbacks at the bottom until they are.

use std::{path::PathBuf, time::Duration};

use gpui::PlatformDisplay;
#[cfg(not(windows))]
use toru_core::time::LocalDateTime;

#[cfg(windows)]
mod windows {
    use gpui::{App, PlatformDisplay, Window};
    use toru_core::time::{LocalDate, LocalDateTime};

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

    /// `SYSTEMTIME`.
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    const SW_NORMAL: i32 = 1;
    const MONITOR_DEFAULTTONULL: u32 = 0;
    const WDA_NONE: u32 = 0;
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

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
        fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
        fn GetLocalTime(time: *mut SystemTime);
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

    pub fn local_now() -> LocalDateTime {
        let mut now = SystemTime::default();
        // SAFETY: `now` is a writable SYSTEMTIME.
        unsafe { GetLocalTime(&mut now) };
        LocalDateTime {
            date: LocalDate {
                year: now.year,
                month: now.month as u8,
                day: now.day as u8,
            },
            hour: now.hour as u8,
            minute: now.minute as u8,
        }
    }

    unsafe extern "system" fn set_affinity(hwnd: isize, affinity: isize) -> i32 {
        // SAFETY: a plain user32 call on a handle the enumeration just handed over. A window that
        // can't take it isn't on screen to begin with, so a failure doesn't matter.
        unsafe { SetWindowDisplayAffinity(hwnd, affinity as u32) };
        1
    }

    /// Takes this thread's windows out of what captures see, or puts them back. They stay on
    /// screen and in place. GPUI opens all of them on this thread.
    /// Tells the user why the app can't start, since a release build has no console.
    pub fn show_startup_error(message: &str) {
        const MB_ICONERROR: u32 = 0x10;
        let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, caption) = (wide(message), wide("Toru"));
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR) };
    }

    pub fn exclude_windows_from_capture(exclude: bool) {
        let affinity = if exclude {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: the callback only passes each handle on, and this returns when enumeration ends.
        unsafe {
            EnumThreadWindows(GetCurrentThreadId(), set_affinity, affinity as isize);
        }
    }
}

/// How long a capture waits after `exclude_windows_from_capture(true)`. Exclusion took effect with
/// no wait in every trial (`docs/spikes.md` §5.1); this is a margin of a few frames that nobody
/// will notice.
pub const EXCLUDE_SETTLE: Duration = Duration::from_millis(50);

#[cfg(windows)]
pub use windows::{exclude_windows_from_capture, local_now, restore_window, show_startup_error};

#[cfg(not(windows))]
pub fn restore_window(_window: &gpui::Window, _cx: &gpui::App) {}

#[cfg(not(windows))]
pub fn exclude_windows_from_capture(_exclude: bool) {}

#[cfg(not(windows))]
pub fn show_startup_error(message: &str) {
    eprintln!("{message}");
}

#[cfg(not(windows))]
pub fn local_now() -> LocalDateTime {
    unimplemented!("local time isn't written for this platform yet")
}

/// Where the store lives: `TORU_DATA_DIR` if set, which tests use, else `%LOCALAPPDATA%\Toru`.
pub fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TORU_DATA_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("LOCALAPPDATA")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("Toru")
}

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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn the_local_time_is_a_real_date() {
        let now = local_now();
        assert!(now.date.year >= 2026);
        assert!((1..=12).contains(&now.date.month));
        assert!((1..=31).contains(&now.date.day));
        assert!(now.hour < 24 && now.minute < 60);
    }
}
