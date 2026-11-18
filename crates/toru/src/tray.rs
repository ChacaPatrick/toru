use std::{error::Error, sync::mpsc::Sender};

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

use crate::platform;

const OPEN_ITEM: &str = "open";
const QUIT_ITEM: &str = "quit";

/// Also carries a global shortcut's press (`shortcuts`), by the id the registry knows it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Show,
    Quit,
    Hotkey(u32),
}

/// Dropping it takes the icon out of the tray.
pub struct Tray {
    _icon: TrayIcon,
}

impl Tray {
    /// Call on GPUI's main thread, whose message loop serves the icon's hidden window
    /// (`docs/spikes.md` §1.1). The handlers run inside a window procedure and must not touch
    /// GPUI, so they only send, holding `sender` for the life of the process.
    pub fn new(sender: Sender<Request>) -> Result<Tray, Box<dyn Error>> {
        let menu = Menu::new();
        for (id, text) in [(OPEN_ITEM, "Open Toru"), (QUIT_ITEM, "Quit")] {
            menu.append(&MenuItem::with_id(id, text, true, None))?;
        }
        let tray = TrayIconBuilder::new()
            .with_tooltip("Toru")
            .with_icon(platform::tray_icon()?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;

        let click_sender = sender.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                click_sender.send(Request::Show).ok();
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let request = if event.id == OPEN_ITEM {
                Request::Show
            } else if event.id == QUIT_ITEM {
                Request::Quit
            } else {
                return;
            };
            sender.send(request).ok();
        }));
        Ok(Tray { _icon: tray })
    }
}
