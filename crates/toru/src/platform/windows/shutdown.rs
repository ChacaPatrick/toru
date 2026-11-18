use super::main_window::main_window;
use super::utf16::wide;

#[link(name = "user32")]
unsafe extern "system" {
    fn ShutdownBlockReasonCreate(hwnd: isize, reason: *const u16) -> i32;
    fn ShutdownBlockReasonDestroy(hwnd: isize) -> i32;
}

/// Windows shows `reason` if it ends the session while the write still holds it up, rather than
/// an app that doesn't say why.
pub fn while_holding_up_shutdown<T>(reason: &str, write: impl FnOnce() -> T) -> T {
    let hwnd = main_window();
    if let Some(hwnd) = hwnd {
        // SAFETY: a plain user32 call on a window of this thread's, with a terminated string
        // that outlives it (Windows copies the reason).
        unsafe { ShutdownBlockReasonCreate(hwnd, wide(reason).as_ptr()) };
    }
    let result = write();
    if let Some(hwnd) = hwnd {
        // SAFETY: as above.
        unsafe { ShutdownBlockReasonDestroy(hwnd) };
    }
    result
}
