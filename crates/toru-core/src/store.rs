//! Captures on disk: image files in a temp folder and their records in SQLite.
//!
//! A row only ever exists with its files complete. Adding writes the files under temporary names,
//! then renames them to their final names inside the row's transaction and commits last; removing
//! deletes the row first and the files after. A crash can therefore only leave files without a row,
//! and `Store::open` deletes those. Because of that sweep, a `Store` holds an exclusive lock on a
//! file in the app-data folder for as long as it is open.

use std::collections::HashMap;
use std::fs::{self, File, TryLockError};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use image::{ImageFormat, RgbaImage, imageops};
use rusqlite::types::Type;
use rusqlite::{Connection, Params, Row, params};

use crate::capture::{Capture, CaptureMode};
use crate::settings::Settings;
use crate::time::{LocalDate, LocalDateTime};

/// Ends the name of a file that `add_capture` hasn't finished with yet.
const PARTIAL_SUFFIX: &str = ".partial";

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
        saved_path TEXT
    );
    CREATE INDEX IF NOT EXISTS captures_by_date ON captures (taken_date);
    CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Io(io::Error),
    Image(image::ImageError),
    /// Another `Store`, usually another running instance of the app, has this folder open.
    AlreadyOpen,
    /// No capture has that ID.
    NotFound,
    /// The pixels are empty or differ from the record's size, or the thumbnail width is zero.
    BadDimensions,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Database(error)
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        StoreError::Io(error)
    }
}

