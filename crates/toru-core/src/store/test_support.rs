use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, io, process};

use image::{Rgba, RgbaImage};

use super::{CaptureId, SessionId, Store, StoredCapture};
use crate::capture::{Capture, CaptureMode};
use crate::time::{LocalDate, LocalDateTime};

pub(super) const TODAY: LocalDate = date(2026, 11, 8);
pub(super) const THUMB: u32 = 2;

pub(super) const fn date(year: u16, month: u8, day: u8) -> LocalDate {
    LocalDate { year, month, day }
}

/// A throwaway folder, removed on drop. Declare it before the store so the store closes first.
pub(super) struct TestDir(pub(super) PathBuf);

impl TestDir {
    pub(super) fn new() -> TestDir {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("toru-store-{}-{number}", process::id()));
        fs::create_dir_all(&path).unwrap();
        TestDir(path)
    }

    pub(super) fn app_data(&self) -> PathBuf {
        self.0.join("app")
    }

    pub(super) fn open(&self) -> Store {
        Store::open(&self.app_data()).unwrap()
    }

    pub(super) fn user_file(&self, name: &str) -> PathBuf {
        let folder = self.0.join("Pictures");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(name);
        fs::write(&path, b"the user's file").unwrap();
        path
    }

    pub(super) fn temp_files(&self) -> Vec<String> {
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

pub(super) fn pixels(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]))
}

pub(super) fn record(name: &str, taken: LocalDate, (hour, minute): (u8, u8)) -> Capture {
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

pub(super) fn add(
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

pub(super) fn names(captures: &[StoredCapture]) -> Vec<&str> {
    captures.iter().map(|c| c.capture.name.as_str()).collect()
}

/// How the tests have `delete_with_file` remove a file: for good, not to the Recycle Bin.
pub(super) fn remove(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

pub(super) fn saved_copy(store: &Store, id: CaptureId, dir: &TestDir, name: &str) -> PathBuf {
    let path = dir.user_file(name);
    store.mark_saved(id, &path).unwrap();
    path
}
