use gpui::{App, Window};

const SW_HIDE: i32 = 0;
const SW_NORMAL: i32 = 1;
const SW_SHOW: i32 = 5;
const SW_RESTORE: i32 = 9;
const WM_CLOSE: u32 = 0x0010;
const GPUI_WINDOW_CLASS: &str = "Zed::Window";
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const MONITOR_DEFAULTTONULL: u32 = 0;
const GWL_EXSTYLE: i32 = -20;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumThreadWindows(
        thread: u32,
        callback: unsafe extern "system" fn(isize, isize) -> i32,
        param: isize,
    ) -> i32;
    fn IsZoomed(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn ShowWindowAsync(hwnd: isize, cmd: i32) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn PostMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn GetClassNameW(hwnd: isize, name: *mut u16, max: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

fn class_name(hwnd: isize) -> String {
    let mut buffer = [0u16; 64];
    // SAFETY: the buffer is writable and its length is passed along.
    let length = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

/// The extended style GPUI gives a `WindowKind::Normal` window, which only the main window
/// is: the overlays are pop-ups (`is_overlay_style`), and the hidden windows GPUI and the
/// tray keep for their messages are tool windows. The style survives retitling, hiding and
/// minimizing, which the title the window is opened with does not: a delayed capture counts
/// down in it.
pub fn is_main_window_style(ex_style: u32) -> bool {
    ex_style & WS_EX_APPWINDOW != 0
}

unsafe extern "system" fn find_main(hwnd: isize, param: isize) -> i32 {
    // SAFETY: plain user32 calls on a handle the enumeration just handed over.
    let matches = unsafe {
        is_main_window_style(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32)
            && class_name(hwnd) == GPUI_WINDOW_CLASS
    };
    if matches {
        // SAFETY: `param` is the `isize` that `main_window` passed and still holds.
        unsafe { *(param as *mut isize) = hwnd };
    }
    i32::from(!matches)
}

/// The one GPUI window of this thread's with an app window's style, even hidden or retitled.
pub(super) fn main_window() -> Option<isize> {
    let mut found: isize = 0;
    // SAFETY: `found` outlives the enumeration, which runs to completion before returning.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            find_main,
            &mut found as *mut isize as isize,
        );
    }
    (found != 0).then_some(found)
}

/// Hidden in the tray (`hide_main_window`), as against minimized.
pub fn main_window_hidden() -> bool {
    // SAFETY: a plain user32 call on a window of this thread's.
    main_window().is_some_and(|hwnd| unsafe { IsWindowVisible(hwnd) } == 0)
}

/// Off the screen, the taskbar and Alt+Tab. Posted rather than sent, since this runs inside the
/// window's own close handling.
pub fn hide_main_window() {
    if let Some(hwnd) = main_window() {
        // SAFETY: a plain user32 call on a window of this thread's.
        unsafe { ShowWindowAsync(hwnd, SW_HIDE) };
    }
}

/// Windows only lets the foreground move when the user asked for it, as a tray click or a second
/// launch did; otherwise the taskbar button flashes instead.
pub fn show_main_window() {
    if let Some(hwnd) = main_window() {
        // SAFETY: plain user32 calls on a window of this thread's.
        unsafe {
            let cmd = if IsIconic(hwnd) != 0 {
                SW_RESTORE
            } else {
                SW_SHOW
            };
            ShowWindow(hwnd, cmd);
            SetForegroundWindow(hwnd);
        }
    }
}

/// Closes the way the caption's X does, so the close goes through
/// `Window::on_window_should_close`. GPUI's `remove_window` would skip that.
pub fn request_close(_window: &mut Window) {
    if let Some(hwnd) = main_window() {
        // SAFETY: a plain user32 call on a window of this thread's.
        unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    }
}

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

/// GPUI's `zoom_window` only maximizes, and the caption button's restore runs inside Windows' own
/// click handling, which a click simulated by assistive technology never reaches. GPUI doesn't
/// hand out the `HWND` without `raw-window-handle`, so the window is found among this thread's
/// top-level windows as the maximized one on `window`'s display: a display holds at most one
/// maximized window of Toru's.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_app_window_counts_as_the_main_window() {
        const TOPMOST_TOOL: u32 = 0x88;
        const APP_WINDOW: u32 = 0x40000;
        const NO_REDIRECTION_BITMAP: u32 = 0x0020_0000;
        assert!(is_main_window_style(APP_WINDOW));
        assert!(is_main_window_style(APP_WINDOW | NO_REDIRECTION_BITMAP));
        assert!(!is_main_window_style(TOPMOST_TOOL));
        assert!(!is_main_window_style(0x80));
        assert!(!is_main_window_style(0));
    }
}
