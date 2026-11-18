use super::queries::date_number;
use super::{CaptureId, Result, Store};
use crate::time::LocalDate;

impl Store {
    /// The unsaved captures `delete_unsaved_older_than` would delete right now, so that a change
    /// confirmed later can delete exactly these with `delete_unsaved`.
    pub fn unsaved_older_than(&self, days: u32, today: LocalDate) -> Result<Vec<CaptureId>> {
        let db = self.db();
        let mut statement =
            db.prepare("SELECT id FROM captures WHERE taken_date < ?1 AND saved_path IS NULL")?;
        let ids = statement
            .query_map([date_number(today.minus_days(days))], |row| {
                row.get(0).map(CaptureId)
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// Deletes those of `ids` that are still unsaved, and their files. Returns the ids of the
    /// captures that went.
    pub fn delete_unsaved(&self, ids: &[CaptureId]) -> Result<Vec<CaptureId>> {
        let mut deleted = Vec::with_capacity(ids.len());
        for &id in ids {
            deleted.extend(self.delete_rows(
                "DELETE FROM captures WHERE id = ?1 AND saved_path IS NULL RETURNING id",
                id.0,
            )?);
        }
        Ok(deleted)
    }

    /// Deletes unsaved captures taken more than `days` days before `today`, and their files.
    /// Returns the ids of the captures that went.
    pub fn delete_unsaved_older_than(&self, days: u32, today: LocalDate) -> Result<Vec<CaptureId>> {
        self.delete_rows(
            "DELETE FROM captures WHERE taken_date < ?1 AND saved_path IS NULL RETURNING id",
            date_number(today.minus_days(days)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::store::test_support::*;

    #[test]
    fn retention_keeps_captures_exactly_n_days_old() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let today = date(2026, 1, 10);
        add(&store, session, "today", today, (9, 0));
        let boundary = add(&store, session, "30 days", date(2025, 12, 11), (0, 0));
        let expired = add(&store, session, "31 days", date(2025, 12, 10), (23, 59));
        let long_ago = add(&store, session, "long ago", date(2024, 6, 1), (9, 0));

        let mut counted = store.unsaved_older_than(30, today).unwrap();
        counted.sort_by_key(|id| id.0);
        assert_eq!(counted, [expired, long_ago]);
        let mut deleted = store.delete_unsaved_older_than(30, today).unwrap();

        deleted.sort_by_key(|id| id.0);
        assert_eq!(deleted, [expired, long_ago]);
        assert!(store.unsaved_older_than(30, today).unwrap().is_empty());
        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["today", "30 days"]
        );
        assert!(store.image_path(boundary).is_file());
        assert!(!store.image_path(expired).exists());
        assert!(!store.thumbnail_path(expired).exists());
    }

    #[test]
    fn the_count_leaves_out_saved_captures() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "old saved", date(2020, 1, 1), (9, 0));
        saved_copy(&store, id, &dir, "old.png");
        let unsaved = add(&store, session, "old unsaved", date(2020, 1, 2), (9, 0));
        assert_eq!(store.unsaved_older_than(30, TODAY).unwrap(), [unsaved]);
    }

    #[test]
    fn deleting_by_id_skips_captures_saved_since_they_were_counted() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let first = add(&store, session, "first", date(2020, 1, 1), (9, 0));
        let second = add(&store, session, "second", date(2020, 1, 2), (9, 0));
        let newer = add(&store, session, "newer", TODAY, (9, 0));
        let counted = store.unsaved_older_than(30, TODAY).unwrap();
        saved_copy(&store, second, &dir, "second.png");

        assert_eq!(store.delete_unsaved(&counted).unwrap(), [first]);
        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["newer", "second"]
        );
        assert!(!store.image_path(first).exists());
        assert!(store.image_path(newer).is_file());
        assert!(store.delete_unsaved(&[]).unwrap().is_empty());
    }

    #[test]
    fn retention_of_zero_days_keeps_only_today() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "today", TODAY, (0, 0));
        add(&store, session, "yesterday", date(2026, 11, 7), (23, 59));
        store.delete_unsaved_older_than(0, TODAY).unwrap();
        assert_eq!(names(&store.session_captures(session).unwrap()), ["today"]);
    }

    #[test]
    fn retention_measures_across_a_leap_day() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "leap day", date(2024, 2, 29), (9, 0));
        add(&store, session, "day before", date(2024, 2, 28), (9, 0));
        store
            .delete_unsaved_older_than(1, date(2024, 3, 1))
            .unwrap();
        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["leap day"]
        );
    }

    #[test]
    fn retention_never_touches_saved_captures_or_user_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "old saved", date(2020, 1, 1), (9, 0));
        let path = saved_copy(&store, id, &dir, "old.png");

        store.delete_unsaved_older_than(30, TODAY).unwrap();

        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["old saved"]
        );
        assert!(store.thumbnail_path(id).is_file());
        assert_eq!(fs::read(path).unwrap(), b"the user's file");
    }

    #[test]
    fn retention_tolerates_missing_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "old", date(2020, 1, 1), (9, 0));
        fs::remove_file(store.image_path(id)).unwrap();
        fs::remove_file(store.thumbnail_path(id)).unwrap();
        store.delete_unsaved_older_than(30, TODAY).unwrap();
        assert!(store.session_captures(session).unwrap().is_empty());
    }

    #[test]
    fn a_huge_retention_count_deletes_nothing_and_does_not_panic() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "old", date(1999, 1, 1), (9, 0));
        store.delete_unsaved_older_than(u32::MAX, TODAY).unwrap();
        assert_eq!(names(&store.session_captures(session).unwrap()), ["old"]);
    }
}
