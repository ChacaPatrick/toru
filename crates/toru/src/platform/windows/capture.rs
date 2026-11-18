const WDA_NONE: u32 = 0;
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: u32 = 0x8;
const WS_EX_TOOLWINDOW: u32 = 0x80;
const WS_EX_TRANSPARENT: u32 = 0x20;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const LWA_ALPHA: u32 = 0x2;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumThreadWindows(
        thread: u32,
        callback: unsafe extern "system" fn(isize, isize) -> i32,
        param: isize,
    ) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, process: *mut u32) -> u32;
    fn IsHungAppWindow(hwnd: isize) -> i32;
    fn AttachThreadInput(attach: u32, attach_to: u32, on: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetLayeredWindowAttributes(
        hwnd: isize,
        key: *mut u32,
        alpha: *mut u8,
        flags: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

unsafe extern "system" fn set_affinity(hwnd: isize, affinity: isize) -> i32 {
    // SAFETY: a plain user32 call on a handle the enumeration just handed over. A window that
    // can't take it isn't on screen to begin with, so a failure doesn't matter.
    unsafe { SetWindowDisplayAffinity(hwnd, affinity as u32) };
    1
}

/// Takes this thread's windows out of what captures see, or puts them back. They stay on
/// screen and in place. GPUI opens all of them on this thread.
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

/// The full `HWND` from the low 32 bits `xcap` keeps as a window id: handles are 32-bit
/// values sign-extended to 64, so the top half is the bottom half's sign.
pub fn hwnd_from_id(id: u32) -> isize {
    id as i32 as isize
}

/// Whether the user can see and click the window `xcap` lists as `id`, so it is worth
/// picking. `xcap` lists every visible top-level window, including click-through ones
/// (`WS_EX_TRANSPARENT`): a layered, click-through, topmost window the size of the desktop,
/// which some apps keep for drawing over everything, would be the only thing under the
/// pointer on that screen. A layered window set fully transparent is invisible too. Layered
/// windows drawn per pixel aren't told apart: there's no cheap way, and they are rare on top
/// of real windows.
pub fn is_window_pickable(id: u32) -> bool {
    let hwnd = hwnd_from_id(id);
    // SAFETY: a plain user32 call on a handle; for a handle that has gone it answers 0.
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    if ex_style & WS_EX_TRANSPARENT != 0 {
        return false;
    }
    if ex_style & WS_EX_LAYERED != 0 {
        let (mut key, mut alpha, mut flags) = (0u32, 0u8, 0u32);
        // SAFETY: three writable out-parameters; the call fails, leaving them, for a window
        // that isn't layered with `SetLayeredWindowAttributes`.
        let known =
            unsafe { GetLayeredWindowAttributes(hwnd, &mut key, &mut alpha, &mut flags) } != 0;
        if known && flags & LWA_ALPHA != 0 && alpha == 0 {
            return false;
        }
    }
    true
}

/// The extended style GPUI gives a `WindowKind::PopUp`, which only the overlays use.
pub fn is_overlay_style(ex_style: u32) -> bool {
    let overlay = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    ex_style & overlay == overlay
}

/// A thread can't attach to itself, and thread 0 means nothing holds the foreground.
pub fn must_attach(foreground_thread: u32, this_thread: u32) -> bool {
    foreground_thread != 0 && foreground_thread != this_thread
}

unsafe extern "system" fn find_overlay(hwnd: isize, param: isize) -> i32 {
    // SAFETY: `param` is the handle `bring_overlay_to_foreground` passed and still holds.
    let found = unsafe { &mut *(param as *mut isize) };
    // SAFETY: plain user32 calls on a handle the enumeration just handed over.
    let overlay = unsafe {
        IsWindowVisible(hwnd) != 0 && is_overlay_style(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32)
    };
    if overlay {
        *found = hwnd;
    }
    i32::from(!overlay)
}

/// Gives an open overlay the foreground, and so the keyboard. Any one will do, since Esc on
/// any of them closes them all. After a delayed capture another app holds the foreground and
/// Windows refuses to hand it over to a background process, unless that process shares the
/// foreground thread's input for the moment it asks. `activate_window` gets there by pressing
/// Alt (`docs/spikes.md` §1.2), which would reach the app being captured.
///
/// Activation is delivered before this returns, and GPUI's handler for it needs the app, so
/// this must be called outside any `update`. A hung foreground app isn't attached to, because
/// that could hang Toru with it; the overlay then gets the keyboard on its first click.
pub fn bring_overlay_to_foreground() {
    let mut overlay = 0isize;
    // SAFETY: `overlay` outlives the enumeration, and the handles passed on are either the
    // one it found or the current foreground window. Nothing between the two
    // `AttachThreadInput` calls can leave early, so an attached thread is always detached.
    unsafe {
        let this_thread = GetCurrentThreadId();
        EnumThreadWindows(
            this_thread,
            find_overlay,
            &mut overlay as *mut isize as isize,
        );
        if overlay == 0 {
            return;
        }
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let attached = must_attach(foreground_thread, this_thread)
            && IsHungAppWindow(foreground) == 0
            && AttachThreadInput(this_thread, foreground_thread, 1) != 0;
        SetForegroundWindow(overlay);
        if attached {
            AttachThreadInput(this_thread, foreground_thread, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_id_sign_extends_back_to_its_handle() {
        assert_eq!(hwnd_from_id(0x0002_0ABC), 0x0002_0ABC);
        assert_eq!(hwnd_from_id(0xFFFF_0ABC), -0xF544);
    }

    #[test]
    fn a_handle_nobody_has_is_left_to_the_other_checks() {
        // Its style reads as 0, so nothing here rules it out, and the call neither fails nor
        // panics on a bad handle.
        assert!(is_window_pickable(0));
    }

    #[test]
    fn only_a_topmost_tool_window_counts_as_an_overlay() {
        const TOPMOST_TOOL: u32 = 0x88;
        const APP_WINDOW: u32 = 0x40000;
        assert!(is_overlay_style(TOPMOST_TOOL));
        assert!(is_overlay_style(TOPMOST_TOOL | APP_WINDOW));
        assert!(!is_overlay_style(0x8));
        assert!(!is_overlay_style(0x80));
        assert!(!is_overlay_style(APP_WINDOW));
    }

    #[test]
    fn input_is_only_shared_with_another_thread_that_holds_the_foreground() {
        assert!(must_attach(7, 3));
        assert!(!must_attach(3, 3));
        assert!(!must_attach(0, 3));
    }
}
