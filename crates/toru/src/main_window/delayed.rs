//! Delayed capture: a countdown on the toolbar's Delayed button, then the capture it was started
//! for. Toru minimizes for the countdown so the user can arrange their screen, and the time left
//! follows in the window title, which is what the taskbar shows. Bringing Toru back shows the
//! button counting, and pressing it cancels. Toru isn't brought back afterwards, which would take
//! the foreground from what was just arranged: the capture lands in the gallery as any capture
//! taken while minimized does (DESIGN.md §3), and if it fails, the message waits for Toru's return.

use std::time::Duration;

use gpui::{Context, Task};
use toru_core::capture::CaptureMode;

use super::{MainWindow, WINDOW_TITLE};

const TICK: Duration = Duration::from_secs(1);

/// How far a delayed capture has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Countdown {
    seconds_left: u32,
    mode: CaptureMode,
}

impl Countdown {
    /// One second on, or `None` if that was the last.
    fn tick(self) -> Option<Countdown> {
        (self.seconds_left > 1).then(|| Countdown {
            seconds_left: self.seconds_left - 1,
            ..self
        })
    }

    fn button_sub(self) -> String {
        format!("{} s", self.seconds_left)
    }

    fn window_title(self) -> String {
        format!("Capture in {} s — {WINDOW_TITLE}", self.seconds_left)
    }
}

/// A countdown under way. The view owns it, so a second one replaces the first, and closing the
/// window drops it before it can fire.
pub(super) struct RunningCountdown {
    countdown: Countdown,
    _ticks: Task<()>,
}

impl MainWindow {
    /// Starts the countdown the Delayed menu describes.
    pub(super) fn start_countdown(&mut self, cx: &mut Context<Self>) {
        let countdown = Countdown {
            seconds_left: self.settings.delay_seconds,
            mode: self.settings.delay_mode,
        };
        self.countdown = Some(RunningCountdown {
            countdown,
            _ticks: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let counting = this.update(cx, |this, cx| this.tick_countdown(cx));
                    if !counting.unwrap_or(false) {
                        break;
                    }
                }
            }),
        });
        self.on_window(cx, move |window| {
            window.set_window_title(&countdown.window_title());
            window.minimize_window();
        });
        cx.notify();
    }

    /// One second on: shows the time left or, when it has run out, takes the capture. Returns
    /// whether it is still counting.
    fn tick_countdown(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(running) = &mut self.countdown else {
            return false;
        };
        let Some(next) = running.countdown.tick() else {
            let mode = running.countdown.mode;
            self.capture(mode, cx);
            return false;
        };
        running.countdown = next;
        self.on_window(cx, move |window| {
            window.set_window_title(&next.window_title())
        });
        cx.notify();
        true
    }

    pub(super) fn cancel_countdown(&mut self, cx: &mut Context<Self>) {
        if self.countdown.take().is_some() {
            self.on_window(cx, |window| window.set_window_title(WINDOW_TITLE));
            cx.notify();
        }
    }

    /// The time left for the Delayed button, while a countdown runs.
    pub(super) fn countdown_sub(&self) -> Option<String> {
        self.countdown
            .as_ref()
            .map(|running| running.countdown.button_sub())
    }
}

#[cfg(test)]
mod tests {
    use toru_core::settings::DELAY_SECONDS;

    use super::*;

    fn countdown(seconds_left: u32) -> Countdown {
        Countdown {
            seconds_left,
            mode: CaptureMode::Window,
        }
    }

    #[test]
    fn it_counts_every_second_down_to_one_and_then_ends() {
        for seconds in DELAY_SECONDS {
            let mut shown = vec![seconds];
            let mut current = countdown(seconds);
            while let Some(next) = current.tick() {
                shown.push(next.seconds_left);
                current = next;
            }
            let expected: Vec<u32> = (1..=seconds).rev().collect();
            assert_eq!(shown, expected);
        }
    }

    #[test]
    fn the_chosen_mode_survives_the_countdown() {
        assert_eq!(countdown(5).tick().unwrap().mode, CaptureMode::Window);
    }

    #[test]
    fn a_countdown_with_no_time_left_ends_at_once() {
        assert_eq!(countdown(1).tick(), None);
        assert_eq!(countdown(0).tick(), None);
    }

    #[test]
    fn the_button_and_the_title_show_the_seconds_left() {
        assert_eq!(countdown(3).button_sub(), "3 s");
        assert_eq!(countdown(3).window_title(), "Capture in 3 s — Toru");
    }
}
