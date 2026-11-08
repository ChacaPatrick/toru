//! The little the app needs from the operating system that GPUI 0.3.11 doesn't offer: restoring
//! a maximized window, a display's size in physical pixels and the monitor's own identity, which
//! key and modifiers are down in a key event, the local time, the app-data and default save folders, showing
//! a file in the file manager, keeping Toru's own windows out of a
//! capture, bringing the capture overlay to the foreground, telling which windows a user can
//! pick, hiding the main window in the tray and showing it again, and telling a running Toru
//! that a second one was launched. Windows is done here; the other platforms get the fallbacks
//! at the bottom until they are.

use std::{path::PathBuf, time::Duration};

#[cfg(not(windows))]
use global_hotkey::hotkey::Code;
use gpui::PlatformDisplay;
#[cfg(not(windows))]
use toru_core::time::LocalDateTime;

#[cfg(windows)]
mod windows {
    use std::{
        ffi::OsString,
        fs,
        hash::{DefaultHasher, Hash, Hasher},
        io,
        os::windows::{ffi::OsStringExt, process::CommandExt},
        path::{Path, PathBuf},
        process::Command,
        thread,
    };

    use global_hotkey::hotkey::Code;
    use gpui::{App, PlatformDisplay, Window};
    use toru_core::time::{LocalDate, LocalDateTime};

    use super::HeldModifiers;
    use crate::main_window::WINDOW_TITLE;