impl From<image::ImageError> for StoreError {
    fn from(error: image::ImageError) -> Self {
        StoreError::Image(error)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureId(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCapture {
    pub id: CaptureId,
    pub capture: Capture,
}

/// Safe to share between threads. Encoding and writing the files happen outside the database lock,
/// so a slow `add_capture` holds up queries from other threads only for its insert and renames.
pub struct Store {
    db: Mutex<Connection>,
    temp_dir: PathBuf,
    next_partial: AtomicU64,
    // Last, so the database has closed by the time another process can get in.
    _instance_lock: File,
}

impl Store {
    /// Opens or creates the store inside `app_data`, and deletes files a crash left without a row.
    /// Fails with `StoreError::AlreadyOpen` while another `Store` has the folder open.
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
        let store = Store {
            db: Mutex::new(db),
            temp_dir,
            next_partial: AtomicU64::new(0),
            _instance_lock: instance_lock,
        };
        store.remove_stray_files()?;
        Ok(store)
    }

    /// Starts a new session. Its ID is never reused, even across restarts.
    pub fn new_session(&self) -> Result<SessionId> {
        let db = self.db();
        db.execute("INSERT INTO sessions DEFAULT VALUES", [])?;
        Ok(SessionId(db.last_insert_rowid()))
    }

    /// Writes the image and a thumbnail no wider than `max_thumbnail_width` and records the capture.
    pub fn add_capture(
        &self,
        session: SessionId,
        pixels: &RgbaImage,
        capture: &Capture,
        max_thumbnail_width: u32,
    ) -> Result<CaptureId> {
        let (width, height) = pixels.dimensions();
        if width == 0
            || height == 0
            || max_thumbnail_width == 0
            || (width, height) != (capture.width, capture.height)
        {
            return Err(StoreError::BadDimensions);
        }
        let saved_path = capture.saved_path.as_deref().map(path_text).transpose()?;

        // A capture that arrives saved already has its full image in the user's folder.
        let image_png = (!capture.is_saved())
            .then(|| encode_png(pixels))
            .transpose()?;
        let thumbnail_width = width.min(max_thumbnail_width);
        let thumbnail_height =
            (u64::from(height) * u64::from(thumbnail_width) / u64::from(width)).max(1) as u32;
        let thumbnail_png = encode_png(&imageops::thumbnail(
            pixels,
            thumbnail_width,
            thumbnail_height,
        ))?;

        let number = self.next_partial.fetch_add(1, Ordering::Relaxed);
        let partial_image = self.temp_dir.join(format!("{number}.png{PARTIAL_SUFFIX}"));
        let partial_thumbnail = self
            .temp_dir
            .join(format!("{number}.thumb.png{PARTIAL_SUFFIX}"));
        let stored = (|| -> Result<CaptureId> {
            if let Some(png) = &image_png {
                write_file(&partial_image, png)?;
            }
            write_file(&partial_thumbnail, &thumbnail_png)?;
            self.insert_and_publish(
                session,
                capture,
                saved_path,
                image_png.is_some().then_some(partial_image.as_path()),
                &partial_thumbnail,
            )
        })();
        if stored.is_err() {
            let _ = fs::remove_file(&partial_image);
            let _ = fs::remove_file(&partial_thumbnail);
        }
        stored
    }

    /// The full image in the temp folder. It exists only until the capture is saved; after that
    /// the image is the file at `saved_path`.
    pub fn image_path(&self, id: CaptureId) -> PathBuf {
        self.temp_dir.join(format!("{}.png", id.0))
    }

    pub fn thumbnail_path(&self, id: CaptureId) -> PathBuf {
        self.temp_dir.join(format!("{}.thumb.png", id.0))
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

    /// The names of the captures taken on `date`, for `next_free_name`.
    pub fn names_on(&self, date: LocalDate) -> Result<Vec<String>> {
        let db = self.db();
        let mut statement = db.prepare("SELECT name FROM captures WHERE taken_date = ?1")?;
        let names = statement
            .query_map([date_number(date)], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(names)
    }

    /// Records that the capture was saved to `path`, a file that must already exist. The temp copy
    /// of the full image is then removed, so saved captures don't pile up in the temp folder
    /// forever. The thumbnail stays for the gallery.
    pub fn mark_saved(&self, id: CaptureId, path: &Path) -> Result<()> {
        fs::metadata(path)?;
        let changed = self.db().execute(
            "UPDATE captures SET saved_path = ?1 WHERE id = ?2",
            params![path_text(path)?, id.0],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        // If this fails, the next `open` tries again.
        let _ = fs::remove_file(self.image_path(id));
        Ok(())
    }

    /// Changes the capture's name in the record. A saved file keeps its own name.
    pub fn rename(&self, id: CaptureId, name: &str) -> Result<()> {
        let changed = self.db().execute(
            "UPDATE captures SET name = ?1 WHERE id = ?2",
            params![name, id.0],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Deletes the record and temp files. A file the user saved stays where it is.
    pub fn delete(&self, id: CaptureId) -> Result<()> {
        self.delete_rows("DELETE FROM captures WHERE id = ?1 RETURNING id", id.0)
    }

    /// Deletes the session's unsaved captures and their files.
    pub fn discard_unsaved(&self, session: SessionId) -> Result<()> {
        self.delete_rows(
            "DELETE FROM captures WHERE session = ?1 AND saved_path IS NULL RETURNING id",
            session.0,
        )
    }

    /// Deletes unsaved captures taken more than `days` days before `today`, and their files.
    pub fn delete_unsaved_older_than(&self, days: u32, today: LocalDate) -> Result<()> {
        self.delete_rows(
            "DELETE FROM captures WHERE taken_date < ?1 AND saved_path IS NULL RETURNING id",
            date_number(today.minus_days(days)),
        )
    }

    /// The saved settings. Anything missing or unreadable keeps its default.
    pub fn load_settings(&self) -> Result<Settings> {
        let db = self.db();
        let mut statement = db.prepare("SELECT key, value FROM settings")?;
        let mut rows = statement.query([])?;
        let mut settings = Settings::default();
        while let Some(row) = rows.next()? {
            // A value that isn't valid text is one more malformed value, not a reason to fail.
            if let (Ok(key), Ok(value)) = (row.get_ref(0)?.as_str(), row.get_ref(1)?.as_str()) {
                settings.apply_stored(key, value);
            }
        }
        Ok(settings)
    }

    /// Writes every setting, all or nothing.
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let values = settings.stored_values()?;
        let mut db = self.db();
        let transaction = db.transaction()?;
        for (key, value) in values {
            transaction.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    // A panic can't leave the connection mid-change, since an open transaction rolls back when
    // dropped, so a poisoned lock is safe to take.
    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn insert_and_publish(
        &self,
        session: SessionId,
        capture: &Capture,
        saved_path: Option<&str>,
        partial_image: Option<&Path>,
        partial_thumbnail: &Path,
    ) -> Result<CaptureId> {
        let mut db = self.db();
        let transaction = db.transaction()?;
        transaction.execute(
            "INSERT INTO captures
                (session, name, mode, width, height, taken_date, taken_minute, saved_path)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.0,
                capture.name,
                mode_text(capture.mode),
                capture.width,
                capture.height,
                date_number(capture.taken_at.date),
                i64::from(capture.taken_at.hour) * 60 + i64::from(capture.taken_at.minute),
                saved_path,
            ],
        )?;
        let id = CaptureId(transaction.last_insert_rowid());
        let published = (|| -> Result<()> {
            if let Some(partial) = partial_image {
                fs::rename(partial, self.image_path(id))?;
            }
            fs::rename(partial_thumbnail, self.thumbnail_path(id))?;
            transaction.commit()?;
            Ok(())
        })();
        // Still under the lock: a rolled-back ID goes to the next capture, and removing these any
        // later could delete that capture's files.
        if published.is_err() {
            self.remove_temp_files(id);
        }
        published.map(|()| id)
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

    // `sql` must be a single `DELETE ... RETURNING id`. Its rows are committed as gone before any
    // file is touched.
    fn delete_rows(&self, sql: &'static str, param: i64) -> Result<()> {
        let ids: Vec<i64> = {
            let db = self.db();
            let mut statement = db.prepare(sql)?;
            statement
                .query_map([param], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            self.remove_temp_files(CaptureId(id));
        }
        Ok(())
    }

    // Failures are ignored: the file may already be gone, and anything left is swept by `open`.
    fn remove_temp_files(&self, id: CaptureId) {
        let _ = fs::remove_file(self.image_path(id));
        let _ = fs::remove_file(self.thumbnail_path(id));
    }

    fn remove_stray_files(&self) -> Result<()> {
        let saved_by_id: HashMap<i64, bool> = {
            let db = self.db();
            let mut statement = db.prepare("SELECT id, saved_path IS NOT NULL FROM captures")?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            if is_stray(&entry.file_name().to_string_lossy(), &saved_by_id) {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

fn is_stray(file_name: &str, saved_by_id: &HashMap<i64, bool>) -> bool {
    let parse = |id: &str| id.parse::<i64>().ok();
    if file_name.ends_with(PARTIAL_SUFFIX) {
        true
    } else if let Some(id) = file_name.strip_suffix(".thumb.png").and_then(parse) {
        !saved_by_id.contains_key(&id)
    } else if let Some(id) = file_name.strip_suffix(".png").and_then(parse) {
        saved_by_id.get(&id) != Some(&false)
    } else {
        false
    }
}

fn encode_png(pixels: &RgbaImage) -> Result<Vec<u8>> {
    let mut png = Cursor::new(Vec::new());
    pixels.write_to(&mut png, ImageFormat::Png)?;
    Ok(png.into_inner())
}

// Synced so the commit that follows can't outlive the data it refers to, even on power loss.
fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

// Refused rather than stored lossily, which would leave the record pointing at another file.
pub(crate) fn path_text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path is not valid Unicode").into()
    })
}

fn date_number(date: LocalDate) -> i64 {
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
    use std::env;
    use std::process;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    use image::Rgba;

    use super::*;
    use crate::naming::{DEFAULT_PATTERN, next_free_name};
    use crate::settings::tests::every_field_changed;

    const TODAY: LocalDate = date(2026, 11, 8);
    const THUMB: u32 = 2;

    const fn date(year: u16, month: u8, day: u8) -> LocalDate {
        LocalDate { year, month, day }
    }

    /// A throwaway folder, removed on drop. Declare it before the store so the store closes first.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> TestDir {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let number = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!("toru-store-{}-{number}", process::id()));
            fs::create_dir_all(&path).unwrap();
            TestDir(path)
        }

        fn app_data(&self) -> PathBuf {
            self.0.join("app")
        }

        fn open(&self) -> Store {
            Store::open(&self.app_data()).unwrap()
        }

        /// Stands in for a file the user saved outside the app's folder.
        fn user_file(&self, name: &str) -> PathBuf {
            let folder = self.0.join("Pictures");
            fs::create_dir_all(&folder).unwrap();
            let path = folder.join(name);
            fs::write(&path, b"the user's file").unwrap();
            path
        }

        fn temp_files(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.app_data().join("temp"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn pixels(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]))
    }

    fn record(name: &str, taken: LocalDate, (hour, minute): (u8, u8)) -> Capture {
        Capture {
            name: name.to_string(),
            mode: CaptureMode::Region,
            width: 4,
            height: 2,
            taken_at: LocalDateTime {
                date: taken,
                hour,
                minute,
            },
            saved_path: None,
        }
    }

    fn add(
        store: &Store,
        session: SessionId,
        name: &str,
        taken: LocalDate,
        time: (u8, u8),
    ) -> CaptureId {
        store
            .add_capture(session, &pixels(4, 2), &record(name, taken, time), THUMB)
            .unwrap()
    }

    fn names(captures: &[StoredCapture]) -> Vec<&str> {
        captures.iter().map(|c| c.capture.name.as_str()).collect()
    }

    fn saved_copy(store: &Store, id: CaptureId, dir: &TestDir, name: &str) -> PathBuf {
        let path = dir.user_file(name);
        store.mark_saved(id, &path).unwrap();
        path
    }

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
    fn adding_writes_the_full_image_and_a_smaller_thumbnail() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let mut original = pixels(40, 20);
        original.put_pixel(3, 7, Rgba([200, 100, 50, 255]));
        let shot = Capture {
            width: 40,
            height: 20,
            ..record("shot", TODAY, (9, 0))
        };
        let id = store.add_capture(session, &original, &shot, 10).unwrap();

        let full = image::open(store.image_path(id)).unwrap().to_rgba8();
        assert_eq!(full, original);
        let thumbnail = image::open(store.thumbnail_path(id)).unwrap().to_rgba8();
        assert_eq!(thumbnail.dimensions(), (10, 5));
    }

    #[test]
    fn a_narrow_image_is_not_enlarged() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let shot = record("shot", TODAY, (9, 0));
        let id = store
            .add_capture(session, &pixels(4, 2), &shot, 1000)
            .unwrap();
        let thumbnail = image::open(store.thumbnail_path(id)).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (4, 2));
    }

    #[test]
    fn a_thin_image_keeps_a_thumbnail_at_least_one_pixel_high() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let shot = Capture {
            width: 1000,
            height: 1,
            ..record("shot", TODAY, (9, 0))
        };
        let id = store
            .add_capture(session, &pixels(1000, 1), &shot, 10)
            .unwrap();
        let thumbnail = image::open(store.thumbnail_path(id)).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (10, 1));
    }

    #[test]
    fn bad_dimensions_are_refused_and_leave_nothing_behind() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let shot = record("shot", TODAY, (9, 0));
        let empty = Capture {
            width: 0,
            height: 0,
            ..shot.clone()
        };
        for (image, record, width) in [
            (pixels(5, 2), &shot, THUMB),
            (pixels(4, 2), &shot, 0),
            (RgbaImage::new(0, 0), &empty, THUMB),
        ] {
            let result = store.add_capture(session, &image, record, width);
            assert!(matches!(result, Err(StoreError::BadDimensions)));
        }
        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(dir.temp_files().is_empty());
    }

    /// Adds a capture with a folder in the way of `blocked`, which must make it fail with `Io`.
    /// The folder is all that may be left in the temp folder afterwards, and no row.
    fn assert_blocked_add_leaves_nothing(blocked: &str) {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        fs::create_dir(dir.app_data().join("temp").join(blocked)).unwrap();

        let shot = record("shot", TODAY, (9, 0));
        let result = store.add_capture(session, &pixels(4, 2), &shot, THUMB);
        assert!(matches!(result, Err(StoreError::Io(_))));
        assert!(store.session_captures(session).unwrap().is_empty());
        assert_eq!(dir.temp_files(), [blocked]);
    }

    #[test]
    fn a_failed_file_write_leaves_no_row_and_no_files() {
        // The first capture's files are numbered 0 until they are renamed.
        assert_blocked_add_leaves_nothing("0.thumb.png.partial");
    }

    #[test]
    fn a_failed_rename_leaves_no_row_and_no_files() {
        // The first capture gets ID 1. Its image is already renamed when the thumbnail fails.
        assert_blocked_add_leaves_nothing("1.thumb.png");
    }

    #[test]
    fn a_failed_insert_leaves_no_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        store
            .db()
            .execute_batch(
                "CREATE TRIGGER refuse BEFORE INSERT ON captures
                    BEGIN SELECT RAISE(ABORT, 'refused'); END",
            )
            .unwrap();

        let shot = record("shot", TODAY, (9, 0));
        let result = store.add_capture(session, &pixels(4, 2), &shot, THUMB);
        assert!(matches!(result, Err(StoreError::Database(_))));
        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(dir.temp_files().is_empty());
    }

    #[test]
    fn a_capture_added_as_saved_keeps_no_temp_copy() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let shot = Capture {
            saved_path: Some(dir.user_file("auto.png")),
            ..record("auto", TODAY, (9, 0))
        };
        let id = store
            .add_capture(session, &pixels(4, 2), &shot, THUMB)
            .unwrap();
        assert!(!store.image_path(id).exists());
        assert!(store.thumbnail_path(id).is_file());
        assert_eq!(store.session_captures(session).unwrap()[0].capture, shot);
    }

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
    fn marking_saved_records_the_path_and_drops_the_temp_image() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = saved_copy(&store, id, &dir, "shot.png");

