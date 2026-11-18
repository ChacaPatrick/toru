use std::path::Path;
use std::{fs, io};

use rusqlite::params;

use super::edits::ORIGINAL_REMOVED;
use super::{CaptureId, Result, Store, StoreError};
use crate::naming::fold_case;

impl Store {
    /// `path` must already exist. The temp copy of the full image goes, so saved captures don't
    /// pile up in the temp folder; the thumbnail stays for the gallery. If the picture hides some
    /// of the capture, the kept original goes too.
    pub fn mark_saved(&self, id: CaptureId, path: &Path) -> Result<()> {
        self.record_saved(id, path, None)
    }

    /// `redacted`: whether the saved picture hides some of the capture, or `None` if it is the
    /// capture's picture as stored.
    pub(crate) fn record_saved(
        &self,
        id: CaptureId,
        path: &Path,
        redacted: Option<bool>,
    ) -> Result<()> {
        fs::metadata(path)?;
        // A removed original stays so, whatever `redacted` says.
        let changed = self.db().execute(
            "UPDATE captures SET saved_path = ?1,
                redacted = CASE WHEN redacted = ?4 THEN ?4 ELSE COALESCE(?3, redacted) END
                WHERE id = ?2",
            params![path_text(path)?, id.0, redacted, ORIGINAL_REMOVED],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        // If this fails, the next `open` tries again.
        let _ = fs::remove_file(self.image_path(id));
        // The file is saved either way, and failing here would have the caller delete it.
        let _ = self.remove_redacted_original(id);
        Ok(())
    }

    // Case is ignored, as Windows ignores it: two spellings of a path are one file.
    pub(super) fn saved_by_another(&self, id: CaptureId, path: &Path) -> Result<bool> {
        let wanted = fold_case(path_text(path)?);
        let db = self.db();
        let mut others = db
            .prepare("SELECT saved_path FROM captures WHERE saved_path IS NOT NULL AND id <> ?1")?;
        for other in others.query_map([id.0], |row| row.get::<_, String>(0))? {
            if fold_case(&other?) == wanted {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// Refused rather than stored lossily, which would leave the record pointing at another file.
pub(crate) fn path_text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path is not valid Unicode").into()
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::settings::Settings;
    use crate::settings::tests::every_field_changed;
    use crate::store::test_support::*;

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
}
