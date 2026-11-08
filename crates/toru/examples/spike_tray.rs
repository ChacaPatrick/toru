//! Spike for `docs/spikes.md` section 1.1: a tray icon and global hotkeys alongside GPUI's event loop.

use std::{collections::HashMap, sync::mpsc, thread};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::{
    App, Bounds, Context, Entity, QuitMode, Render, SharedString, TitlebarOptions, Window,
    WindowBounds, WindowOptions, black, div, prelude::*, px, size, white,
};
use gpui_platform::application;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

const OPEN_ITEM: &str = "open";
const QUIT_ITEM: &str = "quit";
const VISIBLE_LINES: usize = 30;

/// What the tray and hotkey handlers send to the GPUI side.
enum Message {
    Log(String),
    OpenWindow,
    Quit,
}

#[derive(Default)]
struct EventLog {
    lines: Vec<SharedString>,
}

impl Render for EventLog {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        println!("[render] {} lines in the log", self.lines.len());
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_3()
            .bg(white())
            .text_color(black())
            .text_sm()
            .children(
                self.lines
                    .iter()
                    .rev()
                    .take(VISIBLE_LINES)
                    .map(|line| div().child(line.clone())),
            )
    }
}

fn log(event_log: &Entity<EventLog>, line: String, cx: &mut App) {
    println!("{line}");
    event_log.update(cx, |event_log, cx| {
        event_log.lines.push(line.into());
        cx.notify();
    });
}

fn open_or_focus_window(event_log: &Entity<EventLog>, cx: &mut App) {
    if let Some(window) = cx.windows().first() {
        window
            .update(cx, |_, window, _| window.activate_window())
            .ok();
        return;
    }

    let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
    let event_log = event_log.clone();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Toru tray spike".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |_, _| event_log,
    )
    .expect("failed to open the window");
}

fn register_hotkeys(
    manager: &GlobalHotKeyManager,
    event_log: &Entity<EventLog>,
    cx: &mut App,
) -> HashMap<u32, &'static str> {
    let ctrl_shift = Some(Modifiers::CONTROL | Modifiers::SHIFT);
    let hotkeys = [
        ("PrintScreen", HotKey::new(None, Code::PrintScreen)),
        ("Ctrl+Shift+W", HotKey::new(ctrl_shift, Code::KeyW)),
        ("Ctrl+Shift+F", HotKey::new(ctrl_shift, Code::KeyF)),
        // Not a real Toru shortcut: lets an automated test press something harmless.
        ("Ctrl+Shift+F12", HotKey::new(ctrl_shift, Code::F12)),
    ];

    let mut names = HashMap::new();
    for (name, hotkey) in hotkeys {
        let result = match manager.register(hotkey) {
            Ok(()) => {
                names.insert(hotkey.id(), name);
                "registered".to_string()
            }
            Err(error) => format!("FAILED: {error} [{error:?}]"),
        };
        log(event_log, format!("{name}: {result}"), cx);
    }
    names
}

fn build_tray() -> TrayIcon {
    let menu = Menu::new();
    for (id, text) in [(OPEN_ITEM, "Open window"), (QUIT_ITEM, "Quit")] {
        menu.append(&MenuItem::with_id(id, text, true, None))
            .expect("failed to add a tray menu item");
    }

    const ICON_SIZE: u32 = 32;
    let pixels = [0xD9, 0x6B, 0x3A, 0xFF].repeat((ICON_SIZE * ICON_SIZE) as usize);
    let icon = Icon::from_rgba(pixels, ICON_SIZE, ICON_SIZE).expect("invalid tray icon image");

    TrayIconBuilder::new()
        .with_tooltip("Toru tray spike")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .expect("failed to create the tray icon")
}

/// Sends every tray and hotkey event into one channel. These handlers can run inside a Win32
/// window procedure or on `global-hotkey`'s key-release thread, so they must not touch GPUI.
fn forward_events(hotkey_names: HashMap<u32, &'static str>, sender: mpsc::Sender<Message>) {
    let hotkey_sender = sender.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        let name = hotkey_names.get(&event.id).copied().unwrap_or("unknown");
        let source = thread::current().id();
        let line = format!("hotkey {name} {:?}, handler on {source:?}", event.state);
        hotkey_sender.send(Message::Log(line)).ok();
    }));

    let tray_sender = sender.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if !matches!(event, TrayIconEvent::Move { .. }) {
            let source = thread::current().id();
            let line = format!("tray {event:?}, handler on {source:?}");
            tray_sender.send(Message::Log(line)).ok();
        }
    }));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let source = thread::current().id();
        let line = format!("menu {:?}, handler on {source:?}", event.id.as_ref());
        sender.send(Message::Log(line)).ok();
        if event.id == OPEN_ITEM {
            sender.send(Message::OpenWindow).ok();
        } else if event.id == QUIT_ITEM {
            sender.send(Message::Quit).ok();
        }
    }));
}

fn main() {
    // The default quit mode ends the app when the last window closes, which would rule out a tray.
    let app = application().with_quit_mode(QuitMode::Explicit);
    app.run(|cx: &mut App| {
        println!("GPUI main thread is {:?}", thread::current().id());
        let event_log = cx.new(|_| EventLog::default());
        open_or_focus_window(&event_log, cx);

        cx.on_window_closed({
            let event_log = event_log.clone();
            move |cx, _| log(&event_log, "window closed, app still running".into(), cx)
        })
        .detach();

        // Both own hidden windows, so both must be created on GPUI's main thread: its GetMessageW
        // loop is the only message pump, and it dispatches their WM_HOTKEY and tray messages.
        let manager = GlobalHotKeyManager::new().expect("failed to create the hotkey manager");
        let names = register_hotkeys(&manager, &event_log, cx);
        let tray = build_tray();

        let (sender, mut receiver) = mpsc::channel();
        forward_events(names, sender);

        cx.spawn(async move |cx| {
            // Moved in so they live until Quit, and dropping the tray icon removes it from the tray.
            let _keep_alive = (manager, tray);
            loop {
                // `recv` blocks, so each wait runs on a background thread and returns the receiver.
                let (returned, message) = cx
                    .background_spawn(async move {
                        let message = receiver.recv();
                        (receiver, message)
                    })
                    .await;
                receiver = returned;

                println!("event reached GPUI on {:?}", thread::current().id());
                match message.expect("the event handlers hold a sender for the whole run") {
                    Message::Log(line) => cx.update(|cx| log(&event_log, line, cx)),
                    Message::OpenWindow => cx.update(|cx| open_or_focus_window(&event_log, cx)),
                    Message::Quit => {
                        cx.update(|cx| cx.quit());
                        break;
                    }
                }
            }
        })
        .detach();
    });
}
