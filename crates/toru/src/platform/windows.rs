pub mod accessibility;
pub mod capture;
pub mod display;
pub mod files;
pub mod fonts;
pub mod keyboard;
pub mod local_time;
pub mod main_window;
pub mod memory;
mod registry;
pub mod running_instance;
pub mod shutdown;
pub mod start_at_sign_in;
pub mod startup_error;
pub mod tray_icon;
pub mod update;
mod utf16;

#[cfg(test)]
fn unique_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("toru-platform-{}-{name}", std::process::id()))
}
