//! The little the app needs from the operating system that GPUI 0.3.11 doesn't offer: restoring
//! a maximized window, a display's size in physical pixels and the monitor's own identity, which
//! key and modifiers are down in a key event, the local time, the app-data and default save folders, showing
//! a file in the file manager, sending a file to the Recycle Bin, keeping Toru's own windows out of a
//! capture, the system fonts that draw what the bundled ones lack (mapped, not read) and the user's
//! languages that order them, bringing the capture overlay
//! to the foreground, telling which windows a user can
//! pick, hiding the main window in the tray and showing it again, telling a running Toru
//! that a second one was launched, saying why Windows' shutdown waits, starting Toru when the
//! user signs in, whether the user has animations turned off, handing back memory while
//! hidden, the tray's icon out of the exe, and, for updates, fetching a URL and starting the
//! installer. Windows is done in `windows/`; the other platforms get the stand-ins in
//! `unported.rs` until they are.

use std::{path::PathBuf, time::Duration};

use gpui::PlatformDisplay;

#[cfg(not(windows))]
mod unported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unported::*;
#[cfg(windows)]
pub use windows::{
    accessibility::reduce_motion,
    capture::{bring_overlay_to_foreground, exclude_windows_from_capture, is_window_pickable},
    files::{default_save_folder, recycle_file, reveal_file},
    fonts::{fallback_fonts, map_file, user_languages},
    keyboard::{held_modifiers, pressed_key},
    local_time::local_now,
    main_window::{
        hide_main_window, main_window_hidden, request_close, restore_window, show_main_window,
    },
    memory::trim_working_set,
    running_instance::{mark_running, signal_running_instance, watch_for_show},
    shutdown::while_holding_up_shutdown,
    start_at_sign_in::{repoint_start_with_windows, set_starts_with_windows, starts_with_windows},
    startup_error::show_startup_error,
    tray_icon::tray_icon,
    update::{download, fetch_text, start_installer},
};

/// Between the lines of text put on the clipboard.
pub const LINE_BREAK: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// How long a capture waits after `exclude_windows_from_capture(true)`. Exclusion took effect with
/// no wait in every trial (`docs/spikes.md` §5.1); this is a margin of a few frames that nobody
/// will notice.
pub const EXCLUDE_SETTLE: Duration = Duration::from_millis(50);

/// What an update's installer is started with, as the release workflow's installer expects
/// (`docs/PLAN.md` 11.2): install without a word, and `/UPDATE=1` to wait for Toru to exit first
/// and start it again after.
pub const INSTALLER_ARGS: [&str; 4] = [
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/UPDATE=1",
];

/// The modifiers held along with a key (`held_modifiers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
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
/// gives one (`windows::display::display_key`), else GPUI's id for the display's place among the
/// adapters, which whichever monitor is plugged in there inherits.
pub fn display_key(display: &dyn PlatformDisplay) -> String {
    #[cfg(windows)]
    if let Some(key) = windows::display::display_key(display) {
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
    if let Some(pixels) = windows::display::display_pixels(display) {
        return pixels;
    }
    let size = display.bounds().size;
    (
        f32::from(size.width).round() as u32,
        f32::from(size.height).round() as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installer_is_started_as_the_release_workflow_expects() {
        assert_eq!(
            INSTALLER_ARGS.join(" "),
            "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /UPDATE=1"
        );
    }
}
