use gpui::{App, Context, DisplayId, SharedString};
use toru_core::capture::CaptureMode;

use super::MainWindow;
use crate::{overlay, platform};

/// A display Full screen can capture, as the picker shows it.
pub(super) struct Display {
    pub(super) id: DisplayId,
    /// What a per-display shortcut is stored under (`platform::display_key`): the monitor's own
    /// identity, which survives restarts and unplugging, where `id` doesn't.
    pub(super) key: String,
    /// "Display 1 · 2560×1440": its number and its size in physical pixels, which a capture has.
    pub(super) label: SharedString,
}

impl MainWindow {
    /// The displays as they are right now, since monitors come and go while Toru runs, and the
    /// index of the chosen one among them.
    pub(super) fn displays(&self, cx: &App) -> (Vec<Display>, usize) {
        let displays: Vec<Display> = cx
            .displays()
            .iter()
            .enumerate()
            .map(|(index, display)| {
                let (width, height) = platform::display_pixels(display.as_ref());
                Display {
                    id: display.id(),
                    key: platform::display_key(display.as_ref()),
                    label: format!("Display {} · {width}×{height}", index + 1).into(),
                }
            })
            .collect();
        let position = |id: Option<DisplayId>| displays.iter().position(|d| Some(d.id) == id);
        let chosen = position(self.display)
            .or_else(|| position(cx.primary_display().map(|d| d.id())))
            .unwrap_or(0);
        (displays, chosen)
    }

    /// Takes a capture now, whichever way it was asked for, in place of any countdown.
    pub(super) fn capture(&mut self, mode: CaptureMode, cx: &mut Context<Self>) {
        self.cancel_countdown(cx);
        match mode {
            CaptureMode::Region | CaptureMode::Window => self.capture_with_overlay(mode, cx),
            CaptureMode::FullScreen => self.capture_full_screen(None, cx),
        }
    }

    /// Opens the Region or Window overlay (DESIGN.md §6); its image arrives in `add_capture`.
    fn capture_with_overlay(&mut self, mode: CaptureMode, cx: &mut Context<Self>) {
        let this = cx.entity();
        let failed = this.clone();
        overlay::start(
            mode,
            cx,
            move |image, mode, cx| {
                this.update(cx, |this, cx| this.add_capture(image, mode, cx));
            },
            move |cx| failed.update(cx, |this, cx| this.report_region_failed(cx)),
        );
    }
}