        let found = store.session_captures(session).unwrap();
        assert_eq!(found[0].capture.saved_path, Some(path.clone()));
        assert!(!store.image_path(id).exists());
        assert!(store.thumbnail_path(id).is_file());
        assert!(path.is_file());
    }

    #[test]
    fn marking_saved_refuses_a_file_that_does_not_exist() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));

        let result = store.mark_saved(id, &dir.0.join("nowhere.png"));
        assert!(matches!(result, Err(StoreError::Io(_))));
        assert!(store.image_path(id).is_file());
        let found = store.session_captures(session).unwrap();
        assert_eq!(found[0].capture.saved_path, None);
    }

    #[test]
    fn marking_saved_tolerates_a_missing_temp_image() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        fs::remove_file(store.image_path(id)).unwrap();
        saved_copy(&store, id, &dir, "shot.png");
        assert!(
            store.session_captures(session).unwrap()[0]
                .capture
                .is_saved()
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
    fn delete_removes_the_row_and_temp_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let keep = add(&store, session, "keep", TODAY, (9, 0));
        let id = add(&store, session, "gone", TODAY, (10, 0));
        store.delete(id).unwrap();

        assert_eq!(names(&store.session_captures(session).unwrap()), ["keep"]);
        assert!(!store.image_path(id).exists());
        assert!(!store.thumbnail_path(id).exists());
        assert!(store.image_path(keep).is_file());
    }

    #[test]
    fn deleting_a_saved_capture_leaves_the_users_file() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = saved_copy(&store, id, &dir, "shot.png");
        store.delete(id).unwrap();

        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(!store.thumbnail_path(id).exists());
        assert_eq!(fs::read(path).unwrap(), b"the user's file");
    }

    #[test]
    fn delete_tolerates_missing_files_and_unknown_ids() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        fs::remove_file(store.image_path(id)).unwrap();
        fs::remove_file(store.thumbnail_path(id)).unwrap();
        store.delete(id).unwrap();
        store.delete(id).unwrap();
        assert!(store.session_captures(session).unwrap().is_empty());
    }

    #[test]
    fn discard_removes_only_that_sessions_unsaved_captures() {
        let dir = TestDir::new();
        let store = dir.open();
        let ended = store.new_session().unwrap();
        let closing = store.new_session().unwrap();
        let earlier = add(&store, ended, "earlier", TODAY, (8, 0));
        let unsaved = add(&store, closing, "unsaved", TODAY, (9, 0));
        let saved = add(&store, closing, "saved", TODAY, (10, 0));
        let path = saved_copy(&store, saved, &dir, "saved.png");

        store.discard_unsaved(closing).unwrap();

        assert_eq!(names(&store.session_captures(closing).unwrap()), ["saved"]);
        assert_eq!(names(&store.session_captures(ended).unwrap()), ["earlier"]);
        assert!(!store.image_path(unsaved).exists());
        assert!(!store.thumbnail_path(unsaved).exists());
        assert!(store.image_path(earlier).is_file());
        assert!(store.thumbnail_path(saved).is_file());
        assert!(path.is_file());
    }

    #[test]
    fn discard_tolerates_missing_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        fs::remove_file(store.thumbnail_path(id)).unwrap();
        store.discard_unsaved(session).unwrap();
        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(!store.image_path(id).exists());
    }

    #[test]
    fn retention_keeps_captures_exactly_n_days_old() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let today = date(2026, 1, 10);
        add(&store, session, "today", today, (9, 0));
        let boundary = add(&store, session, "30 days", date(2025, 12, 11), (0, 0));
        let expired = add(&store, session, "31 days", date(2025, 12, 10), (23, 59));
        add(&store, session, "long ago", date(2024, 6, 1), (9, 0));

        store.delete_unsaved_older_than(30, today).unwrap();

        assert_eq!(
            names(&store.session_captures(session).unwrap()),
            ["today", "30 days"]
        );
        assert!(store.image_path(boundary).is_file());
        assert!(!store.image_path(expired).exists());
        assert!(!store.thumbnail_path(expired).exists());
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

    #[test]
    fn a_new_store_loads_the_default_settings() {
        let dir = TestDir::new();
        assert_eq!(dir.open().load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn a_database_from_before_settings_gets_the_table_on_open() {
        let dir = TestDir::new();
        let store = dir.open();
        store.db().execute_batch("DROP TABLE settings").unwrap();
        drop(store);
        assert_eq!(dir.open().load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn settings_survive_reopening() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        drop(store);
        assert_eq!(dir.open().load_settings().unwrap(), every_field_changed());
    }

    #[test]
    fn saving_settings_again_replaces_them() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        let cleared = Settings {
            region_shortcut: None,
            save_folder: None,
            ..Settings::default()
        };
        store.save_settings(&cleared).unwrap();
        assert_eq!(store.load_settings().unwrap(), cleared);
    }

    #[test]
    fn a_missing_setting_takes_its_default_and_the_others_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute("DELETE FROM settings WHERE key = 'look'", [])
            .unwrap();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                look: Settings::default().look,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn a_malformed_setting_takes_its_default_and_the_others_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute("UPDATE settings SET value = 'huh' WHERE key = 'format'", [])
            .unwrap();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                format: Settings::default().format,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn stored_values_that_are_not_text_do_not_fail_the_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute_batch(
                "UPDATE settings SET value = x'00ff' WHERE key = 'on_close';
                 UPDATE settings SET value = CAST(x'ff' AS TEXT) WHERE key = 'look';
                 INSERT INTO settings (key, value) VALUES (x'ff', 'x');",
            )
            .unwrap();
        let defaults = Settings::default();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                on_close: defaults.on_close,
                look: defaults.look,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn a_failed_settings_save_changes_nothing() {
        let dir = TestDir::new();
        let store = dir.open();
        store
            .db()
            .execute_batch(
                "CREATE TRIGGER refuse BEFORE INSERT ON settings
                    WHEN NEW.key = 'settings_collapsed'
                    BEGIN SELECT RAISE(ABORT, 'refused'); END",
            )
            .unwrap();

        let result = store.save_settings(&every_field_changed());
        assert!(matches!(result, Err(StoreError::Database(_))));
        assert_eq!(store.load_settings().unwrap(), Settings::default());
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_that_is_not_valid_unicode_is_refused() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let dir = TestDir::new();
        let store = dir.open();
        let settings = Settings {
            save_folder: Some(PathBuf::from(OsString::from_wide(&[0xD800]))),
            ..every_field_changed()
        };
        let result = store.save_settings(&settings);
        assert!(matches!(result, Err(StoreError::Io(_))));
        assert_eq!(store.load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn captures_can_be_added_and_queried_from_several_threads() {
        let dir = TestDir::new();
        let store = Arc::new(dir.open());
        let session = store.new_session().unwrap();

        let threads: Vec<_> = (0..4)
            .map(|worker| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    let mut ids = Vec::new();
                    for minute in 0..5 {
                        ids.push(add(&store, session, "shot", TODAY, (worker, minute)));
                        store.session_captures(session).unwrap();
                    }
                    ids
                })
            })
            .collect();

        let mut ids: Vec<CaptureId> = threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        assert_eq!(ids.len(), 20);
        assert!(ids.iter().all(|&id| store.image_path(id).is_file()));
        assert_eq!(store.session_captures(session).unwrap().len(), 20);
        assert_eq!(dir.temp_files().len(), 40, "two files per capture, no more");
    }
}
