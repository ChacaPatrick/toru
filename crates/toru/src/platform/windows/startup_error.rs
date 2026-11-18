use super::utf16::wide;
use crate::main_window::WINDOW_TITLE;

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

/// Tells the user why the app can't start, since a release build has no console.
pub fn show_startup_error(message: &str) {
    const MB_ICONERROR: u32 = 0x10;
    let (text, caption) = (wide(message), wide(WINDOW_TITLE));
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR) };
}
