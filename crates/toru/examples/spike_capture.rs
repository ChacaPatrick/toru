//! Spike for `docs/spikes.md` section 5.1: is a GDI capture faithful to what is on screen, and can
//! Toru's own window be kept out of one without moving it?
//!
//! `spike_capture fidelity` draws known pixels in a small plain Win32 window on each monitor, reads
//! them back with `xcap` and prints how they differ.
//! `spike_capture exclude` opens a magenta GPUI window and counts its pixels in a capture, with and
//! without `WDA_EXCLUDEFROMCAPTURE`, after several waits.
//!
//! Captures stay in memory and only counts are printed.

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null,
    thread,
    time::Duration,
};

use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowKind, WindowOptions, div, point,
    prelude::*, px, rgb, size,
};
use gpui_platform::application;
use xcap::{Monitor, image::RgbaImage};

const PATTERN_WIDTH: i32 = 256;
const PATTERN_BAND: i32 = 32;
const PATTERN_HEIGHT: i32 = PATTERN_BAND * 3;
const PATTERN_OFFSET: i32 = 100;
/// A pixel further than this from the source counts as wrong, as in section 1.2's report.
const OFF_THRESHOLD: i32 = 3;
const WAITS_MS: [u64; 6] = [0, 16, 33, 50, 100, 200];
const RUNS: usize = 3;
const MAGENTA: [u8; 3] = [255, 0, 255];

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const SW_SHOWNOACTIVATE: i32 = 4;
const WM_PAINT: u32 = 0x000F;
const PM_REMOVE: u32 = 1;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;
const WDA_NONE: u32 = 0;
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
const PER_MONITOR_AWARE_V2: isize = -4;

#[repr(C)]
struct WndClassExW {
    size: u32,
    style: u32,
    proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
    class_extra: i32,
    window_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
    small_icon: isize,
}

#[repr(C)]
struct PaintStruct {
    hdc: isize,
    erase: i32,
    paint: [i32; 4],
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}

#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: [i32; 2],
    private: u32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_per_meter: i32,
    y_per_meter: i32,
    clr_used: u32,
    clr_important: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(class: *const WndClassExW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const c_void,
    ) -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: isize) -> i32;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn BeginPaint(hwnd: isize, paint: *mut PaintStruct) -> isize;
    fn EndPaint(hwnd: isize, paint: *const PaintStruct) -> i32;
    fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    fn EnumThreadWindows(
        thread: u32,
        callback: unsafe extern "system" fn(isize, isize) -> i32,
        param: isize,
    ) -> i32;
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn SetDIBitsToDevice(
        hdc: isize,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        src_x: i32,
        src_y: i32,
        start_scan: u32,
        scan_lines: u32,
        bits: *const c_void,
        info: *const BitmapInfoHeader,
        usage: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn GetModuleHandleW(name: *const u16) -> isize;
}

/// The first band alternates 64 and 192 by column, as section 1.2's quads did. The second is black
/// and white by column. The third is flat steps, which sharpening leaves alone.
fn source_pixel(x: i32, y: i32) -> [u8; 3] {
    let grey = match y / PATTERN_BAND {
        0 => [64, 192][(x % 2) as usize],
        1 => [0, 255][(x % 2) as usize],
        _ => (x / 16 * 16) as u8,
    };
    [grey, grey, grey]
}

/// Top-down BGRA for `SetDIBitsToDevice`.
fn source_bits() -> Vec<u8> {
    let mut bits = Vec::with_capacity((PATTERN_WIDTH * PATTERN_HEIGHT * 4) as usize);
    for y in 0..PATTERN_HEIGHT {
        for x in 0..PATTERN_WIDTH {
            let [r, g, b] = source_pixel(x, y);
            bits.extend_from_slice(&[b, g, r, 255]);
        }
    }
    bits
}

