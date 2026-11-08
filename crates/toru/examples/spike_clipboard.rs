//! Spike for `docs/spikes.md` section 1.4: put one image, or a set of files, on the Windows clipboard.
//!
//! `spike_clipboard image [width height] [--gpui]` copies a generated test image (default 1440x900).
//! `spike_clipboard files [count] [--gpui]` writes generated PNGs to a temp folder and copies that file list.
//! `--gpui` runs the same copy on GPUI's background executor instead of the main thread.

use std::{env, fs, thread, time::Instant};

use clipboard_rs::{
    Clipboard, ClipboardContext,
    common::{RustImage, RustImageData},
};
use gpui::{App, AppContext};
use gpui_platform::application;
use xcap::image::{DynamicImage, RgbaImage};

const DEFAULT_WIDTH: u32 = 1440;
const DEFAULT_HEIGHT: u32 = 900;
const DEFAULT_FILE_COUNT: u32 = 3;
const TEMP_FOLDER: &str = "toru_spike_clipboard";

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Top band: opaque, known colours, to detect swapped channels. Second band: one colour at four alphas.
/// The rest is a gradient that doesn't compress to nothing, and `seed` makes each file different.
fn test_pixel(x: u32, y: u32, width: u32, seed: u32) -> [u8; 4] {
    let column = (x * 4 / width).min(3) as usize;
    match y {
        0..100 => [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [10, 20, 30, 255],
        ][column],
        100..200 => [200, 100, 50, [128, 64, 1, 0][column]],
        _ => [x as u8, y as u8, (x + y + seed) as u8, 255],
    }
}

fn test_image(width: u32, height: u32, seed: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| test_pixel(x, y, width, seed).into())
}

fn copy_image(width: u32, height: u32) -> Result {
    let start = Instant::now();
    let image =
        RustImageData::from_dynamic_image(DynamicImage::ImageRgba8(test_image(width, height, 0)));
    println!("generate {width}x{height}: {:?}", start.elapsed());

    // `set_image` does both of these encodes itself, with the clipboard open. Timed separately to show the breakdown.
    let start = Instant::now();
    let png = image.to_png()?.get_bytes().len();
    println!("PNG encode alone: {:?}, {png} bytes", start.elapsed());
    let start = Instant::now();
    let bmp = image.to_bitmap()?.get_bytes().len();
    println!("BMP encode alone: {:?}, {bmp} bytes", start.elapsed());

    let context = ClipboardContext::new()?;
    let start = Instant::now();
    context.set_image(image)?;
    println!("set_image: {:?}", start.elapsed());
    Ok(())
}

fn copy_files(count: u32) -> Result {
    let folder = env::temp_dir().join(TEMP_FOLDER);
    if folder.exists() {
        fs::remove_dir_all(&folder)?;
    }
    fs::create_dir_all(&folder)?;

    let start = Instant::now();
    let mut paths = Vec::new();
    for index in 0..count {
        // The first name has a space and non-ASCII characters, as a user's rename could.
        let name = match index {
            0 => "screenshot 日本語 テスト.png".to_string(),
            _ => format!("screenshot_{index:02}.png"),
        };
        let path = folder.join(name);
        test_image(DEFAULT_WIDTH, DEFAULT_HEIGHT, index).save(&path)?;
        paths.push(path.to_string_lossy().into_owned());
    }
    println!("write {count} PNGs: {:?}", start.elapsed());
    println!("folder: {}", folder.display());

    let context = ClipboardContext::new()?;
    let start = Instant::now();
    context.set_files(paths)?;
    println!("set_files: {:?}", start.elapsed());
    Ok(())
}

fn run(args: &[String]) -> Result {
    println!("running on {:?}", thread::current().id());
    let number = |index: usize, default: u32| {
        args.get(index)
            .map_or(Ok(default), |value| value.parse::<u32>())
    };
    match args.first().map(String::as_str) {
        Some("image") => copy_image(number(1, DEFAULT_WIDTH)?, number(2, DEFAULT_HEIGHT)?),
        Some("files") => copy_files(number(1, DEFAULT_FILE_COUNT)?),
        _ => Err("usage: spike_clipboard image [width height] | files [count] [--gpui]".into()),
    }
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let on_gpui = args.iter().any(|arg| arg == "--gpui");
    args.retain(|arg| arg != "--gpui");

    if !on_gpui {
        run(&args).expect("clipboard spike failed");
        return;
    }

    application().run(move |cx: &mut App| {
        println!("GPUI main thread is {:?}", thread::current().id());
        cx.spawn(async move |cx| {
            let result = cx.background_spawn(async move { run(&args) }).await;
            result.expect("clipboard spike failed");
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
