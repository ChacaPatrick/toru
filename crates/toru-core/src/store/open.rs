use std::collections::HashMap;
use std::fs::{self, File, TryLockError};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use rusqlite::Connection;

use super::copy_folders::{latest_copy_folder, remove_copy_folders_except};
use super::edits::ORIGINAL_REMOVED;
use super::files::PARTIAL_SUFFIX;
use super::{CaptureId, Result, Store, StoreError};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY AUTOINCREMENT);
    CREATE TABLE IF NOT EXISTS captures (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session INTEGER NOT NULL,
        name TEXT NOT NULL,
        mode TEXT NOT NULL,
        width INTEGER NOT NULL,
        height INTEGER NOT NULL,
        -- YYYYMMDD. The range keeps every value convertible to a LocalDate and back, which day paging relies on.
        taken_date INTEGER NOT NULL CHECK (taken_date BETWEEN 0 AND 655351231),
        taken_minute INTEGER NOT NULL,
        saved_path TEXT,
        -- 1 if the picture hides some of the capture under blur, pixelate or solid boxes, and 2
        -- once such a picture has been saved, from when the original must be gone for good.
        redacted INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS captures_by_date ON captures (taken_date);
    CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

impl Store {
    /// Deletes files a crash left without a row. Fails with `StoreError::AlreadyOpen` while another
    /// `Store` has the folder open.
    pub fn open(app_data: &Path) -> Result<Store> {
        let temp_dir = app_data.join("temp");
        fs::create_dir_all(&temp_dir)?;
        let instance_lock = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(app_data.join("store.lock"))?;
        instance_lock.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => StoreError::AlreadyOpen,
            TryLockError::Error(error) => error.into(),
        })?;
        let db = Connection::open(app_data.join("history.sqlite3"))?;
        db.execute_batch(SCHEMA)?;
        add_redacted_column(&db)?;
        let copies_dir = app_data.join("copies");
        let latest_copy = latest_copy_folder(&copies_dir);
        // The newest set may still be on the clipboard, waiting to be pasted.
        if let Some(number) = latest_copy {
            remove_copy_folders_except(&copies_dir, &copies_dir.join(number.to_string()));
        }
        let store = Store {
            db: Mutex::new(db),
            temp_dir,
            copies_dir,
            next_partial: AtomicU64::new(0),
            next_copy: AtomicU64::new(latest_copy.map_or(0, |number| number + 1)),
            thumbnail_lock: Mutex::new(()),
            _instance_lock: instance_lock,
        };
        store.remove_stray_files()?;
        Ok(store)
    }

    fn remove_stray_files(&self) -> Result<()> {
        // Originals a save meant to delete, if deleting them failed then.
        let removed: Vec<i64> = {
            let db = self.db();
            let mut statement = db.prepare("SELECT id FROM captures WHERE redacted = ?1")?;
            statement
                .query_map([ORIGINAL_REMOVED], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in removed {
            let _ = fs::remove_file(self.original_path(CaptureId(id)));
        }
        let mut saved_by_id: HashMap<i64, bool> = {
            let db = self.db();
            // A saved picture that hides some of the capture keeps no original, but its temp
            // image may still hold edits awaiting Save.
            let mut statement =
                db.prepare("SELECT id, saved_path IS NOT NULL AND redacted = 0 FROM captures")?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        // The temp image of a saved capture with its original kept holds edits awaiting Save.
        for (&id, saved) in &mut saved_by_id {
            if *saved && self.original_path(CaptureId(id)).is_file() {
                *saved = false;
            }
        }
        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            if is_stray(&entry.file_name().to_string_lossy(), &saved_by_id) {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

/// Databases made before captures recorded redactions lack the column.
fn add_redacted_column(db: &Connection) -> rusqlite::Result<()> {
    let has: bool = db.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'redacted'",
        [],
        |row| row.get(0),
    )?;
    if !has {
        db.execute_batch("ALTER TABLE captures ADD COLUMN redacted INTEGER NOT NULL DEFAULT 0")?;
    }
    Ok(())
}

fn is_stray(file_name: &str, saved_by_id: &HashMap<i64, bool>) -> bool {
    let parse = |id: &str| id.parse::<i64>().ok();
    if file_name.ends_with(PARTIAL_SUFFIX) {
        true
    } else if let Some(id) = file_name.strip_suffix(".thumb.png").and_then(parse) {
        !saved_by_id.contains_key(&id)
    } else if let Some(id) = file_name.strip_suffix(".original.png").and_then(parse) {
        !saved_by_id.contains_key(&id)
    } else if let Some(id) = file_name.strip_suffix(".png").and_then(parse) {
        saved_by_id.get(&id) != Some(&false)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::store::Redaction;
    use crate::store::test_support::*;

    #[test]
    fn open_creates_the_database_and_temp_folder() {
        let dir = TestDir::new();
        let _store = dir.open();
        assert!(dir.app_data().join("history.sqlite3").is_file());
        assert!(dir.app_data().join("temp").is_dir());
    }

    #[test]
    fn a_second_open_is_refused_and_works_once_the_first_store_is_dropped() {
        let dir = TestDir::new();
        let store = dir.open();
        let in_progress = dir.app_data().join("temp").join("0.png.partial");
        fs::write(&in_progress, b"being written").unwrap();

        let second = Store::open(&dir.app_data());
        assert!(matches!(second, Err(StoreError::AlreadyOpen)));
        assert!(in_progress.is_file(), "a refused open must not sweep");

        drop(store);
        dir.open();
    }

    #[test]
    fn captures_survive_reopening() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 5));
        drop(store);

        let store = dir.open();
        let found = store.session_captures(session).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id);
        assert_eq!(found[0].capture, record("shot", TODAY, (9, 5)));
        assert!(store.image_path(id).is_file());
        assert!(store.thumbnail_path(id).is_file());
    }

    #[test]
    fn session_ids_are_never_reused() {
        let dir = TestDir::new();
        let store = dir.open();
        let first = store.new_session().unwrap();
        let second = store.new_session().unwrap();
        assert_ne!(first, second);
        drop(store);

        let store = dir.open();
        let third = store.new_session().unwrap();
        assert_ne!(third, first);
        assert_ne!(third, second);
    }

    #[test]
    fn a_database_from_before_redactions_gains_the_column() {
        let dir = TestDir::new();
        fs::create_dir_all(dir.app_data()).unwrap();
        let start = SCHEMA.find("saved_path TEXT,").unwrap();
        let end = SCHEMA.find("redacted INTEGER NOT NULL DEFAULT 0").unwrap();
        let old_schema = format!(
            "{}saved_path TEXT{}",
            &SCHEMA[..start],
            &SCHEMA[end + "redacted INTEGER NOT NULL DEFAULT 0".len()..]
        );
        assert_ne!(old_schema, SCHEMA);
        Connection::open(dir.app_data().join("history.sqlite3"))
            .unwrap()
            .execute_batch(&old_schema)
            .unwrap();

        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));

        assert_eq!(store.redaction(id).unwrap(), Redaction::default());
        drop(store);
        dir.open();
    }

    #[test]
    fn the_schema_refuses_dates_outside_the_convertible_range() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let update = "UPDATE captures SET taken_date = ?1 WHERE id = ?2";
        assert!(store.db().execute(update, [-1, id.0]).is_err());
        assert!(store.db().execute(update, [655_351_232, id.0]).is_err());
        assert!(store.db().execute(update, params!["x", id.0]).is_err());
    }

    #[test]
    fn open_sweeps_files_a_crash_left_behind() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let unsaved = add(&store, session, "unsaved", TODAY, (9, 0));
        let saved = add(&store, session, "saved", TODAY, (10, 0));
        saved_copy(&store, saved, &dir, "saved.png");
        drop(store);

        let temp = dir.app_data().join("temp");
        let no_row = [
            "999.png".to_string(),
            "999.thumb.png".to_string(),
            format!("{}.png", saved.0),
            "7.png.partial".to_string(),
            "7.thumb.png.partial".to_string(),
        ];
        for name in &no_row {
            fs::write(temp.join(name), b"stray").unwrap();
        }
        fs::write(temp.join("notes.txt"), b"not ours").unwrap();
        fs::write(temp.join("holiday.png"), b"not ours").unwrap();

        let store = dir.open();
        let mut expected = vec![
            "holiday.png".to_string(),
            "notes.txt".to_string(),
            format!("{}.png", unsaved.0),
            format!("{}.thumb.png", unsaved.0),
            format!("{}.thumb.png", saved.0),
        ];
        expected.sort();
        assert_eq!(dir.temp_files(), expected);
        assert_eq!(store.session_captures(session).unwrap().len(), 2);
    }
}