    /// `GUID`.
    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    /// `FOLDERID_Pictures`.
    const PICTURES: Guid = Guid(
        0x33E2_8130,
        0x4E1E,
        0x4676,
        [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB],
    );

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    /// `MONITORINFOEXW`: `MONITORINFO` and the adapter's device name, `\\.\DISPLAY1`.
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
        device: [u16; 32],
    }

    impl MonitorInfo {
        fn new() -> MonitorInfo {
            let rect = || Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            MonitorInfo {
                size: size_of::<MonitorInfo>() as u32,
                monitor: rect(),
                work: rect(),
                flags: 0,
                device: [0; 32],
            }
        }
    }

    /// `DISPLAY_DEVICEW`.
    #[repr(C)]
    struct DisplayDevice {
        size: u32,
        name: [u16; 32],
        string: [u16; 128],
        state_flags: u32,
        id: [u16; 128],
        key: [u16; 128],
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

    const SW_HIDE: i32 = 0;
    const SW_NORMAL: i32 = 1;
    const SW_SHOW: i32 = 5;
    const SW_RESTORE: i32 = 9;
    const WM_CLOSE: u32 = 0x0010;
    const EVENT_MODIFY_STATE: u32 = 0x0002;
    const WAIT_OBJECT_0: u32 = 0;
    const INFINITE: u32 = u32::MAX;
    const ASFW_ANY: u32 = u32::MAX;
    /// GPUI's window class on Windows.
    const GPUI_WINDOW_CLASS: &str = "Zed::Window";
    const WS_EX_APPWINDOW: u32 = 0x0004_0000;
    const MONITOR_DEFAULTTONULL: u32 = 0;
    const WDA_NONE: u32 = 0;
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOPMOST: u32 = 0x8;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_EX_TRANSPARENT: u32 = 0x20;
    const WS_EX_LAYERED: u32 = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x2;
    const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x1;

    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    /// The first letter, digit and function key; the rest follow in order.
    const VK_A: i32 = 0x41;
    const VK_0: i32 = 0x30;
    const VK_F1: i32 = 0x70;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetKeyState(key: i32) -> i16;
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
        fn AllowSetForegroundWindow(process: u32) -> i32;
        fn PostMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> i32;
        fn GetClassNameW(hwnd: isize, name: *mut u16, max: i32) -> i32;
        fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
        fn EnumDisplayDevicesW(
            device: *const u16,
            index: u32,
            display_device: *mut DisplayDevice,
            flags: u32,
        ) -> i32;
        fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
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
        fn GetLocalTime(time: *mut SystemTime);
        fn CreateEventW(
            attributes: *const u8,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> isize;
        fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> isize;
        fn SetEvent(event: isize) -> i32;
        fn WaitForSingleObject(handle: isize, milliseconds: u32) -> u32;
        fn CloseHandle(handle: isize) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    /// `GetClassNameW`, as a `String`.
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

    /// The main window's handle: the one GPUI window of this thread's with an app window's
    /// style (GPUI opens all of them on this thread). A hidden or retitled window is found too.
    fn main_window() -> Option<isize> {
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

    /// Whether the key with the virtual-key code `key` is down in the key event being handled.
    /// `GetKeyState` follows the thread's messages, so inside a key-down handler it says which
    /// keys were down as that key went down, whatever the layout names them.
    pub(super) fn key_down(key: i32) -> bool {
        // SAFETY: a plain user32 call with a virtual-key code.
        (unsafe { GetKeyState(key) } as u16) & 0x8000 != 0
    }

    /// The virtual-key code of a key a shortcut can use, by the key's name: the letters, digits
    /// and function keys are numbered in order, and the few other keys are named one by one.
    /// `None` for a key a shortcut can't use.
    pub(super) fn virtual_key(key: Code) -> Option<i32> {
        let name = key.to_string();
        match name.as_bytes() {
            [b'K', b'e', b'y', letter @ b'A'..=b'Z'] => {
                return Some(VK_A + i32::from(letter - b'A'));
            }
            [b'D', b'i', b'g', b'i', b't', digit @ b'0'..=b'9'] => {
                return Some(VK_0 + i32::from(digit - b'0'));
            }
            _ => {}
        }
        if let Some(number) = name.strip_prefix('F')
            && let Ok(number @ 1..=24) = number.parse::<i32>()
        {
            return Some(VK_F1 + number - 1);
        }
        Some(match key {
            Code::PrintScreen => 0x2C,
            Code::Space => 0x20,
            Code::Insert => 0x2D,
            Code::Delete => 0x2E,
            Code::Home => 0x24,
            Code::End => 0x23,
            Code::PageUp => 0x21,
            Code::PageDown => 0x22,
            Code::ArrowUp => 0x26,
            Code::ArrowDown => 0x28,
            Code::ArrowLeft => 0x25,
            Code::ArrowRight => 0x27,
            _ => return None,
        })
    }

    /// The first of `keys` that `key_down` finds down.
    pub(super) fn pressed_among(
        keys: impl Iterator<Item = Code>,
        key_down: impl Fn(i32) -> bool,
    ) -> Option<Code> {
        let mut keys = keys;
        keys.find(|key| virtual_key(*key).is_some_and(&key_down))
    }

    /// Which of `keys` is down in the key event being handled (`key_down`), or `None` if none
    /// of them is.
    pub fn pressed_key(keys: impl Iterator<Item = Code>) -> Option<Code> {
        pressed_among(keys, key_down)
    }

    /// Whether the main window is hidden in the tray (`hide_main_window`), as against shown or
    /// minimized.
    pub fn main_window_hidden() -> bool {
        // SAFETY: a plain user32 call on a window of this thread's.
        main_window().is_some_and(|hwnd| unsafe { IsWindowVisible(hwnd) } == 0)
    }

    /// The modifiers down in the key event being handled (`key_down`).
    pub fn held_modifiers() -> HeldModifiers {
        HeldModifiers {
            control: key_down(VK_CONTROL),
            alt: key_down(VK_MENU),
            shift: key_down(VK_SHIFT),
            win: key_down(VK_LWIN) || key_down(VK_RWIN),
        }
    }

    /// Takes the main window off the screen, the taskbar and Alt+Tab; the app keeps running in
    /// the tray. Posted rather than sent, since this runs inside the window's own close handling.
    pub fn hide_main_window() {
        if let Some(hwnd) = main_window() {
            // SAFETY: a plain user32 call on a window of this thread's.
            unsafe { ShowWindowAsync(hwnd, SW_HIDE) };
        }
    }

    /// Shows the main window, restoring it if minimized, and brings it forward. Windows only
    /// lets the foreground move when the user asked for it, as a tray click or a second launch
    /// did; otherwise the taskbar button flashes instead, which is Windows' own answer.
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

    /// Asks the main window to close the way its caption's X does, so the close goes through
    /// `Window::on_window_should_close`. GPUI's `remove_window` would skip that.
    pub fn request_close(_window: &mut Window) {
        if let Some(hwnd) = main_window() {
            // SAFETY: a plain user32 call on a window of this thread's.
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
        }
    }

    /// The named event a second launch signals, one per data folder so two Torus on different
    /// folders (the owner's and a test's) don't reach each other.
    fn show_event_name(data_dir: &Path) -> Vec<u16> {
        let key = fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
        let mut hasher = DefaultHasher::new();
        key.as_os_str().hash(&mut hasher);
        wide(&format!("Local\\Toru.Show.{:016x}", hasher.finish()))
    }

    /// Creates the event a second launch signals, and runs `on_show` on a background thread
    /// each time it is signalled, for the life of the process.
    pub fn watch_for_show(data_dir: &Path, on_show: impl Fn() + Send + 'static) {
        let name = show_event_name(data_dir);
        // SAFETY: the name is NUL-terminated. An auto-reset event, so each signal is one show.
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        if event == 0 {
            return;
        }
        thread::spawn(move || {
            // SAFETY: `event` is a handle this process owns and never closes.
            while unsafe { WaitForSingleObject(event, INFINITE) } == WAIT_OBJECT_0 {
                on_show();
            }
        });
    }

    /// Tells the Toru already running on `data_dir` to show its window, and lets it take the
    /// foreground, since this process was just launched by the user. `false` if none is
    /// listening.
    pub fn signal_running_instance(data_dir: &Path) -> bool {
        let name = show_event_name(data_dir);
        // SAFETY: the name is NUL-terminated, and the handle is closed once used.
        unsafe {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
            if event == 0 {
                return false;
            }
            AllowSetForegroundWindow(ASFW_ANY);
            let signalled = SetEvent(event) != 0;
            CloseHandle(event);
            signalled
        }
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetKnownFolderPath(
            id: *const Guid,
            flags: u32,
            token: isize,
            path: *mut *mut u16,
        ) -> i32;
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoTaskMemFree(memory: *mut std::ffi::c_void);
    }

    /// Where the Pictures folder really is, which may have been moved or redirected to OneDrive.
    fn pictures_folder() -> Option<PathBuf> {
        let mut text = std::ptr::null_mut();
        // SAFETY: `PICTURES` and `text` outlive the call.
        let result = unsafe { SHGetKnownFolderPath(&PICTURES, 0, 0, &mut text) };
        // SAFETY: on success `text` is a NUL-terminated string, and it is read before being freed.
        let folder = (result == 0).then(|| unsafe {
            let length = (0..).take_while(|&i| *text.add(i) != 0).count();
            PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(
                text, length,
            )))
        });
        // SAFETY: Windows allocated `text` for the caller to free, even on failure.
        unsafe { CoTaskMemFree(text.cast()) };
        folder
    }

    pub fn default_save_folder() -> Option<PathBuf> {
        let pictures = pictures_folder().or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Pictures"))
        })?;
        Some(pictures.join("Screenshots"))
    }

    /// Explorer's own syntax for opening a folder with a file selected.
    pub(super) fn select_argument(path: &Path) -> String {
        format!("/select,\"{}\"", path.display())
    }

    /// Opens Explorer on the file's folder with the file selected, or just on the folder if the
    /// file has gone, since `/select` on a missing file opens Documents instead.
    pub fn reveal_file(path: &Path) -> io::Result<()> {
        let mut explorer = Command::new("explorer");
        if path.is_file() {
            explorer.raw_arg(select_argument(path));
        } else {
            let folder = path.parent().filter(|folder| folder.is_dir());
            explorer.arg(folder.ok_or(io::ErrorKind::NotFound)?);
        }
        explorer.spawn().map(drop)
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
        let info = monitor_info(display)?;
        let width = u32::try_from(info.monitor.right - info.monitor.left).ok()?;
        let height = u32::try_from(info.monitor.bottom - info.monitor.top).ok()?;
        Some((width, height))
    }

    fn monitor_info(display: &dyn PlatformDisplay) -> Option<MonitorInfo> {
        let mut info = MonitorInfo::new();
        let monitor = u64::from(display.id()) as isize;
        // SAFETY: `info` is a writable MONITORINFOEXW with its size filled in.
        (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info)
    }

    /// The text up to the first NUL.
    pub(super) fn until_nul(text: &[u16]) -> String {
        let length = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16_lossy(&text[..length])
    }

    /// The monitor's device interface path, as `EnumDisplayDevices` gives it with
    /// `EDD_GET_DEVICE_INTERFACE_NAME`: `\\?\DISPLAY#GSM5B09#5&2d9b2c1f&0&UID4352#{...}`. It
    /// names the monitor's make and model and the port it is on, so it is the same after a
    /// restart or an unplugging, and a different monitor on the same port gets its own; the
    /// same monitor moved to another port counts as new. GPUI's `uuid` hashes only the adapter
    /// name, `\\.\DISPLAY2`, which whichever monitor is plugged in there inherits. `None` for a
    /// monitor Windows can't name, as a remote desktop's.
    pub fn display_key(display: &dyn PlatformDisplay) -> Option<String> {
        let info = monitor_info(display)?;
        let mut device = DisplayDevice {
            size: size_of::<DisplayDevice>() as u32,
            name: [0; 32],
            string: [0; 128],
            state_flags: 0,
            id: [0; 128],
            key: [0; 128],
        };
        // SAFETY: the adapter name is NUL-terminated within its 32 characters, and `device` is
        // a writable DISPLAY_DEVICEW with its size filled in.
        let found = unsafe {
            EnumDisplayDevicesW(
                info.device.as_ptr(),
                0,
                &mut device,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
        } != 0;
        let key = until_nul(&device.id);
        (found && !key.is_empty()).then_some(key)
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
        let (text, caption) = (wide(message), wide(WINDOW_TITLE));
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR) };
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
}

