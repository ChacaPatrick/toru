use gpui::PlatformDisplay;

use super::utf16::until_nul;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `MONITORINFOEXW`. `device` is the adapter's name, `\\.\DISPLAY1`.
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

const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x1;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn EnumDisplayDevicesW(
        device: *const u16,
        index: u32,
        display_device: *mut DisplayDevice,
        flags: u32,
    ) -> i32;
}

/// GPUI's `DisplayId` is the `HMONITOR`, and `GetMonitorInfoW` answers in physical pixels for a
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

/// The monitor's device interface path, `\\?\DISPLAY#GSM5B09#5&2d9b2c1f&0&UID4352#{...}`. It
/// names the monitor's make and model and the port it is on, so it is the same after a restart
/// or an unplugging, and a different monitor on the same port gets its own; the same monitor
/// moved to another port counts as new. GPUI's `uuid` hashes only the adapter name,
/// `\\.\DISPLAY2`, which whichever monitor is plugged in there inherits. `None` for a monitor
/// Windows can't name, as a remote desktop's.
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
