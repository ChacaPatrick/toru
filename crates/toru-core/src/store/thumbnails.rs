use std::sync::{MutexGuard, PoisonError};

use image::{RgbaImage, imageops};
use rusqlite::params;

use super::{CaptureId, Result, Store, encode_png};

impl Store {
    /// Also records `edited`'s size.
    pub fn replace_thumbnail(
        &self,
        id: CaptureId,
        edited: &RgbaImage,
        max_thumbnail_width: u32,
    ) -> Result<()> {
        let _writing = self.lock_thumbnails();
        self.write_thumbnail(id, edited, max_thumbnail_width)
    }

    /// The caller holds the thumbnail lock.
    fn write_thumbnail(&self, id: CaptureId, pixels: &RgbaImage, max_width: u32) -> Result<()> {
        let png = thumbnail_png(pixels, max_width)?;
        self.publish(&self.thumbnail_path(id), &png)?;
        self.db().execute(
            "UPDATE captures SET width = ?1, height = ?2 WHERE id = ?3",
            params![pixels.width(), pixels.height(), id.0],
        )?;
        Ok(())
    }

    /// Narrow ones were stored before the cards could grow; a missing one couldn't follow an edit
    /// (`store_edit`). A saved file whose size no longer matches the record is left alone: it was
    /// changed outside Toru. A missing thumbnail is made whatever the picture's size, which the
    /// record then takes, since the edit may have failed to record a crop's new size.
    pub fn remake_narrow_or_missing_thumbnail(
        &self,
        id: CaptureId,
        max_width: u32,
    ) -> Result<bool> {
        let _writing = self.lock_thumbnails();
        let capture = self.capture(id)?;
        if !self.thumbnail_path(id).exists() {
            let pixels = image::open(self.full_image_path(id)?)?.to_rgba8();
            self.write_thumbnail(id, &pixels, max_width)?;
            return Ok(true);
        }
        let (width, _) = image::image_dimensions(self.thumbnail_path(id))?;
        if width >= capture.width.min(max_width) {
            return Ok(false);
        }
        let pixels = image::open(self.full_image_path(id)?)?.to_rgba8();
        if pixels.dimensions() != (capture.width, capture.height) {
            return Ok(false);
        }
        self.publish(
            &self.thumbnail_path(id),
            &thumbnail_png(&pixels, max_width)?,
        )?;
        Ok(true)
    }

    fn lock_thumbnails(&self) -> MutexGuard<'_, ()> {
        self.thumbnail_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

pub(super) fn thumbnail_png(pixels: &RgbaImage, max_width: u32) -> Result<Vec<u8>> {
    let (width, height) = pixels.dimensions();
    let thumbnail_width = width.min(max_width);
    let thumbnail_height =
        (u64::from(height) * u64::from(thumbnail_width) / u64::from(width)).max(1) as u32;
    encode_png(&imageops::thumbnail(
        pixels,
        thumbnail_width,
        thumbnail_height,
    ))
}

#[cfg(test)]
mod tests {
    use crate::store::Capture;
    use crate::store::test_support::*;

    #[test]
    fn a_thumbnail_narrower_than_wanted_is_remade_from_the_picture() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let shot = Capture {
            width: 40,
            height: 20,
            ..record("shot", TODAY, (9, 0))
        };
        let id = store
            .add_capture(session, &pixels(40, 20), &shot, 10)
            .unwrap();
        let width = || image::image_dimensions(store.thumbnail_path(id)).unwrap();

        assert!(!store.remake_narrow_or_missing_thumbnail(id, 10).unwrap());
        assert!(store.remake_narrow_or_missing_thumbnail(id, 30).unwrap());
        assert_eq!(width(), (30, 15));
        // Never wider than the picture, so a thumbnail as wide as it is left alone.
        assert!(store.remake_narrow_or_missing_thumbnail(id, 100).unwrap());
        assert_eq!(width(), (40, 20));
        assert!(!store.remake_narrow_or_missing_thumbnail(id, 100).unwrap());
    }

    #[test]
    fn a_saved_file_changed_elsewhere_leaves_the_thumbnail_alone() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        let path = dir.0.join("shot.png");
        pixels(8, 8).save(&path).unwrap();
        store.mark_saved(id, &path).unwrap();

        assert!(!store.remake_narrow_or_missing_thumbnail(id, 100).unwrap());
        assert_eq!(
            image::image_dimensions(store.thumbnail_path(id)).unwrap(),
            (THUMB, 1)
        );
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
}
