//! The tray icon and its menu (`docs/spikes.md` §1.1): created on GPUI's main thread, whose
//! message loop also serves the icon's hidden window, and heard through a channel, since its
//! handlers run inside a window procedure and must not touch GPUI. A left click on the icon and
//! the menu's Open both ask for the window; Quit goes through the same On close path as closing
//! the window with the setting off.

use std::{error::Error, sync::mpsc::Sender};

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

use crate::platform;

const OPEN_ITEM: &str = "open";
const QUIT_ITEM: &str = "quit";

/// What the tray asks of the app, and, on the same channel, a global shortcut's press
/// (`shortcuts`), carrying the id the registry knows it by.
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
    /// Puts the icon in the tray. The handlers hold `sender` for the life of the process.
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
