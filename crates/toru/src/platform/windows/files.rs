use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

/// `FOLDERID_Pictures`.
const PICTURES: Guid = Guid(
    0x33E2_8130,
    0x4E1E,
    0x4676,
    [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB],
);

/// `SHFILEOPSTRUCTW`, as laid out on 64-bit Windows.
#[repr(C)]
struct FileOperation {
    hwnd: isize,
    func: u32,
    /// Paths, each NUL-terminated, with one more NUL after the last.
    from: *const u16,
    to: *const u16,
    flags: u16,
    any_aborted: i32,
    name_mappings: *mut std::ffi::c_void,
    progress_title: *const u16,
}

const FO_DELETE: u32 = 3;
const FOF_SILENT: u16 = 0x0004;
const FOF_NOCONFIRMATION: u16 = 0x0010;
const FOF_ALLOWUNDO: u16 = 0x0040;
const FOF_NOERRORUI: u16 = 0x0400;

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: isize, path: *mut *mut u16) -> i32;
    fn SHFileOperationW(operation: *mut FileOperation) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(memory: *mut std::ffi::c_void);
}

/// It may have been moved or redirected to OneDrive.
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

/// Asks nothing and shows nothing. A missing file is `NotFound`, and anything but a file is
/// refused, since the shell would take a whole folder.
/// On a drive with no Recycle Bin, a network share say, the shell deletes the file for good.
pub fn recycle_file(path: &Path) -> io::Result<()> {
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::other("not a file"));
    }
    // The shell wants a full path, and two NULs after the last one.
    let mut from: Vec<u16> = std::path::absolute(path)?
        .as_os_str()
        .encode_wide()
        .collect();
    from.extend([0, 0]);
    let mut operation = FileOperation {
        hwnd: 0,
        func: FO_DELETE,
        from: from.as_ptr(),
        to: std::ptr::null(),
        flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
        any_aborted: 0,
        name_mappings: std::ptr::null_mut(),
        progress_title: std::ptr::null(),
    };
    // SAFETY: `operation` and the path list it points to outlive the call.
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result != 0 || operation.any_aborted != 0 {
        return Err(io::Error::other(format!(
            "SHFileOperationW failed with {result:#x}"
        )));
    }
    Ok(())
}

pub(super) fn select_argument(path: &Path) -> String {
    format!("/select,\"{}\"", path.display())
}

/// Just the folder if the file has gone, since `/select` on a missing file opens Documents
/// instead.
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::platform::windows::unique_dir;

    #[test]
    fn recycling_refuses_a_folder_and_reports_a_missing_file() {
        let dir = unique_dir("recycle-refuses");
        std::fs::create_dir_all(&dir).unwrap();

        assert!(recycle_file(&dir).is_err());
        assert!(dir.is_dir());
        let missing = recycle_file(&dir.join("missing.png")).unwrap_err();
        assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);

        std::fs::remove_dir(&dir).unwrap();
    }

    /// Ignored, as it leaves a file in the Recycle Bin: `cargo test -- --ignored recycling`.
    #[test]
    #[ignore]
    fn recycling_moves_a_file_out_of_its_folder() {
        let dir = unique_dir("recycle-moves");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("toru-recycle-test.png");
        std::fs::write(&file, b"test").unwrap();

        recycle_file(&file).unwrap();

        assert!(!file.exists());
        std::fs::remove_dir(&dir).unwrap();
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
            select_argument(Path::new(r"C:\My, Pictures\a shot.png")),
            r#"/select,"C:\My, Pictures\a shot.png""#
        );
    }
}
