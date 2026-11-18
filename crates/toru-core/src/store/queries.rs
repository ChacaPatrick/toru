use std::path::PathBuf;

use rusqlite::types::Type;
use rusqlite::{Params, Row};

use super::{CaptureId, Result, SessionId, Store, StoreError, StoredCapture};
use crate::capture::{Capture, CaptureMode};
use crate::time::{LocalDate, LocalDateTime};

impl Store {
    pub fn capture(&self, id: CaptureId) -> Result<Capture> {
        let found = self.select("id = ?1", [id.0])?.pop();
        found
            .map(|stored| stored.capture)
            .ok_or(StoreError::NotFound)
    }

    /// The session's captures, newest first.
    pub fn session_captures(&self, session: SessionId) -> Result<Vec<StoredCapture>> {
        self.select("session = ?1", [session.0])
    }

    /// Today's captures from every session but `current`, newest first.
    pub fn earlier_today(
        &self,
        current: SessionId,
        today: LocalDate,
    ) -> Result<Vec<StoredCapture>> {
        self.select(
            "taken_date = ?1 AND session <> ?2",
            [date_number(today), current.0],
        )
    }

    /// The most recent day with captures before `date`, if there is one.
    pub fn latest_day_before(&self, date: LocalDate) -> Result<Option<LocalDate>> {
        let day: Option<i64> = self.db().query_row(
            "SELECT MAX(taken_date) FROM captures WHERE taken_date < ?1",
            [date_number(date)],
            |row| row.get(0),
        )?;
        Ok(day.map(date_from_number))
    }

    /// The captures taken on `date`, newest first.
    pub fn captures_on(&self, date: LocalDate) -> Result<Vec<StoredCapture>> {
        self.select("taken_date = ?1", [date_number(date)])
    }

    fn select(&self, filter: &'static str, params: impl Params) -> Result<Vec<StoredCapture>> {
        let db = self.db();
        let mut statement = db.prepare(&format!(
            "SELECT id, name, mode, width, height, taken_date, taken_minute, saved_path
                FROM captures WHERE {filter}
                ORDER BY taken_date DESC, taken_minute DESC, id DESC"
        ))?;
        let captures = statement
            .query_map(params, stored_capture)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(captures)
    }
}

pub(super) fn date_number(date: LocalDate) -> i64 {
    i64::from(date.year) * 10_000 + i64::from(date.month) * 100 + i64::from(date.day)
}

fn date_from_number(number: i64) -> LocalDate {
    LocalDate {
        year: (number / 10_000) as u16,
        month: (number / 100 % 100) as u8,
        day: (number % 100) as u8,
    }
}

pub(crate) fn mode_text(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Region => "region",
        CaptureMode::Window => "window",
        CaptureMode::FullScreen => "full_screen",
    }
}

pub(crate) fn mode_from_text(text: &str) -> Option<CaptureMode> {
    match text {
        "region" => Some(CaptureMode::Region),
        "window" => Some(CaptureMode::Window),
        "full_screen" => Some(CaptureMode::FullScreen),
        _ => None,
    }
}

