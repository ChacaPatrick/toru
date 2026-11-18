use tray_icon::{BadIcon, Icon};

/// `build.rs` embeds `assets/toru.ico` under this id, the one GPUI loads the window's icon from.
const APP_ICON_RESOURCE: u16 = 1;
const SM_CXSMICON: i32 = 49;
const SM_CYSMICON: i32 = 50;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
}

pub fn tray_icon() -> Result<Icon, BadIcon> {
    // SAFETY: both only read a system setting.
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)) };
    // Asked for no size, Windows loads the 32 px frame and the tray shrinks it, which loses the
    // drawings made for 16 to 24 px.
    Icon::from_resource(APP_ICON_RESOURCE, Some((width as u32, height as u32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exe_carries_the_icon_the_tray_loads() {
        assert!(tray_icon().is_ok());
    }
}
