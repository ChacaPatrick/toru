use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::thread;

use super::utf16::wide;

const EVENT_MODIFY_STATE: u32 = 0x0002;
const WAIT_OBJECT_0: u32 = 0;
const INFINITE: u32 = u32::MAX;
const ASFW_ANY: u32 = u32::MAX;

#[link(name = "user32")]
unsafe extern "system" {
    fn AllowSetForegroundWindow(process: u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const u8,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> isize;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> isize;
    fn CreateMutexW(attributes: *const u8, initial_owner: i32, name: *const u16) -> isize;
    fn SetEvent(event: isize) -> i32;
    fn WaitForSingleObject(handle: isize, milliseconds: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
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

/// Holds a named mutex for the life of the process, which the installer and uninstaller
/// look for (`AppMutex` in `installer/toru.iss`) so they ask for Toru to be quit first.
pub fn mark_running() {
    let name = wide("Toru.Running");
    // SAFETY: the name is NUL-terminated. The handle is never closed, so the mutex lasts as
    // long as the process.
    unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;
    use std::time::Duration;

    use crate::platform::windows::unique_dir;

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
}
