#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The theme carries the values for the cards, gallery and editor, and the controls include the
// card, settings and bottom bar ones; the sections that use them follow (Phases 4 and 8).
mod editor;
mod main_window;
mod overlay;
mod platform;
mod shortcuts;
#[allow(dead_code)]
mod theme;
mod tray;
#[allow(dead_code)]
mod ui;

use std::path::Path;
use std::sync::{Arc, mpsc};

use gpui::{
    App, Bounds, Entity, QuitMode, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px,
    size,
};
use gpui_platform::application;
use toru_core::{
    settings::{BUILT_IN_PALETTES, Look, PaletteChoice, Settings},
    store::{Store, StoreError},
};

use main_window::MainWindow;
use shortcuts::Hotkeys;
use theme::{Theme, install_fonts, set_theme};
use tray::{Request, Tray};

/// The mockups' size.
const WINDOW_SIZE: (f32, f32) = (1280., 940.);
/// Wide enough for three usable cards beside the open settings column; the toolbar wraps.
const MIN_WINDOW_SIZE: (f32, f32) = (900., 480.);

/// `toru [japandi|pixel_art|neo_brutalism] [palette|custom] [editor]` opens the window in that
/// look and palette, and straight in the editor if asked, so each can be checked against the mockups
/// without clicking through. It only changes what is shown, so the saved settings keep their own
/// look until a look or palette is picked in settings, which saves what is shown.
fn appearance_from_args(settings: &Settings) -> (Settings, bool) {
    let mut appearance = settings.clone();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let open_editor = args.iter().any(|arg| arg == "editor");
    args.retain(|arg| arg != "editor");
    let mut args = args.into_iter();
    appearance.look = match args.next().as_deref() {
        Some("japandi") => Look::Japandi,
        Some("pixel_art") => Look::PixelArt,
        Some("neo_brutalism") => Look::NeoBrutalism,
        _ => appearance.look,
    };
    let palette = match args.next().as_deref() {
        Some("custom") => Some(PaletteChoice::Custom),
        Some(index) => index
            .parse()
            .ok()
            .filter(|index| *index < BUILT_IN_PALETTES)
            .map(PaletteChoice::BuiltIn),
        None => None,
    };
    if let Some(palette) = palette {
        appearance.appearance_mut(appearance.look).palette = palette;
    }
    (appearance, open_editor)
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
        // Another Toru has this folder: ask it to show its window instead.
        Err(StoreError::AlreadyOpen) => {
            platform::signal_running_instance(&data_dir);
            return;
        }
        Err(error) => cannot_start(error),
    };
    // The store only fails here if the database itself can't be read, and the defaults still work.
    let settings = store.load_settings().unwrap_or_default();
    let session = store
        .new_session()
        .unwrap_or_else(|error| cannot_start(error));

    // The default quit mode ends the app when its last window closes, which a tray app can't
    // have: the window closes into the tray, and quitting is explicit (`main_window::closing`).
    let app = application().with_quit_mode(QuitMode::Explicit);
    app.run(move |cx: &mut App| {
        gpui_kit::init(cx);
        ui::init(cx);
        install_fonts(cx);
        let (appearance, open_editor) = appearance_from_args(&settings);
        set_theme(Theme::from_settings(&appearance), cx);
        let bounds = Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
        // One channel carries the tray's requests, a second launch and the global shortcuts'
        // presses to the main thread (`run_in_tray`). The shortcut registry opens its hidden
        // window here, on that thread, like the tray.
        let (sender, receiver) = mpsc::channel();
        let hotkeys = Hotkeys::new(sender.clone(), Request::Hotkey);
        let main = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // The title bar is drawn by `main_window::title_bar`; `appears_transparent` hides
                // the system one while keeping its frame behaviour.
                titlebar: Some(TitlebarOptions {
                    title: Some(main_window::WINDOW_TITLE.into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_min_size: Some(size(px(MIN_WINDOW_SIZE.0), px(MIN_WINDOW_SIZE.1))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut main = MainWindow::new(store, session, settings, hotkeys, window, cx);
                    main.clean_up_history(cx);
                    if open_editor {
                        main.open_newest_in_editor(cx);
                    }
                    main
                })
            },
        )
        .expect("failed to open the main window");
        cx.activate(true);
        run_in_tray(main.1, &data_dir, sender, receiver, cx);
    });
}

/// Puts the icon in the tray and acts on it, and on a second launch, for the life of the app
/// (`docs/spikes.md` §1.1). The tray's handlers run inside a window procedure and the launch
/// watcher on its own thread, so both only send into a channel, and a task on the main thread
/// does the work. At quit, Toru's own or Windows shutting down, the window applies the On close
/// setting (`main_window::closing`) and dropping the icon takes it out of the tray at once.
fn run_in_tray(
    main: Entity<MainWindow>,
    data_dir: &Path,
    sender: mpsc::Sender<Request>,
    mut receiver: mpsc::Receiver<Request>,
    cx: &mut App,
) {
    // The tray owns a hidden window, so it is created here, on GPUI's main thread, whose message
    // loop then serves it.
    let mut tray = match Tray::new(sender.clone()) {
        Ok(tray) => Some(tray),
        // Rare. The window says so and closing it quits, since there is no icon to come back from.
        Err(error) => {
            eprintln!("Toru couldn't add its tray icon: {error}");
            main.update(cx, |this, cx| this.tray_unavailable(cx));
            None
        }
    };
    platform::watch_for_show(data_dir, move || {
        sender.send(Request::Show).ok();
    });
    cx.on_app_quit({
        let main = main.clone();
        move |cx| {
            tray.take();
            main.update(cx, |this, cx| this.on_app_quit(cx))
        }
    })
    .detach();
    cx.spawn(async move |cx| {
        loop {
            // `recv` blocks, so each wait runs on a background thread and returns the receiver.
            let (returned, request) = cx
                .background_spawn(async move {
                    let request = receiver.recv();
                    (receiver, request)
                })
                .await;
            receiver = returned;
            let Ok(request) = request else {
                break;
            };
            cx.update(|cx| match request {
                Request::Show => platform::show_main_window(),
                Request::Quit => main.update(cx, |this, cx| this.request_quit(cx)),
                Request::Hotkey(id) => main.update(cx, |this, cx| this.hotkey_pressed(id, cx)),
            });
        }
    })
    .detach();
}
