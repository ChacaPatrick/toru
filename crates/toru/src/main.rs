#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The theme carries the values for the cards, gallery and editor, and the controls include the
// card, settings and bottom bar ones; the sections that use them follow (Phases 4 and 8).
mod main_window;
mod platform;
#[allow(dead_code)]
mod theme;
#[allow(dead_code)]
mod ui;

use std::sync::Arc;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;
use toru_core::{
    settings::{Look, PaletteChoice, Settings},
    store::{Store, StoreError},
};

use main_window::MainWindow;
use theme::{Theme, install_fonts, set_theme};

/// The mockups' size.
const WINDOW_SIZE: (f32, f32) = (1280., 940.);
/// Wide enough for three usable cards beside the open settings column; the toolbar wraps.
const MIN_WINDOW_SIZE: (f32, f32) = (900., 480.);

/// `toru [japandi|pixel_art|neo_brutalism] [palette]` opens the window in that look and palette,
/// so each can be checked against the mockups without clicking through. It only changes what is
/// shown, so the saved settings keep their own look.
fn appearance_from_args(settings: &Settings) -> Settings {
    let mut appearance = settings.clone();
    let mut args = std::env::args().skip(1);
    appearance.look = match args.next().as_deref() {
        Some("japandi") => Look::Japandi,
        Some("pixel_art") => Look::PixelArt,
        Some("neo_brutalism") => Look::NeoBrutalism,
        _ => appearance.look,
    };
    if let Some(palette) = args.next().and_then(|arg| arg.parse().ok()) {
        appearance.appearance_mut(appearance.look).palette = PaletteChoice::BuiltIn(palette);
    }
    appearance
}

fn main() {
    let data_dir = platform::app_data_dir();
    let cannot_start = |error: StoreError| -> ! {
        platform::show_startup_error(&format!(
            "Toru couldn't open its data in {}.

{error:?}",
            data_dir.display()
        ));
        std::process::exit(1)
    };
    let store = match Store::open(&data_dir) {
        Ok(store) => Arc::new(store),
        // Another Toru has this folder. Section 6.1 will show that one instead.
        Err(StoreError::AlreadyOpen) => return,
        Err(error) => cannot_start(error),
    };
    // The store only fails here if the database itself can't be read, and the defaults still work.
    let settings = store.load_settings().unwrap_or_default();
    let session = store
        .new_session()
        .unwrap_or_else(|error| cannot_start(error));

    application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        ui::init(cx);
        install_fonts(cx);
        set_theme(Theme::from_settings(&appearance_from_args(&settings)), cx);
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
            |window, cx| cx.new(|cx| MainWindow::new(store, session, settings, window, cx)),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
