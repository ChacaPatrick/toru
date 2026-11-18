use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::utf16::until_nul;
use crate::editor::text::Cjk;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: isize) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const u8,
        disposition: u32,
        flags: u32,
        template: isize,
    ) -> isize;
    fn GetFileSizeEx(file: isize, size: *mut i64) -> i32;
    fn CreateFileMappingW(
        file: isize,
        security: *const u8,
        protect: u32,
        size_high: u32,
        size_low: u32,
        name: *const u16,
    ) -> isize;
    fn MapViewOfFile(
        mapping: isize,
        access: u32,
        offset_high: u32,
        offset_low: u32,
        bytes: usize,
    ) -> *const u8;
    fn GetUserDefaultUILanguage() -> u16;
    fn LCIDToLocaleName(locale: u32, name: *mut u16, length: i32, flags: u32) -> i32;
    fn GetUserDefaultLocaleName(name: *mut u16, length: i32) -> i32;
}

/// The system fonts that draw what the editor's bundled fonts lack, in the order they are
/// tried, each with the CJK language it is for: Segoe UI for Latin, Greek and Cyrillic, its
/// symbols and emoji (the emoji's plain outlines), then Chinese, Japanese and Korean, which
/// `editor::text` puts in the user's order, then the Indic scripts. A file this Windows
/// doesn't have, such as Meiryo where the Japanese fonts aren't installed, is skipped when it
/// is read.
pub fn fallback_fonts() -> Vec<(PathBuf, Option<Cjk>)> {
    const FILES: [(&str, Option<Cjk>); 9] = [
        ("segoeui.ttf", None),
        ("seguisym.ttf", None),
        ("seguiemj.ttf", None),
        ("msyh.ttc", Some(Cjk::Simplified)),
        ("msjh.ttc", Some(Cjk::Traditional)),
        ("YuGothM.ttc", Some(Cjk::Japanese)),
        ("meiryo.ttc", Some(Cjk::Japanese)),
        ("malgun.ttf", Some(Cjk::Korean)),
        ("Nirmala.ttc", None),
    ];
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    let fonts = PathBuf::from(windows).join("Fonts");
    FILES
        .iter()
        .map(|&(file, cjk)| (fonts.join(file), cjk))
        .collect()
}

/// The file's bytes, mapped read-only for the app's life rather than read: only the pages
/// that are looked at are loaded, and they are the system's cached copy of the file, which
/// it can drop and read again. While mapped, the file can't be shortened, and deleting or
/// replacing it (as Windows Update does) leaves the mapped bytes as they were. An
/// administrator could still write into it in place; that's accepted, since the font
/// parser checks every read against the length, so the worst is wrong glyphs.
/// `None` for a missing or empty file.
pub fn map_file(path: &Path) -> Option<&'static [u8]> {
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const OPEN_EXISTING: u32 = 3;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
    const INVALID_HANDLE_VALUE: isize = -1;
    const PAGE_READONLY: u32 = 0x2;
    const FILE_MAP_READ: u32 = 0x4;
    let name: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `name` is NUL-terminated; the other arguments are plain values.
    let file = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            0,
        )
    };
    if file == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut size = 0i64;
    // SAFETY: `file` is the handle just opened and `size` is writable. Windows can't map an
    // empty file, so none is tried.
    let mapping = unsafe {
        if GetFileSizeEx(file, &mut size) != 0 && size > 0 {
            CreateFileMappingW(
                file,
                std::ptr::null(),
                PAGE_READONLY,
                0,
                0,
                std::ptr::null(),
            )
        } else {
            0
        }
    };
    // SAFETY: the view, once made, keeps the mapping and the file open after their handles
    // close, and is never unmapped, so the slice stays valid at this length for the app's
    // life (see above for writes in place).
    unsafe {
        let view = if mapping != 0 {
            MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0)
        } else {
            std::ptr::null()
        };
        if mapping != 0 {
            CloseHandle(mapping);
        }
        CloseHandle(file);
        (!view.is_null()).then(|| std::slice::from_raw_parts(view, size as usize))
    }
}

/// The user's Windows display language, then the locale their formats follow, as names
/// like `ja-JP`; either is left out if Windows can't name it.
pub fn user_languages() -> Vec<String> {
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    let mut ui = [0u16; LOCALE_NAME_MAX_LENGTH];
    let mut region = [0u16; LOCALE_NAME_MAX_LENGTH];
    // SAFETY: both buffers are writable and their lengths are passed along. A language id is
    // the locale id with the default sort order, which is 0.
    let lengths = unsafe {
        [
            LCIDToLocaleName(
                u32::from(GetUserDefaultUILanguage()),
                ui.as_mut_ptr(),
                ui.len() as i32,
                0,
            ),
            GetUserDefaultLocaleName(region.as_mut_ptr(), region.len() as i32),
        ]
    };
    [ui, region]
        .iter()
        .zip(lengths)
        .filter(|(_, length)| *length > 0)
        .map(|(name, _)| until_nul(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::platform::windows::unique_dir;

    /// A system font stands in for a mapped file, since the mapping lasts as long as the tests.
    #[test]
    fn a_mapped_file_holds_its_bytes_and_a_missing_or_empty_one_none() {
        let (font, _) = fallback_fonts().swap_remove(0);
        assert_eq!(map_file(&font).unwrap(), std::fs::read(&font).unwrap());

        let dir = unique_dir("map-file");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(map_file(&dir.join("missing.ttf")).is_none());
        let empty = dir.join("empty.ttf");
        std::fs::write(&empty, b"").unwrap();
        assert!(map_file(&empty).is_none());
        // Nothing is left open on a file that wasn't mapped.
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_users_languages_are_locale_names() {
        let languages = user_languages();
        assert!(!languages.is_empty());
        assert!(
            languages
                .iter()
                .all(|name| name.len() >= 2 && name.is_ascii())
        );
    }
}
