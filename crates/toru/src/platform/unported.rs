use std::path::PathBuf;

use global_hotkey::hotkey::Code;
use toru_core::time::LocalDateTime;

use super::HeldModifiers;

/// Where the platform can't say which key is down, the recorder falls back to the names GPUI
/// gives keys.
pub fn pressed_key(_keys: impl Iterator<Item = Code>) -> Option<Code> {
    None
}

pub fn held_modifiers() -> HeldModifiers {
    HeldModifiers {
        control: false,
        alt: false,
        shift: false,
        win: false,
    }
}

pub fn restore_window(_window: &gpui::Window, _cx: &gpui::App) {}

pub fn default_save_folder() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Pictures").join("Screenshots"))
}

pub fn reveal_file(_path: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// No Recycle Bin here yet, so the file is deleted for good.
pub fn recycle_file(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

pub fn bring_overlay_to_foreground() {}

pub fn fallback_fonts() -> Vec<(PathBuf, Option<crate::editor::text::Cjk>)> {
    Vec::new()
}

/// Read whole here, where no mapping is written yet.
pub fn map_file(path: &std::path::Path) -> Option<&'static [u8]> {
    let data = std::fs::read(path).ok().filter(|data| !data.is_empty())?;
    Some(Box::leak(data.into_boxed_slice()))
}

/// The locale the environment sets, such as `ja_JP.UTF-8`.
pub fn user_languages() -> Vec<String> {
    std::env::var("LANG").into_iter().collect()
}

pub fn exclude_windows_from_capture(_exclude: bool) {}

pub fn is_window_pickable(_id: u32) -> bool {
    true
}

pub fn mark_running() {}

pub fn repoint_start_with_windows() {}

pub fn starts_with_windows() -> bool {
    false
}

pub fn reduce_motion() -> bool {
    false
}

pub fn set_starts_with_windows(_on: bool) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

pub fn trim_working_set() {}

/// Updates aren't written for this platform yet.
pub fn fetch_text(_url: &str) -> std::io::Result<String> {
    Err(std::io::ErrorKind::Unsupported.into())
}

pub fn download(_url: &str, _to: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

pub fn start_installer(_setup: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

pub fn hide_main_window() {}

pub fn show_main_window() {}

pub fn while_holding_up_shutdown<T>(_reason: &str, write: impl FnOnce() -> T) -> T {
    write()
}

pub fn main_window_hidden() -> bool {
    false
}

pub fn request_close(window: &mut gpui::Window) {
    window.remove_window();
}

pub fn watch_for_show(_data_dir: &std::path::Path, _on_show: impl Fn() + Send + 'static) {}

pub fn signal_running_instance(_data_dir: &std::path::Path) -> bool {
    false
}

pub fn show_startup_error(message: &str) {
    eprintln!("{message}");
}

pub fn local_now() -> LocalDateTime {
    unimplemented!("local time isn't written for this platform yet")
}