/// Between the lines of text put on the clipboard.
pub const LINE_BREAK: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// How long a capture waits after `exclude_windows_from_capture(true)`. Exclusion took effect with
/// no wait in every trial (`docs/spikes.md` §5.1); this is a margin of a few frames that nobody
/// will notice.
pub const EXCLUDE_SETTLE: Duration = Duration::from_millis(50);

#[cfg(windows)]
pub use windows::{
    bring_overlay_to_foreground, default_save_folder, exclude_windows_from_capture, held_modifiers,
    hide_main_window, is_window_pickable, local_now, main_window_hidden, pressed_key,
    request_close, restore_window, reveal_file, show_main_window, show_startup_error,
    signal_running_instance, watch_for_show,
};

/// The modifiers held along with a key (`held_modifiers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// Where the platform can't say which key is down, the recorder falls back to the names GPUI
/// gives keys.
#[cfg(not(windows))]
pub fn pressed_key(_keys: impl Iterator<Item = Code>) -> Option<Code> {
    None
}

#[cfg(not(windows))]
pub fn held_modifiers() -> HeldModifiers {
    HeldModifiers {
        control: false,
        alt: false,
        shift: false,
        win: false,
    }
}

#[cfg(not(windows))]
pub fn restore_window(_window: &gpui::Window, _cx: &gpui::App) {}

