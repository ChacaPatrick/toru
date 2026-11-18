use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use image::{ImageFormat, RgbaImage};

use super::{CaptureId, Result, Store};

/// Ends the name of a file that `add_capture` hasn't finished with yet.
pub(super) const PARTIAL_SUFFIX: &str = ".partial";

impl Store {
    /// The full image in the temp folder. It exists until the capture is saved, after which the
    /// image is the file at `saved_path`, unless edits are waiting for another Save.
    pub fn image_path(&self, id: CaptureId) -> PathBuf {
        self.temp_dir.join(format!("{}.png", id.0))
    }

    pub fn thumbnail_path(&self, id: CaptureId) -> PathBuf {
        self.temp_dir.join(format!("{}.thumb.png", id.0))
    }

    /// The capture as it was taken, kept here once it has been edited, for Reset edits.
    pub fn original_path(&self, id: CaptureId) -> PathBuf {
        self.temp_dir.join(format!("{}.original.png", id.0))
    }

    /// Where the capture's full image is now: its temp copy while there is one, which on a saved
    /// capture holds edits not yet saved, or else the file it was saved to.
    pub fn full_image_path(&self, id: CaptureId) -> Result<PathBuf> {
        let temp = self.image_path(id);
        Ok(match self.capture(id)?.saved_path {
            Some(saved) if !temp.is_file() => saved,
            _ => temp,
        })
    }

    /// Replaces the file at `path` in one step, so a crash leaves the old one whole.
    pub(super) fn publish(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let number = self.next_partial.fetch_add(1, Ordering::Relaxed);
        let partial = self.temp_dir.join(format!("{number}.edit{PARTIAL_SUFFIX}"));
        let published = write_file(&partial, bytes).and_then(|()| fs::rename(&partial, path));
        if published.is_err() {
            let _ = fs::remove_file(&partial);
        }
        published
    }

    // Failures are ignored: the file may already be gone, and anything left is swept by `open`.
    pub(super) fn remove_temp_files(&self, id: CaptureId) {
        let _ = fs::remove_file(self.image_path(id));
        let _ = fs::remove_file(self.thumbnail_path(id));
        let _ = fs::remove_file(self.original_path(id));
    }
}

pub(crate) fn encode_png(pixels: &RgbaImage) -> Result<Vec<u8>> {
    let mut png = Cursor::new(Vec::new());
    pixels.write_to(&mut png, ImageFormat::Png)?;
    Ok(png.into_inner())
}

// Synced so the commit that follows can't outlive the data it refers to, even on power loss.
pub(super) fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StoreError;
    use crate::store::test_support::*;

    #[test]
    fn the_full_image_is_the_saved_file_once_there_is_one() {
        let dir = TestDir::new();
        let store = dir.open();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot", TODAY, (9, 0));
        assert_eq!(store.full_image_path(id).unwrap(), store.image_path(id));

        let path = saved_copy(&store, id, &dir, "shot.png");
        assert_eq!(store.full_image_path(id).unwrap(), path);
        assert!(matches!(
            store.full_image_path(CaptureId(42)),
            Err(StoreError::NotFound)
        ));
    }
}
