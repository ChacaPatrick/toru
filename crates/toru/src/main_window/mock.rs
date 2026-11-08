//! Mock gallery data until Phase 5 reads real captures. It is shaped as `toru_core`'s `Capture`,
//! so only the source changes then. The file sizes stand in for the ones the files will have.

use std::path::PathBuf;

use toru_core::{
    capture::{Capture, CaptureMode},
    time::{LocalDate, LocalDateTime},
};

use super::gallery::GroupKind;

/// The folder the mock data calls the save folder, until 5.6 asks the system for the real one.
pub const SAVE_FOLDER: &str = r"C:\Users\Pat\Pictures\Screenshots";

/// The day the mock data calls today.
pub const TODAY: LocalDate = LocalDate {
    year: 2026,
    month: 11,
    day: 8,
};

pub struct MockCapture {
    pub capture: Capture,
    pub bytes: u64,
}

/// The groups, newest first: this session, earlier today, then one per earlier day. Enough to
/// scroll, with saved and unsaved captures in each.
pub fn groups() -> Vec<(GroupKind, Vec<MockCapture>)> {
    use CaptureMode::*;
    let today = TODAY;
    let day = |days_ago| today.minus_days(days_ago);
    vec![
        (
            GroupKind::Session,
            vec![
                shot(
                    "screenshot_11_08_2026_4",
                    Region,
                    (1440, 900),
                    today,
                    14,
                    31,
                    false,
                ),
                shot("login-bug-repro", Window, (820, 610), today, 14, 22, true),
                shot(
                    "screenshot_11_08_2026_3",
                    FullScreen,
                    (2560, 1440),
                    today,
                    14,
                    15,
                    false,
                ),
            ],
        ),
        (
            GroupKind::EarlierToday,
            vec![
                shot("api-error", Window, (1180, 760), today, 11, 48, true),
                shot(
                    "screenshot_11_08_2026_2",
                    Region,
                    (960, 540),
                    today,
                    11,
                    12,
                    false,
                ),
                shot("standup-notes", Region, (1320, 820), today, 9, 40, true),
                shot(
                    "screenshot_11_08_2026",
                    FullScreen,
                    (2560, 1440),
                    today,
                    9,
                    5,
                    false,
                ),
            ],
        ),
        (
            GroupKind::Day(day(1)),
            vec![
                shot(
                    "screenshot_11_07_2026_5",
                    Region,
                    (1200, 700),
                    day(1),
                    17,
                    2,
                    false,
                ),
                shot(
                    "release-notes-draft",
                    Window,
                    (1024, 768),
                    day(1),
                    16,
                    20,
                    true,
                ),
                shot(
                    "screenshot_11_07_2026_4",
                    Region,
                    (640, 420),
                    day(1),
                    15,
                    55,
                    false,
                ),
                shot("dashboard", FullScreen, (2560, 1440), day(1), 14, 10, true),
                shot("invoice-march", Window, (900, 1200), day(1), 10, 30, true),
            ],
        ),
        (
            GroupKind::Day(day(2)),
            vec![
                shot(
                    "screenshot_11_06_2026_2",
                    Region,
                    (1200, 700),
                    day(2),
                    16,
                    48,
                    false,
                ),
                shot(
                    "screenshot_11_06_2026",
                    Window,
                    (640, 420),
                    day(2),
                    16,
                    40,
                    false,
                ),
            ],
        ),
        (
            GroupKind::Day(day(3)),
            vec![
                shot("map-route", Region, (1500, 900), day(3), 18, 5, true),
                shot(
                    "screenshot_11_05_2026_5",
                    Window,
                    (720, 480),
                    day(3),
                    17,
                    30,
                    false,
                ),
                shot(
                    "screenshot_11_05_2026_4",
                    Region,
                    (1100, 640),
                    day(3),
                    16,
                    12,
                    false,
                ),
                shot("recipe", Region, (800, 1100), day(3), 12, 44, true),
                shot(
                    "screenshot_11_05_2026_2",
                    FullScreen,
                    (2560, 1440),
                    day(3),
                    11,
                    2,
                    false,
                ),
                shot(
                    "screenshot_11_05_2026",
                    Region,
                    (980, 560),
                    day(3),
                    9,
                    15,
                    false,
                ),
            ],
        ),
        (
            GroupKind::Day(day(5)),
            vec![
                shot("team-photo", Window, (1600, 1000), day(5), 15, 25, true),
                shot(
                    "screenshot_11_03_2026_3",
                    Region,
                    (1280, 720),
                    day(5),
                    14,
                    0,
                    false,
                ),
                shot(
                    "screenshot_11_03_2026_2",
                    Region,
                    (560, 380),
                    day(5),
                    13,
                    48,
                    false,
                ),
                shot("error-dialog", Window, (480, 320), day(5), 10, 5, true),
            ],
        ),
        (
            GroupKind::Day(day(9)),
            vec![
                shot("chart-q3", Region, (1400, 860), day(9), 17, 40, true),
                shot(
                    "screenshot_10_30_2026_6",
                    Window,
                    (1024, 700),
                    day(9),
                    16,
                    15,
                    false,
                ),
                shot(
                    "screenshot_10_30_2026_5",
                    Region,
                    (900, 600),
                    day(9),
                    15,
                    52,
                    false,
                ),
                shot("ticket-4821", Window, (1180, 760), day(9), 14, 30, true),
                shot(
                    "screenshot_10_30_2026_3",
                    FullScreen,
                    (2560, 1440),
                    day(9),
                    12,
                    5,
                    false,
                ),
                shot(
                    "screenshot_10_30_2026_2",
                    Region,
                    (700, 500),
                    day(9),
                    10,
                    40,
                    false,
                ),
                shot("onboarding-flow", Window, (1440, 900), day(9), 9, 20, true),
            ],
        ),
    ]
}

fn shot(
    name: &str,
    mode: CaptureMode,
    (width, height): (u32, u32),
    date: LocalDate,
    hour: u8,
    minute: u8,
    saved: bool,
) -> MockCapture {
    MockCapture {
        capture: Capture {
            name: name.to_string(),
            mode,
            width,
            height,
            taken_at: LocalDateTime { date, hour, minute },
            saved_path: saved.then(|| PathBuf::from(format!(r"{SAVE_FOLDER}\{name}.png"))),
        },
        // Roughly a PNG's weight: about 1.1 bytes per pixel.
        bytes: u64::from(width) * u64::from(height) * 11 / 10,
    }
}