#[cfg(not(windows))]
pub fn default_save_folder() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Pictures").join("Screenshots"))
}

#[cfg(not(windows))]
pub fn reveal_file(_path: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(windows))]
pub fn bring_overlay_to_foreground() {}

#[cfg(not(windows))]
pub fn exclude_windows_from_capture(_exclude: bool) {}

#[cfg(not(windows))]
pub fn is_window_pickable(_id: u32) -> bool {
    true
}

#[cfg(not(windows))]
pub fn hide_main_window() {}

#[cfg(not(windows))]
pub fn show_main_window() {}

#[cfg(not(windows))]
pub fn main_window_hidden() -> bool {
    false
}

#[cfg(not(windows))]
pub fn request_close(window: &mut gpui::Window) {
    window.remove_window();
}

#[cfg(not(windows))]
pub fn watch_for_show(_data_dir: &std::path::Path, _on_show: impl Fn() + Send + 'static) {}

#[cfg(not(windows))]
pub fn signal_running_instance(_data_dir: &std::path::Path) -> bool {
    false
}

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

/// What a per-display shortcut is stored under: the monitor's own identity where the platform
/// gives one (`windows::display_key`), else GPUI's id for the display's place among the
/// adapters, which whichever monitor is plugged in there inherits.
pub fn display_key(display: &dyn PlatformDisplay) -> String {
    #[cfg(windows)]
    if let Some(key) = windows::display_key(display) {
        return key;
    }
    display
        .uuid()
        .map(|uuid| uuid.to_string())
        .unwrap_or_default()
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
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("toru-platform-{}-{name}", std::process::id()))
    }

    #[test]
    fn a_signal_with_nobody_listening_is_reported() {
        assert!(!signal_running_instance(&unique_dir("nobody")));
    }

    #[test]
    fn a_signal_reaches_the_watcher_for_the_same_folder_only() {
        let dir = unique_dir("watched");
        let (sender, receiver) = mpsc::channel();
        watch_for_show(&dir, move || {
            sender.send(()).ok();
        });
        assert!(!signal_running_instance(&unique_dir("other")));
        assert!(signal_running_instance(&dir));
        assert!(receiver.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn the_local_time_is_a_real_date() {
        let now = local_now();
        assert!(now.date.year >= 2026);
        assert!((1..=12).contains(&now.date.month));
        assert!((1..=31).contains(&now.date.day));
        assert!(now.hour < 24 && now.minute < 60);
    }

    #[test]
    fn the_default_save_folder_is_screenshots_under_an_absolute_pictures_folder() {
        let folder = default_save_folder().unwrap();
        assert!(folder.is_absolute(), "{folder:?}");
        assert!(folder.ends_with("Screenshots"), "{folder:?}");
    }

    #[test]
    fn explorer_selects_a_path_with_spaces_and_commas() {
        assert_eq!(
            windows::select_argument(Path::new(r"C:\My, Pictures\a shot.png")),
            r#"/select,"C:\My, Pictures\a shot.png""#
        );
    }

    #[test]
    fn only_a_topmost_tool_window_counts_as_an_overlay() {
        const TOPMOST_TOOL: u32 = 0x88;
        const APP_WINDOW: u32 = 0x40000;
        assert!(windows::is_overlay_style(TOPMOST_TOOL));
        assert!(windows::is_overlay_style(TOPMOST_TOOL | APP_WINDOW));
        assert!(!windows::is_overlay_style(0x8));
        assert!(!windows::is_overlay_style(0x80));
        assert!(!windows::is_overlay_style(APP_WINDOW));
    }

    #[test]
    fn only_an_app_window_counts_as_the_main_window() {
        const TOPMOST_TOOL: u32 = 0x88;
        const APP_WINDOW: u32 = 0x40000;
        const NO_REDIRECTION_BITMAP: u32 = 0x0020_0000;
        assert!(windows::is_main_window_style(APP_WINDOW));
        assert!(windows::is_main_window_style(
            APP_WINDOW | NO_REDIRECTION_BITMAP
        ));
        assert!(!windows::is_main_window_style(TOPMOST_TOOL));
        assert!(!windows::is_main_window_style(0x80));
        assert!(!windows::is_main_window_style(0));
    }

    #[test]
    fn input_is_only_shared_with_another_thread_that_holds_the_foreground() {
        assert!(windows::must_attach(7, 3));
        assert!(!windows::must_attach(3, 3));
        assert!(!windows::must_attach(0, 3));
    }

    #[test]
    fn a_window_id_sign_extends_back_to_its_handle() {
        assert_eq!(windows::hwnd_from_id(0x0002_0ABC), 0x0002_0ABC);
        assert_eq!(windows::hwnd_from_id(0xFFFF_0ABC), -0xF544);
    }

    #[test]
    fn text_is_read_up_to_its_nul() {
        let text: Vec<u16> = "abc\0def".encode_utf16().collect();
        assert_eq!(windows::until_nul(&text), "abc");
        assert_eq!(windows::until_nul(&[]), "");
        assert_eq!(windows::until_nul(&[0]), "");
    }

    #[test]
    fn every_shortcut_key_has_its_own_virtual_key_code() {
        use global_hotkey::hotkey::Code;

        assert_eq!(windows::virtual_key(Code::KeyA), Some(0x41));
        assert_eq!(windows::virtual_key(Code::KeyZ), Some(0x5A));
        assert_eq!(windows::virtual_key(Code::Digit0), Some(0x30));
        assert_eq!(windows::virtual_key(Code::Digit9), Some(0x39));
        assert_eq!(windows::virtual_key(Code::F1), Some(0x70));
        assert_eq!(windows::virtual_key(Code::F24), Some(0x87));
        assert_eq!(windows::virtual_key(Code::PrintScreen), Some(0x2C));
        assert_eq!(windows::virtual_key(Code::PageDown), Some(0x22));
        for other in [
            Code::Escape,
            Code::Tab,
            Code::Enter,
            Code::Fn,
            Code::F25,
            Code::Comma,
        ] {
            assert_eq!(windows::virtual_key(other), None, "{other}");
        }
        let codes: Vec<i32> = crate::shortcuts::keys()
            .map(|key| windows::virtual_key(key).expect("every shortcut key has a code"))
            .collect();
        let distinct: std::collections::HashSet<i32> = codes.iter().copied().collect();
        assert_eq!(distinct.len(), codes.len());
    }

    #[test]
    fn the_key_down_is_found_by_its_virtual_key_code_whatever_it_types() {
        use global_hotkey::hotkey::Code;

        let keys = crate::shortcuts::keys;
        // Ctrl+Shift+1: GPUI would say "!" and nothing else is down.
        assert_eq!(
            windows::pressed_among(keys(), |vk| vk == 0x31),
            Some(Code::Digit1)
        );
        assert_eq!(
            windows::pressed_among(keys(), |vk| vk == 0x2C),
            Some(Code::PrintScreen)
        );
        // Esc, Tab and Enter aren't keys a shortcut can use.
        for other in [0x1B, 0x09, 0x0D] {
            assert_eq!(windows::pressed_among(keys(), |vk| vk == other), None);
        }
        assert_eq!(windows::pressed_among(keys(), |_| false), None);
    }

    #[test]
    fn nothing_is_down_in_a_test_with_no_key_event() {
        // `GetKeyState` follows this thread's messages, and a test thread gets none.
        assert!(!windows::key_down(0x41));
        assert_eq!(pressed_key(crate::shortcuts::keys()), None);
        assert_eq!(
            held_modifiers(),
            HeldModifiers {
                control: false,
                alt: false,
                shift: false,
                win: false,
            }
        );
    }

    #[test]
    fn a_handle_nobody_has_is_left_to_the_other_checks() {
        // Its style reads as 0, so nothing here rules it out, and the call neither fails nor
        // panics on a bad handle.
        assert!(is_window_pickable(0));
    }
}
