#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The theme carries the values for the cards, gallery and editor, and the controls include the
// card, settings and bottom bar ones; the sections that use them follow (Phases 4 and 8).
mod main_window;
mod platform;
#[allow(dead_code)]
mod theme;
#[allow(dead_code)]
mod ui;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;
use toru_core::settings::{Look, PaletteChoice, Settings};

use main_window::MainWindow;
use theme::{Theme, install_fonts, set_theme};

/// The mockups' size.
const WINDOW_SIZE: (f32, f32) = (1280., 940.);
const MIN_WINDOW_SIZE: (f32, f32) = (720., 480.);

/// `toru [japandi|pixel_art|neo_brutalism] [palette]` opens the window in that look and palette,
/// so each can be checked against the mockups without clicking through.
fn settings_from_args() -> Settings {
    let mut settings = Settings::default();
    let mut args = std::env::args().skip(1);
    settings.look = match args.next().as_deref() {
        Some("pixel_art") => Look::PixelArt,
        Some("neo_brutalism") => Look::NeoBrutalism,
        _ => Look::Japandi,
    };
    if let Some(palette) = args.next().and_then(|arg| arg.parse().ok()) {
        settings.appearance_mut(settings.look).palette = PaletteChoice::BuiltIn(palette);
    }
    settings
}

fn main() {
    application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        ui::init(cx);
        install_fonts(cx);
        let settings = settings_from_args();
        set_theme(Theme::from_settings(&settings), cx);
        let bounds = Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // The title bar is drawn by `main_window::title_bar`; `appears_transparent` hides
                // the system one while keeping its frame behaviour.
                titlebar: Some(TitlebarOptions {
                    title: Some("Toru".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_min_size: Some(size(px(MIN_WINDOW_SIZE.0), px(MIN_WINDOW_SIZE.1))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| MainWindow::new(settings, window, cx)),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
