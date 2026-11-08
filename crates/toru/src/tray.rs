//! The tray icon and its menu (`docs/spikes.md` §1.1): created on GPUI's main thread, whose
//! message loop also serves the icon's hidden window, and heard through a channel, since its
//! handlers run inside a window procedure and must not touch GPUI. A left click on the icon and
//! the menu's Open both ask for the window; Quit goes through the same On close path as closing
//! the window with the setting off.

use std::{error::Error, sync::mpsc::Sender};

use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

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
        let icon = Icon::from_rgba(icon_pixels(ICON_SIZE), ICON_SIZE, ICON_SIZE)
            .expect("the tray icon is drawn at its stated size");
        let tray = TrayIconBuilder::new()
            .with_tooltip("Toru")
            .with_icon(icon)
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

/// The icon is `assets/icon.svg` drawn by hand: a rounded square in its teal with a white T,
/// which is only two bars. The tray scales it down to the icon size the shell uses.
const ICON_SIZE: u32 = 32;
/// `assets/icon.svg`'s `fill` and corner radius, for the 64-unit view box it is drawn in.
const ICON_TEAL: [u8; 3] = [0x08, 0x91, 0xb2];
const ICON_RADIUS: f32 = 14. / 64.;
/// The T's bar and stem, as fractions of the side: left, top, right, bottom.
const BAR: [f32; 4] = [0.25, 0.25, 0.75, 0.375];
const STEM: [f32; 4] = [0.4375, 0.25, 0.5625, 0.75];

/// RGBA pixels, row by row, of the icon at `size` by `size`.
fn icon_pixels(size: u32) -> Vec<u8> {
    let side = size as f32;
    let radius = ICON_RADIUS * side;
    let inside = |[left, top, right, bottom]: [f32; 4], x: f32, y: f32| {
        x >= left * side && x < right * side && y >= top * side && y < bottom * side
    };
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for column in 0..size {
            // The pixel's centre.
            let (x, y) = (column as f32 + 0.5, row as f32 + 0.5);
            let pixel = if !in_rounded_square(x, y, side, radius) {
                [0, 0, 0, 0]
            } else if inside(BAR, x, y) || inside(STEM, x, y) {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [ICON_TEAL[0], ICON_TEAL[1], ICON_TEAL[2], 0xff]
            };
            pixels.extend(pixel);
        }
    }
    pixels
}

/// Whether the point is inside a square of `side` with corners rounded by `radius`.
fn in_rounded_square(x: f32, y: f32, side: f32, radius: f32) -> bool {
    // How far the point is past the corner circle's centre on each axis; zero along the edges.
    let dx = (radius - x).max(x - (side - radius)).max(0.);
    let dy = (radius - y).max(y - (side - radius)).max(0.);
    dx * dx + dy * dy <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * size + x) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn the_icon_is_a_teal_square_with_a_white_t_and_clear_corners() {
        let pixels = icon_pixels(ICON_SIZE);
        assert_eq!(pixels.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        let teal = [ICON_TEAL[0], ICON_TEAL[1], ICON_TEAL[2], 0xff];
        let white = [0xff; 4];
        assert_eq!(pixel(&pixels, ICON_SIZE, 0, 0), [0; 4], "corner is clear");
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 0), teal, "top edge is teal");
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 9), white, "bar");
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 20), white, "stem");
        assert_eq!(pixel(&pixels, ICON_SIZE, 9, 20), teal, "beside the stem");
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 27), teal, "below the stem");
    }

    #[test]
    fn the_rounded_square_keeps_its_edges_and_drops_its_corners() {
        assert!(in_rounded_square(16., 0.5, 32., 7.));
        assert!(in_rounded_square(0.5, 16., 32., 7.));
        assert!(!in_rounded_square(0.5, 0.5, 32., 7.));
        assert!(in_rounded_square(7., 7., 32., 7.));
    }
}
