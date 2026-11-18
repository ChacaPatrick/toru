use std::io;
use std::path::{Path, PathBuf};

use super::registry::{
    RRF_RT_REG_BINARY, delete_user_value, read_user_data, read_user_value, write_user_value,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Windows' Startup apps page (Settings and Task Manager) marks a `RUN_KEY` value of
/// the same name enabled or disabled, leaving the value itself in place.
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Toru's value under `RUN_KEY` and `APPROVED_KEY`. The installer's "Start Toru when Windows
/// starts" writes the first, and its uninstaller removes both (`installer/toru.iss`).
const RUN_VALUE: &str = "Toru";

pub(super) fn run_command(exe: &Path) -> String {
    format!("\"{}\" --tray", exe.display())
}

/// Only when it is quoted as `run_command` writes it.
pub(super) fn run_command_exe(command: &str) -> Option<PathBuf> {
    let (exe, _) = command.strip_prefix('"')?.split_once('"')?;
    Some(PathBuf::from(exe))
}

/// The first byte is 2 (or 6) while enabled and 3 (or 7) while disabled, followed by when.
pub(super) fn approved_says_disabled(data: &[u8]) -> bool {
    data.first().is_some_and(|flags| flags & 1 != 0)
}

pub(super) fn starts_at_sign_in(run_key: &str, approved_key: &str) -> bool {
    read_user_value(run_key, RUN_VALUE).is_some()
        && !read_user_data(approved_key, RUN_VALUE, RRF_RT_REG_BINARY)
            .is_some_and(|data| approved_says_disabled(&data))
}

/// Either way the Startup apps page's mark goes, so turning on isn't still blocked by an earlier
/// "disabled" there.
pub(super) fn set_start_at_sign_in(
    run_key: &str,
    approved_key: &str,
    on: bool,
    exe: &Path,
) -> io::Result<()> {
    if on {
        write_user_value(run_key, RUN_VALUE, &run_command(exe))?;
    } else {
        delete_user_value(run_key, RUN_VALUE)?;
    }
    delete_user_value(approved_key, RUN_VALUE)
}

/// Only when the exe the Run value names is gone, as after Toru was moved or reinstalled
/// elsewhere. One naming another copy that still exists, such as a development build, is left
/// to the user's switch.
pub(super) fn repoint_start_at_sign_in(run_key: &str, exe: &Path) {
    let stale = read_user_value(run_key, RUN_VALUE)
        .and_then(|command| run_command_exe(&command))
        .is_some_and(|named| named != exe && !named.exists());
    if stale {
        write_user_value(run_key, RUN_VALUE, &run_command(exe)).ok();
    }
}

/// Whichever copy of Toru the Run value names.
pub fn starts_with_windows() -> bool {
    starts_at_sign_in(RUN_KEY, APPROVED_KEY)
}

pub fn set_starts_with_windows(on: bool) -> io::Result<()> {
    set_start_at_sign_in(RUN_KEY, APPROVED_KEY, on, &std::env::current_exe()?)
}

pub fn repoint_start_with_windows() {
    if let Ok(exe) = std::env::current_exe() {
        repoint_start_at_sign_in(RUN_KEY, &exe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::platform::windows::registry::{REG_BINARY, delete_user_key, write_user_data};
    use crate::platform::windows::unique_dir;

    #[test]
    fn the_run_command_starts_this_exe_in_the_tray() {
        assert_eq!(
            run_command(Path::new(r"C:\Users\A B\Toru\toru.exe")),
            r#""C:\Users\A B\Toru\toru.exe" --tray"#
        );
    }

    #[test]
    fn the_startup_apps_mark_reads_disabled_on_its_odd_flags() {
        // As Windows writes them: the flag, then for a disabled entry when it was turned off.
        let enabled = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let disabled = [3, 0, 0, 0, 0xDD, 0x5D, 0x67, 0xCC, 0xA3, 0x9E, 0xDC, 0x01];
        assert!(!approved_says_disabled(&enabled));
        assert!(approved_says_disabled(&disabled));
        assert!(!approved_says_disabled(&[6]));
        assert!(approved_says_disabled(&[7]));
        assert!(!approved_says_disabled(&[]));
    }

    #[test]
    fn the_exe_is_read_back_from_a_quoted_run_command() {
        let exe = Path::new(r"C:\Users\A B\Toru\toru.exe");
        assert_eq!(run_command_exe(&run_command(exe)).as_deref(), Some(exe));
        assert_eq!(run_command_exe(r"C:\toru.exe --tray"), None);
    }

    /// On keys of the test's own, never the real Run or StartupApproved keys.
    #[test]
    fn start_at_sign_in_follows_the_startup_apps_mark_and_clears_it() {
        let run = format!(r"Software\Toru-tests-{}-run", std::process::id());
        let approved = format!(r"Software\Toru-tests-{}-approved", std::process::id());
        let exe = std::env::current_exe().unwrap();
        let mark =
            |flag: u8| write_user_data(&approved, "Toru", REG_BINARY, &[flag, 0, 0, 0]).unwrap();

        assert!(!starts_at_sign_in(&run, &approved));
        set_start_at_sign_in(&run, &approved, true, &exe).unwrap();
        assert!(starts_at_sign_in(&run, &approved));
        mark(2);
        assert!(starts_at_sign_in(&run, &approved));
        // Turned off on the Startup apps page: the Run value stays, but Toru won't start.
        mark(3);
        assert!(read_user_value(&run, "Toru").is_some());
        assert!(!starts_at_sign_in(&run, &approved));
        // Turning the switch on again clears that mark.
        set_start_at_sign_in(&run, &approved, true, &exe).unwrap();
        assert!(starts_at_sign_in(&run, &approved));
        assert_eq!(read_user_value(&approved, "Toru"), None);
        mark(3);
        set_start_at_sign_in(&run, &approved, false, &exe).unwrap();
        assert!(!starts_at_sign_in(&run, &approved));
        assert_eq!(read_user_value(&run, "Toru"), None);

        delete_user_key(&run);
        delete_user_key(&approved);
    }

    /// On a key of the test's own, never the real Run key.
    #[test]
    fn a_run_value_naming_a_missing_exe_is_repointed_and_another_copy_is_not() {
        let run = format!(r"Software\Toru-tests-{}-repoint", std::process::id());
        let exe = std::env::current_exe().unwrap();
        let read = || read_user_value(&run, "Toru");

        repoint_start_at_sign_in(&run, &exe);
        assert_eq!(read(), None);
        // Another copy that exists, here the test's own folder standing in for one.
        let other = run_command(&std::env::temp_dir());
        write_user_value(&run, "Toru", &other).unwrap();
        repoint_start_at_sign_in(&run, &exe);
        assert_eq!(read(), Some(other));
        let gone = unique_dir("gone").join("toru.exe");
        write_user_value(&run, "Toru", &run_command(&gone)).unwrap();
        repoint_start_at_sign_in(&run, &exe);
        assert_eq!(read(), Some(run_command(&exe)));

        delete_user_value(&run, "Toru").unwrap();
        delete_user_key(&run);
    }
}
