use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;

use image::RgbaImage;
use rusqlite::params;

use super::files::{PARTIAL_SUFFIX, write_file};
use super::queries::date_number;
use super::thumbnails::thumbnail_png;
use super::{CaptureId, Result, SessionId, Store, StoreError, encode_png, mode_text, path_text};
use crate::capture::Capture;

impl Store {
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
        let thumbnail_png = thumbnail_png(pixels, max_thumbnail_width)?;

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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use image::Rgba;

    use super::*;
    use crate::store::test_support::*;

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
