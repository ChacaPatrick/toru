use std::io;

use super::utf16::wide;

/// `HKEY_CURRENT_USER`, sign-extended as Windows declares it.
const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const REG_SZ: u32 = 1;
#[cfg(test)]
pub(super) const REG_BINARY: u32 = 3;
const RRF_RT_REG_SZ: u32 = 0x2;
pub(super) const RRF_RT_REG_BINARY: u32 = 0x8;
const ERROR_FILE_NOT_FOUND: i32 = 2;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: isize,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut u8,
        bytes: *mut u32,
    ) -> i32;
    fn RegSetKeyValueW(
        key: isize,
        subkey: *const u16,
        value: *const u16,
        kind: u32,
        data: *const u8,
        bytes: u32,
    ) -> i32;
    fn RegDeleteKeyValueW(key: isize, subkey: *const u16, value: *const u16) -> i32;
    #[cfg(test)]
    fn RegDeleteKeyW(key: isize, subkey: *const u16) -> i32;
}

/// The key must have no subkeys.
#[cfg(test)]
pub(super) fn delete_user_key(subkey: &str) {
    let subkey = wide(subkey);
    // SAFETY: the name is NUL-terminated.
    unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, subkey.as_ptr()) };
}

pub(super) fn read_user_data(subkey: &str, value: &str, flags: u32) -> Option<Vec<u8>> {
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buffer = vec![0u8; 2048];
    let mut bytes = buffer.len() as u32;
    // SAFETY: the names are NUL-terminated and the buffer's size in bytes is passed along.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            flags,
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            &mut bytes,
        )
    };
    (status == 0).then(|| {
        buffer.truncate(bytes as usize);
        buffer
    })
}

pub(super) fn read_user_value(subkey: &str, value: &str) -> Option<String> {
    let data = read_user_data(subkey, value, RRF_RT_REG_SZ)?;
    let text: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&text))
}

/// Creates the key if need be.
pub(super) fn write_user_data(subkey: &str, value: &str, kind: u32, data: &[u8]) -> io::Result<()> {
    let (subkey, value) = (wide(subkey), wide(value));
    // SAFETY: both names are NUL-terminated, and the data's size in bytes is passed along.
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            kind,
            data.as_ptr(),
            data.len() as u32,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status))
    }
}

pub(super) fn write_user_value(subkey: &str, value: &str, text: &str) -> io::Result<()> {
    let data: Vec<u8> = wide(text)
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    write_user_data(subkey, value, REG_SZ, &data)
}

/// One that isn't there is already gone.
pub(super) fn delete_user_value(subkey: &str, value: &str) -> io::Result<()> {
    let (subkey, value) = (wide(subkey), wide(value));
    // SAFETY: both names are NUL-terminated.
    let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, subkey.as_ptr(), value.as_ptr()) };
    match status {
        0 | ERROR_FILE_NOT_FOUND => Ok(()),
        _ => Err(io::Error::from_raw_os_error(status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a key of the test's own, never the real Run key.
    #[test]
    fn a_user_value_is_written_read_and_deleted() {
        let key = format!(r"Software\Toru-tests-{}", std::process::id());
        let text = r#""C:\Users\Tōru Ä\toru.exe" --tray"#;
        assert_eq!(read_user_value(&key, "Toru"), None);
        write_user_value(&key, "Toru", text).unwrap();
        assert_eq!(read_user_value(&key, "Toru").as_deref(), Some(text));
        delete_user_value(&key, "Toru").unwrap();
        assert_eq!(read_user_value(&key, "Toru"), None);
        // Deleting a value that's already gone isn't an error.
        delete_user_value(&key, "Toru").unwrap();
        delete_user_key(&key);
    }
}
