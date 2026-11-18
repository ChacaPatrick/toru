use std::io;
use std::path::Path;

use super::{CaptureId, Result, SessionId, Store, StoreError};

impl Store {
    /// A file the user saved stays where it is.
    pub fn delete(&self, id: CaptureId) -> Result<()> {
        self.delete_rows("DELETE FROM captures WHERE id = ?1 RETURNING id", id.0)
            .map(drop)
    }

    /// In the app, `remove` sends the user's file to the Recycle Bin. The record goes first, so a
    /// failure can only leave a file without a record. Returns whether the file is gone; `false`
    /// doesn't undo the delete. A file already missing counts as gone. A file another capture is
    /// saved to stays, and counts as not gone: the path may have been reused since.
    pub fn delete_with_file(
        &self,
        id: CaptureId,
        remove: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<bool> {
        let saved = match self.capture(id) {
            Ok(capture) => capture.saved_path,
            Err(StoreError::NotFound) => None,
            Err(error) => return Err(error),
        };
        // Asked before the record goes, so a failure here changes nothing.
        let shared = match &saved {
            Some(path) => self.saved_by_another(id, path)?,
            None => false,
        };
        self.delete(id)?;
        if shared {
            return Ok(false);
        }
        Ok(saved.is_none_or(|path| match remove(&path) {
            Ok(()) => true,
            Err(error) => error.kind() == io::ErrorKind::NotFound,
        }))
    }

    pub fn discard_unsaved(&self, session: SessionId) -> Result<()> {
        self.delete_rows(
            "DELETE FROM captures WHERE session = ?1 AND saved_path IS NULL RETURNING id",
            session.0,
        )
        .map(drop)
    }

    /// `sql` must be a single `DELETE … RETURNING id`. Its rows are committed as gone before any
    /// file is touched.
    pub(super) fn delete_rows(&self, sql: &'static str, param: i64) -> Result<Vec<CaptureId>> {
        let ids: Vec<CaptureId> = {
            let db = self.db();
            let mut statement = db.prepare(sql)?;
            statement
                .query_map([param], |row| row.get(0).map(CaptureId))?
                .collect::<rusqlite::Result<_>>()?
        };
        for &id in &ids {
            self.remove_temp_files(id);
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::store::test_support::*;

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
    fn delete_with_file_removes_the_record_and_the_users_file() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = saved_copy(&store, id, &dir, "shot.png");

        assert!(store.delete_with_file(id, remove).unwrap());

        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(!path.exists());
        assert!(!store.thumbnail_path(id).exists());
    }

    #[test]
    fn delete_with_file_leaves_a_file_another_capture_now_has() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let moved = add(&store, session, "moved", TODAY, (9, 0));
        let owner = add(&store, session, "owner", TODAY, (10, 0));
        // The user moves the first file away and the second capture is saved to that name.
        fs::remove_file(saved_copy(&store, moved, &dir, "shot.png")).unwrap();
        let path = saved_copy(&store, owner, &dir, "shot.png");

        assert!(!store.delete_with_file(moved, remove).unwrap());

        assert_eq!(names(&store.session_captures(session).unwrap()), ["owner"]);
        assert_eq!(fs::read(&path).unwrap(), b"the user's file");

        assert!(store.delete_with_file(owner, remove).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn delete_with_file_treats_paths_differing_in_case_as_one_file() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let moved = add(&store, session, "moved", TODAY, (9, 0));
        let owner = add(&store, session, "owner", TODAY, (10, 0));
        fs::remove_file(saved_copy(&store, moved, &dir, "shot.png")).unwrap();
        let path = saved_copy(&store, owner, &dir, "SHOT.PNG");

        assert!(!store.delete_with_file(moved, remove).unwrap());

        assert_eq!(fs::read(&path).unwrap(), b"the user's file");
    }

    #[test]
    fn delete_with_file_on_an_unsaved_capture_touches_only_its_temp_files() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));

        assert!(store.delete_with_file(id, remove).unwrap());

        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(!store.image_path(id).exists());
    }

    #[test]
    fn delete_with_file_counts_a_missing_file_as_gone_and_tolerates_unknown_ids() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = saved_copy(&store, id, &dir, "shot.png");
        fs::remove_file(path).unwrap();

        assert!(store.delete_with_file(id, remove).unwrap());
        assert!(store.delete_with_file(id, remove).unwrap());
    }

    #[test]
    fn a_file_that_cannot_be_removed_is_reported_but_the_record_still_goes() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = saved_copy(&store, id, &dir, "shot.png");
        // `remove_file` refuses a folder, as it would a file another program holds open.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();

        assert!(!store.delete_with_file(id, remove).unwrap());

        assert!(store.session_captures(session).unwrap().is_empty());
        assert!(path.is_dir());
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
}
