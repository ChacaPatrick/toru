//! Captures on disk: image files in a temp folder, records in SQLite.
//!
//! A row only exists with its files complete: adding writes the files under temporary names and
//! renames them inside the row's transaction, committing last; removing deletes the row first. So
//! a crash can only leave files without a row, which `Store::open` sweeps, and because of that
//! sweep a `Store` holds an exclusive lock in the app-data folder for as long as it is open.

mod add;
mod copy_folders;
mod delete;
mod edits;
mod error;
mod files;
mod names;
mod open;
mod queries;
mod retention;
mod saved;
mod settings;
#[cfg(test)]
mod test_support;
mod thumbnails;

use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::Connection;

use crate::capture::Capture;

pub use edits::Redaction;
pub use error::{Result, StoreError};
pub(crate) use files::encode_png;
pub(crate) use queries::{mode_from_text, mode_text};
pub(crate) use saved::path_text;

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
    copies_dir: PathBuf,
    next_partial: AtomicU64,
    next_copy: AtomicU64,
    /// Held while a thumbnail is written, so a stale one can't land over a newer picture's.
    thumbnail_lock: Mutex<()>,
    // Last, so the database has closed by the time another process can get in.
    _instance_lock: File,
}

impl Store {
    /// IDs are never reused, even across restarts.
    pub fn new_session(&self) -> Result<SessionId> {
        let db = self.db();
        db.execute("INSERT INTO sessions DEFAULT VALUES", [])?;
        Ok(SessionId(db.last_insert_rowid()))
    }

    // A panic can't leave the connection mid-change, since an open transaction rolls back when
    // dropped, so a poisoned lock is safe to take.
    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