unsafe extern "system" fn pattern_proc(
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if message != WM_PAINT {
        // SAFETY: passes through what Windows sent.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let bits = source_bits();
    let info = BitmapInfoHeader {
        size: size_of::<BitmapInfoHeader>() as u32,
        width: PATTERN_WIDTH,
        height: -PATTERN_HEIGHT,
        planes: 1,
        bit_count: 32,
        compression: BI_RGB,
        size_image: 0,
        x_per_meter: 0,
        y_per_meter: 0,
        clr_used: 0,
        clr_important: 0,
    };
    // SAFETY: the bits and header are valid for the call, and the DC comes from `BeginPaint`.
    unsafe {
        let mut paint: PaintStruct = zeroed();
        let hdc = BeginPaint(hwnd, &mut paint);
        SetDIBitsToDevice(
            hdc,
            0,
            0,
            PATTERN_WIDTH as u32,
            PATTERN_HEIGHT as u32,
            0,
            0,
            0,
            PATTERN_HEIGHT as u32,
            bits.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
        );
        EndPaint(hwnd, &paint);
    }
    0
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn pump(for_time: Duration) {
    let end = std::time::Instant::now() + for_time;
    while std::time::Instant::now() < end {
        // SAFETY: `msg` is written by `PeekMessageW` before it is read.
        unsafe {
            let mut msg: Msg = zeroed();
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn open_pattern_window(class: &[u16], x: i32, y: i32) -> isize {
    // SAFETY: `class` is a registered, NUL-terminated class name.
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            x,
            y,
            PATTERN_WIDTH,
            PATTERN_HEIGHT,
            0,
            0,
            GetModuleHandleW(null()),
            null(),
        );
        assert!(hwnd != 0, "couldn't create the pattern window");
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        UpdateWindow(hwnd);
        hwnd
    }
}

#[derive(Default)]
struct Band {
    pixels: u32,
    exact: u32,
    wrong: u32,
    worst: i32,
    total_error: u64,
    low: Option<u8>,
    high: Option<u8>,
}

impl Band {
    fn add(&mut self, read: [u8; 3], expected: [u8; 3]) {
        let error = (0..3)
            .map(|channel| (i32::from(read[channel]) - i32::from(expected[channel])).abs())
            .max()
            .unwrap_or(0);
        self.pixels += 1;
        self.exact += u32::from(error == 0);
        self.wrong += u32::from(error > OFF_THRESHOLD);
        self.worst = self.worst.max(error);
        self.total_error += error as u64;
        self.low = Some(self.low.map_or(read[0], |low| low.min(read[0])));
        self.high = Some(self.high.map_or(read[0], |high| high.max(read[0])));
    }

    fn print(&self, name: &str) {
        println!(
            "  {name}: {} px, exact {}, wrong {}, worst {}, mean error {:.2}, values {}..{}",
            self.pixels,
            self.exact,
            self.wrong,
            self.worst,
            self.total_error as f64 / self.pixels.max(1) as f64,
            self.low.unwrap_or(0),
            self.high.unwrap_or(0),
        );
    }
}

fn compare_pattern(frame: &RgbaImage, origin: (i32, i32)) {
    let mut bands: [Band; 3] = Default::default();
    for y in 0..PATTERN_HEIGHT {
        for x in 0..PATTERN_WIDTH {
            let pixel = frame
                .get_pixel((origin.0 + x) as u32, (origin.1 + y) as u32)
                .0;
            bands[(y / PATTERN_BAND) as usize]
                .add([pixel[0], pixel[1], pixel[2]], source_pixel(x, y));
        }
    }
    bands[0].print("columns 64/192");
    bands[1].print("columns 0/255");
    bands[2].print("flat steps");
}

fn fidelity() {
    // SAFETY: called before anything creates a window.
    unsafe { SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) };
    let class = wide("ToruSpikeCapture");
    let registration = WndClassExW {
        size: size_of::<WndClassExW>() as u32,
        style: 0,
        proc: pattern_proc,
        class_extra: 0,
        window_extra: 0,
        // SAFETY: a null name asks for this executable's own handle.
        instance: unsafe { GetModuleHandleW(null()) },
        icon: 0,
        cursor: 0,
        background: 0,
        menu_name: null(),
        class_name: class.as_ptr(),
        small_icon: 0,
    };
    // SAFETY: the structure and class name are valid for the call.
    assert!(unsafe { RegisterClassExW(&registration) } != 0);

    for monitor in Monitor::all().expect("list monitors") {
        let (x, y) = (monitor.x().unwrap(), monitor.y().unwrap());
        println!(
            "monitor at ({x}, {y}), {}x{}, {} Hz, scale {}",
            monitor.width().unwrap(),
            monitor.height().unwrap(),
            monitor.frequency().unwrap(),
            monitor.scale_factor().unwrap(),
        );
        let hwnd = open_pattern_window(&class, x + PATTERN_OFFSET, y + PATTERN_OFFSET);
        pump(Duration::from_millis(300));
        for run in 1..=RUNS {
            let frame = monitor.capture_image().expect("capture");
            println!(" run {run}:");
            compare_pattern(&frame, (PATTERN_OFFSET, PATTERN_OFFSET));
            pump(Duration::from_millis(100));
        }
        // SAFETY: the window is this thread's own.
        unsafe { DestroyWindow(hwnd) };
        pump(Duration::from_millis(100));
    }
}

unsafe extern "system" fn set_affinity(hwnd: isize, affinity: isize) -> i32 {
    // SAFETY: the handle comes from the enumeration, on the thread that owns it.
    unsafe { SetWindowDisplayAffinity(hwnd, affinity as u32) };
    1
}

fn set_this_threads_affinity(affinity: u32) {
    // SAFETY: the callback only passes the handle on, and the call returns when enumeration ends.
    unsafe {
        EnumThreadWindows(GetCurrentThreadId(), set_affinity, affinity as isize);
    }
}

fn magenta_pixels(monitor: &Monitor) -> usize {
    let frame = monitor.capture_image().expect("capture");
    frame
        .pixels()
        .filter(|pixel| pixel.0[0..3] == MAGENTA)
        .count()
}

struct Magenta;

impl Render for Magenta {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xFF00FF))
    }
}

fn exclude() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::new(point(px(120.), px(120.)), size(px(320.), px(200.)));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                // Topmost, so nothing the owner opens can cover it mid-run.
                kind: WindowKind::PopUp,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Magenta),
        )
        .expect("open window");
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(600))
                .await;
            let monitors = Monitor::all().expect("list monitors");
            let primary = monitors
                .into_iter()
                .find(|monitor| monitor.is_primary().unwrap())
                .expect("a primary monitor");
            println!("visible: {} magenta px", magenta_pixels(&primary));
            for run in 1..=RUNS {
                for wait in WAITS_MS {
                    set_this_threads_affinity(WDA_EXCLUDEFROMCAPTURE);
                    thread::sleep(Duration::from_millis(wait));
                    let excluded = magenta_pixels(&primary);
                    set_this_threads_affinity(WDA_NONE);
                    thread::sleep(Duration::from_millis(150));
                    let back = magenta_pixels(&primary);
                    println!("run {run}, wait {wait} ms: excluded {excluded}, afterwards {back}");
                }
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("fidelity") => fidelity(),
        Some("exclude") => exclude(),
        _ => eprintln!("usage: spike_capture fidelity | exclude"),
    }
}
