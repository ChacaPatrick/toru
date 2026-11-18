use std::fs;

use image::RgbaImage;
use rusqlite::{OptionalExtension, params};

use super::{CaptureId, Result, Store, StoreError, encode_png};

/// The `redacted` value of a capture whose picture, hiding some of it, has been saved, so that
/// its original must be gone.
pub(super) const ORIGINAL_REMOVED: i64 = 2;

/// What `Store::redaction` tells about a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Redaction {
    /// The picture hides some of what was taken under blur, pixelate or solid boxes.
    pub redacted: bool,
    /// The original was deleted when that picture was saved, so Reset edits can't bring it back.
    pub original_removed: bool,
}

impl Store {
    /// Makes `edited` the capture's picture in the temp folder and the thumbnail, leaving any
    /// file the user saved as it is until they save again. `original` is the capture as it was
    /// taken, kept for Reset edits; `None` says `edited` is that picture again, so an unsaved
    /// capture needs nothing kept. A saved capture keeps it, as its file may still differ.
    /// `redacted`: whether `edited` hides some of the capture, so a Save deletes the original.
    /// Returns the size of the picture written. Once that is written the capture is `edited`,
    /// even if its thumbnail can't follow: that one is deleted, so the card can't show what the
    /// picture hides, and made again when the gallery next loads it (`remake_narrow_thumbnail`).
    pub fn store_edit(
        &self,
        id: CaptureId,
        edited: &RgbaImage,
        original: Option<&RgbaImage>,
        redacted: bool,
        max_thumbnail_width: u32,
    ) -> Result<u64> {
        self.check_not_revealing(id, redacted)?;
        if original.is_some() || !self.capture(id)?.is_saved() {
            self.keep_original(id, original)?;
        }
        // Before the picture, so a failure part way can only cost Reset, never keep what was hidden.
        self.set_redacted(id, redacted)?;
        let png = encode_png(edited)?;
        self.publish(&self.image_path(id), &png)?;
        if self
            .replace_thumbnail(id, edited, max_thumbnail_width)
            .is_err()
        {
            let _ = fs::remove_file(self.thumbnail_path(id));
        }
        Ok(png.len() as u64)
    }

    /// Keeps `original` for Reset edits, the first time it is given. `None` drops what was kept.
    /// An original deleted because a save hid some of it is never written back.
    pub(crate) fn keep_original(&self, id: CaptureId, original: Option<&RgbaImage>) -> Result<()> {
        if self.redaction(id)?.original_removed {
            return Ok(());
        }
        let path = self.original_path(id);
        match original {
            Some(original) if !path.is_file() => self.publish(&path, &encode_png(original)?)?,
            Some(_) => {}
            // A leftover only costs disk until the capture is deleted.
            None => drop(fs::remove_file(path)),
        }
        Ok(())
    }

    /// Whether the capture's picture hides some of what was taken, and whether its original has
    /// been deleted because of that.
    pub fn redaction(&self, id: CaptureId) -> Result<Redaction> {
        let value: i64 = self
            .db()
            .query_row(
                "SELECT redacted FROM captures WHERE id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        Ok(Redaction {
            redacted: value != 0,
            original_removed: value == ORIGINAL_REMOVED,
        })
    }

    /// Refuses a picture without redactions for a capture whose original was deleted, as it
    /// can only come from before that save: it would bring back what was hidden.
    pub fn check_not_revealing(&self, id: CaptureId, redacted: bool) -> Result<()> {
        if !redacted && self.redaction(id)?.original_removed {
            return Err(StoreError::WouldReveal);
        }
        Ok(())
    }

    // Once the original is removed, it stays so.
    fn set_redacted(&self, id: CaptureId, redacted: bool) -> Result<()> {
        self.db().execute(
            "UPDATE captures SET redacted = ?1 WHERE id = ?2 AND redacted <> ?3",
            params![redacted, id.0, ORIGINAL_REMOVED],
        )?;
        Ok(())
    }

    /// Deletes the kept original if the capture's picture hides some of it, as a saved copy of
    /// that picture now exists. Returns whether the original is now removed for that reason. It
    /// counts as removed even if the file can't be deleted yet: Reset no longer offers it, and
    /// `open` deletes it.
    pub fn remove_redacted_original(&self, id: CaptureId) -> Result<bool> {
        let changed = self.db().execute(
            "UPDATE captures SET redacted = ?1 WHERE id = ?2 AND redacted <> 0",
            params![ORIGINAL_REMOVED, id.0],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        // If this fails, the next `open` tries again.
        let _ = fs::remove_file(self.original_path(id));
        Ok(true)
    }
}
