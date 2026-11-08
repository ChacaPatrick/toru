//! The capture record and its thumbnail caption.

use std::path::PathBuf;

use crate::time::{LocalDate, LocalDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Region,
    Window,
    FullScreen,
}

impl CaptureMode {
    pub fn label(self) -> &'static str {
        match self {
            CaptureMode::Region => "Region",
            CaptureMode::Window => "Window",
            CaptureMode::FullScreen => "Full screen",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub mode: CaptureMode,
    pub width: u32,
    pub height: u32,
    pub taken_at: LocalDateTime,
    pub saved_path: Option<PathBuf>,
}

impl Capture {
    pub fn is_saved(&self) -> bool {
        self.saved_path.is_some()
    }

    /// The thumbnail caption. The date only appears for a capture from a day other than `today`.
    pub fn caption(&self, today: LocalDate) -> String {
        let LocalDateTime { date, hour, minute } = self.taken_at;
        let period = if hour < 12 { "AM" } else { "PM" };
        let hour = if hour % 12 == 0 { 12 } else { hour % 12 };
        let time = format!("{hour}:{minute:02} {period}");
        let when = if date == today {
            time
        } else {
            format!("{} {}, {time}", date.month_short(), date.day)
        };
        let mode = self.mode.label();
        format!("{}×{} · {mode} · {when}", self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: LocalDate = LocalDate {
        year: 2026,
        month: 11,
        day: 7,
    };

    fn capture(
        mode: CaptureMode,
        (width, height): (u32, u32),
        date: LocalDate,
        (hour, minute): (u8, u8),
    ) -> Capture {
        Capture {
            name: "shot".to_string(),
            mode,
            width,
            height,
            taken_at: LocalDateTime { date, hour, minute },
            saved_path: None,
        }
    }

    fn caption_at(hour: u8, minute: u8) -> String {
        capture(CaptureMode::Region, (10, 20), TODAY, (hour, minute)).caption(TODAY)
    }

    #[test]
    fn caption_for_today_has_only_the_time() {
        let shot = capture(CaptureMode::Region, (1440, 900), TODAY, (14, 31));
        assert_eq!(shot.caption(TODAY), "1440×900 · Region · 2:31 PM");
    }

    #[test]
    fn caption_uses_each_mode_label() {
        let window = capture(CaptureMode::Window, (820, 610), TODAY, (14, 22));
        assert_eq!(window.caption(TODAY), "820×610 · Window · 2:22 PM");
        let full = capture(CaptureMode::FullScreen, (2560, 1440), TODAY, (14, 15));
        assert_eq!(full.caption(TODAY), "2560×1440 · Full screen · 2:15 PM");
    }

    #[test]
    fn caption_for_an_earlier_day_has_the_date() {
        let nov_6 = LocalDate { day: 6, ..TODAY };
        let shot = capture(CaptureMode::Region, (1200, 700), nov_6, (16, 48));
        assert_eq!(shot.caption(TODAY), "1200×700 · Region · Nov 6, 4:48 PM");
    }

    #[test]
    fn caption_shows_the_date_when_only_the_month_or_year_differs() {
        let january = LocalDate { month: 1, ..TODAY };
        let shot = capture(CaptureMode::Window, (640, 420), january, (9, 2));
        assert_eq!(shot.caption(TODAY), "640×420 · Window · Jan 7, 9:02 AM");

        let last_year = LocalDate {
            year: 2025,
            ..TODAY
        };
        let shot = capture(CaptureMode::Window, (640, 420), last_year, (9, 2));
        assert_eq!(shot.caption(TODAY), "640×420 · Window · Nov 7, 9:02 AM");
    }

    #[test]
    fn caption_uses_a_twelve_hour_clock() {
        assert_eq!(caption_at(0, 5), "10×20 · Region · 12:05 AM");
        assert_eq!(caption_at(9, 0), "10×20 · Region · 9:00 AM");
        assert_eq!(caption_at(11, 59), "10×20 · Region · 11:59 AM");
        assert_eq!(caption_at(12, 0), "10×20 · Region · 12:00 PM");
        assert_eq!(caption_at(13, 7), "10×20 · Region · 1:07 PM");
        assert_eq!(caption_at(23, 59), "10×20 · Region · 11:59 PM");
    }

    #[test]
    fn caption_copes_with_an_impossible_month() {
        for month in [0, 13, 255] {
            let bad = LocalDate { month, ..TODAY };
            let shot = capture(CaptureMode::Region, (10, 20), bad, (9, 0));
            assert_eq!(shot.caption(TODAY), "10×20 · Region · ??? 7, 9:00 AM");
        }
    }

    #[test]
    fn a_capture_is_saved_once_it_has_a_saved_path() {
        let mut shot = capture(CaptureMode::Region, (10, 20), TODAY, (9, 0));
        assert!(!shot.is_saved());
        shot.saved_path = Some(PathBuf::from(r"C:\Pictures\shot.png"));
        assert!(shot.is_saved());
    }
}