fn stored_capture(row: &Row) -> rusqlite::Result<StoredCapture> {
    let text = row.get_ref(2)?.as_str()?;
    let Some(mode) = mode_from_text(text) else {
        let message = format!("unknown capture mode {text:?}");
        return Err(rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            message.into(),
        ));
    };
    let minutes: i64 = row.get(6)?;
    let saved_path: Option<String> = row.get(7)?;
    Ok(StoredCapture {
        id: CaptureId(row.get(0)?),
        capture: Capture {
            name: row.get(1)?,
            mode,
            width: row.get(3)?,
            height: row.get(4)?,
            taken_at: LocalDateTime {
                date: date_from_number(row.get(5)?),
                hour: (minutes / 60) as u8,
                minute: (minutes % 60) as u8,
            },
            saved_path: saved_path.map(PathBuf::from),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;

    #[test]
    fn every_mode_round_trips() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        for (minute, mode) in [
            CaptureMode::Region,
            CaptureMode::Window,
            CaptureMode::FullScreen,
        ]
        .into_iter()
        .enumerate()
        {
            let shot = Capture {
                mode,
                ..record("shot", TODAY, (9, minute as u8))
            };
            store
                .add_capture(session, &pixels(4, 2), &shot, THUMB)
                .unwrap();
        }
        let modes: Vec<CaptureMode> = store
            .session_captures(session)
            .unwrap()
            .iter()
            .map(|c| c.capture.mode)
            .collect();
        assert_eq!(
            modes,
            [
                CaptureMode::FullScreen,
                CaptureMode::Window,
                CaptureMode::Region
            ]
        );
    }

    #[test]
    fn session_lists_only_its_own_captures_newest_first() {
        let dir = TestDir::new();
        let store = dir.open();
        let old = store.new_session().unwrap();
        let current = store.new_session().unwrap();
        add(&store, old, "other", TODAY, (8, 0));
        add(&store, current, "morning", TODAY, (9, 0));
        add(&store, current, "evening", TODAY, (18, 30));
        add(&store, current, "noon", TODAY, (12, 0));
        assert_eq!(
            names(&store.session_captures(current).unwrap()),
            ["evening", "noon", "morning"]
        );
    }

    #[test]
    fn captures_in_the_same_minute_list_the_latest_added_first() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "first", TODAY, (9, 0));
        add(&store, session, "second", TODAY, (9, 0));
        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["second", "first"]
        );
    }

    #[test]
    fn earlier_today_is_todays_captures_from_ended_sessions() {
        let dir = TestDir::new();
        let store = dir.open();
        let first = store.new_session().unwrap();
        let second = store.new_session().unwrap();
        let current = store.new_session().unwrap();
        add(&store, first, "yesterday", date(2026, 11, 7), (23, 0));
        add(&store, first, "early", TODAY, (8, 0));
        let saved = add(&store, second, "saved", TODAY, (10, 0));
        add(&store, second, "unsaved", TODAY, (9, 0));
        add(&store, current, "now", TODAY, (11, 0));
        saved_copy(&store, saved, &dir, "saved.png");

        assert_eq!(
            names(&store.earlier_today(current, TODAY).unwrap()),
            ["saved", "unsaved", "early"]
        );
    }

    #[test]
    fn day_paging_skips_gaps_and_crosses_months_and_years() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let days = [
            TODAY,
            date(2026, 11, 3),
            date(2026, 10, 31),
            date(2026, 3, 1),
            date(2026, 1, 1),
            date(2025, 12, 31),
            date(2024, 2, 29),
        ];
        for day in days {
            add(&store, session, "shot", day, (9, 0));
        }

        let mut walked = Vec::new();
        let mut cursor = TODAY;
        while let Some(day) = store.latest_day_before(cursor).unwrap() {
            walked.push(day);
            cursor = day;
        }
        assert_eq!(walked, days[1..]);
    }

    #[test]
    fn latest_day_before_is_none_without_earlier_captures() {
        let dir = TestDir::new();
        let store = dir.open();
        assert_eq!(store.latest_day_before(TODAY).unwrap(), None);
        let session = store.new_session().unwrap();
        add(&store, session, "shot", TODAY, (9, 0));
        assert_eq!(store.latest_day_before(TODAY).unwrap(), None);
    }

    #[test]
    fn a_days_captures_include_saved_and_unsaved_newest_first() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let day = date(2026, 11, 3);
        add(&store, session, "other day", date(2026, 11, 2), (9, 0));
        add(&store, session, "unsaved", day, (9, 0));
        let saved = add(&store, session, "saved", day, (15, 0));
        let path = saved_copy(&store, saved, &dir, "saved.png");

        let found = store.captures_on(day).unwrap();
        assert_eq!(names(&found), ["saved", "unsaved"]);
        assert_eq!(found[0].capture.saved_path, Some(path));
        assert_eq!(found[1].capture.saved_path, None);
    }

    #[test]
    fn unknown_ids_are_reported() {
        let dir = TestDir::new();
        let store = dir.open();
        let unknown = CaptureId(42);
        assert!(matches!(
            store.rename(unknown, "x"),
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            store.mark_saved(unknown, &dir.user_file("x.png")),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn an_unknown_mode_is_an_error() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "shot", TODAY, (9, 0));
        store
            .db()
            .execute("UPDATE captures SET mode = 'hologram'", [])
            .unwrap();
        assert!(matches!(
            store.session_captures(session),
            Err(StoreError::Database(_))
        ));
    }
}
