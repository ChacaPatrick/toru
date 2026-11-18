use rusqlite::{OptionalExtension, params};

use super::queries::date_number;
use super::{CaptureId, Result, Store, StoreError};
use crate::naming::{fold_case, validate_name};
use crate::time::LocalDate;

impl Store {
    /// The names of the captures taken on `date`, for `next_free_name`.
    pub fn names_on(&self, date: LocalDate) -> Result<Vec<String>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT name FROM captures WHERE taken_date = ?1")?;
        let names = statement
            .query_map([date_number(date)], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(names)
    }

    /// Changes the capture's name in the record, if it can be a file name and no other capture
    /// from that day has it. A saved file keeps its own name: it is the user's, and may have been
    /// moved or be in use.
    pub fn rename(&self, id: CaptureId, name: &str) -> Result<()> {
        validate_name(name)?;
        // One lock around the check and the change, so two renames can't both take a name.
        let db = self.db();
        let date: i64 = db
            .query_row(
                "SELECT taken_date FROM captures WHERE id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let wanted = fold_case(name);
        let mut others =
            db.prepare("SELECT name FROM captures WHERE taken_date = ?1 AND id <> ?2")?;
        for other in others.query_map([date, id.0], |row| row.get::<_, String>(0))? {
            if fold_case(&other?) == wanted {
                return Err(StoreError::NameTaken);
            }
        }
        db.execute(
            "UPDATE captures SET name = ?1 WHERE id = ?2",
            params![name, id.0],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::naming::{DEFAULT_PATTERN, next_free_name};
    use crate::store::test_support::*;

    #[test]
    fn names_on_a_date_feed_next_free_name() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "screenshot_11_08_2026", TODAY, (9, 0));
        add(&store, session, "other", date(2026, 11, 7), (9, 0));

        let taken = store.names_on(TODAY).unwrap();
        assert_eq!(taken, ["screenshot_11_08_2026"]);
        let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
        assert_eq!(
            next_free_name(DEFAULT_PATTERN, TODAY, &taken),
            "screenshot_11_08_2026_2"
        );
    }

    #[test]
    fn rename_changes_only_that_capture() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "keep", TODAY, (9, 0));
        let id = add(&store, session, "old", TODAY, (10, 0));
        store.rename(id, "new name").unwrap();
        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["new name", "keep"]
        );
    }

    #[test]
    fn rename_refuses_a_name_another_capture_that_day_has() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "Taken", TODAY, (9, 0));
        let id = add(&store, session, "mine", TODAY, (10, 0));

        for name in ["Taken", "TAKEN", "taken"] {
            assert!(matches!(store.rename(id, name), Err(StoreError::NameTaken)));
        }
        assert_eq!(store.capture(id).unwrap().name, "mine");
    }

    #[test]
    fn rename_allows_a_name_used_on_another_day_and_its_own_name_in_a_new_case() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        add(&store, session, "shot", date(2026, 11, 7), (9, 0));
        let id = add(&store, session, "mine", TODAY, (10, 0));

        store.rename(id, "shot").unwrap();
        store.rename(id, "SHOT").unwrap();
        assert_eq!(store.capture(id).unwrap().name, "SHOT");
    }

    #[test]
    fn rename_refuses_what_cannot_be_a_file_name() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "mine", TODAY, (10, 0));

        for name in ["", "a:b", "ends with a dot.", "con"] {
            assert!(matches!(
                store.rename(id, name),
                Err(StoreError::InvalidName(_))
            ));
        }
        assert_eq!(store.capture(id).unwrap().name, "mine");
    }

    #[test]
    fn renaming_a_saved_capture_leaves_its_file_alone() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "old", TODAY, (10, 0));
        let path = saved_copy(&store, id, &dir, "old.png");

        store.rename(id, "new").unwrap();

        let capture = store.capture(id).unwrap();
        assert_eq!(capture.name, "new");
        assert_eq!(capture.saved_path, Some(path.clone()));
        assert_eq!(fs::read(path).unwrap(), b"the user's file");
    }
}
