//! Writing captures out as image files: into the user's folder when they are saved, and into a
//! throwaway folder for Copy all.

use std::fs::{self, File};
use std::io::{self, Cursor, ErrorKind, Write};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;

use crate::settings::SaveFormat;
use crate::store::{CaptureId, Result, Store, path_text};

const JPEG_QUALITY: u8 = 90;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedFile {
    pub path: PathBuf,
    pub bytes: u64,
}

/// What Copy all or Copy as files puts on the clipboard.
#[derive(Debug, PartialEq, Eq)]
pub struct CopySet {
    /// Holds the unsaved captures' files, which must outlive the paste (`docs/spikes.md` §1.4).
    pub folder: PathBuf,
    pub files: Vec<String>,
    /// Saved captures left out because their file has gone: one missing file loses a whole paste.
    pub missing: usize,
}

/// Writes the capture into `folder`, creating it if needed, and records it as saved. A capture
/// already saved is left alone and gives `None`, so saving twice can't make a second file. If
/// anything fails, the capture stays unsaved with its temp image and no new file is left behind.
pub fn save_capture(
    store: &Store,
    id: CaptureId,
    folder: &Path,
    format: SaveFormat,
) -> Result<Option<SavedFile>> {
    let capture = store.capture(id)?;
    if capture.is_saved() {
        return Ok(None);
    }
    let bytes = encoded(&store.image_path(id), format)?;
    fs::create_dir_all(folder)?;
    let path = write_new_file(folder, &capture.name, format, &bytes)?;
    if let Err(error) = store.mark_saved(id, &path) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(Some(SavedFile {
        path,
        bytes: bytes.len() as u64,
    }))
}

/// Gets `captures` ready to copy as files: a saved capture is its own file, and an unsaved one is
/// written to a new folder from `Store::new_copy_folder` under its name.
pub fn prepare_copy_set(
    store: &Store,
    captures: &[CaptureId],
    format: SaveFormat,
) -> Result<CopySet> {
    let folder = store.new_copy_folder()?;
    let prepared = fill_copy_set(store, captures, format, &folder);
    if prepared.is_err() {
        let _ = fs::remove_dir_all(&folder);
    }
    let (files, missing) = prepared?;
    Ok(CopySet {
        folder,
        files,
        missing,
    })
}

fn fill_copy_set(
    store: &Store,
    captures: &[CaptureId],
    format: SaveFormat,
    folder: &Path,
) -> Result<(Vec<String>, usize)> {
    let mut files = Vec::new();
    let mut missing = 0;
    for &id in captures {
        let capture = store.capture(id)?;
        match capture.saved_path {
            Some(path) if path.is_file() => files.push(path_text(&path)?.to_string()),
            Some(_) => missing += 1,
            None => {
                let bytes = encoded(&store.image_path(id), format)?;
                let path = write_new_file(folder, &capture.name, format, &bytes)?;
                files.push(path_text(&path)?.to_string());
            }
        }
    }
    Ok((files, missing))
}

/// The temp PNG in `format`. PNG needs no re-encoding, so the bytes go across untouched.
fn encoded(source: &Path, format: SaveFormat) -> Result<Vec<u8>> {
    if format == SaveFormat::Png {
        return Ok(fs::read(source)?);
    }
    let image = image::open(source)?;
    match format {
        SaveFormat::Png => unreachable!("returned above"),
        // JPEG has no alpha channel.
        SaveFormat::Jpg => {
            let mut bytes = Cursor::new(Vec::new());
            image
                .to_rgb8()
                .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY))?;
            Ok(bytes.into_inner())
        }
        // Lossless, because screenshots are mostly text and flat colour.
        SaveFormat::WebP => {
            let rgba = image.to_rgba8();
            let (width, height) = rgba.dimensions();
            webp::Encoder::from_rgba(&rgba, width, height)
                .encode_simple(true, 75.)
                .map(|memory| memory.to_vec())
                .map_err(|error| io::Error::other(format!("WebP encoding: {error:?}")).into())
        }
    }
}

/// Writes `bytes` to `name.ext`, or to `name_2.ext` and so on if that is taken. `create_new` makes
/// the check and the creation one step, so an existing file is never overwritten.
fn write_new_file(
    folder: &Path,
    name: &str,
    format: SaveFormat,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    let extension = format.extension();
    let mut number = 1;
    loop {
        let file_name = if number == 1 {
            format!("{name}.{extension}")
        } else {
            format!("{name}_{number}.{extension}")
        };
        let path = folder.join(file_name);
        match File::options().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let written = file.write_all(bytes).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = written {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => number += 1,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::process;
    use std::sync::atomic::{AtomicU32, Ordering};

    use image::{ImageFormat, Rgba, RgbaImage};

    use super::*;
    use crate::capture::{Capture, CaptureMode};
    use crate::store::{SessionId, StoreError};
    use crate::time::{LocalDate, LocalDateTime};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> TestDir {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let number = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!("toru-export-{}-{number}", process::id()));
            fs::create_dir_all(&path).unwrap();
            TestDir(path)
        }

        fn store(&self) -> Store {
            Store::open(&self.0.join("app")).unwrap()
        }

        fn folder(&self) -> PathBuf {
            self.0.join("Pictures").join("Screenshots")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn frame() -> RgbaImage {
        RgbaImage::from_fn(32, 20, |x, y| Rgba([x as u8 * 7, y as u8 * 11, 200, 255]))
    }

    fn add(store: &Store, session: SessionId, name: &str) -> CaptureId {
        let image = frame();
        let capture = Capture {
            name: name.to_string(),
            mode: CaptureMode::Region,
            width: image.width(),
            height: image.height(),
            taken_at: LocalDateTime {
                date: LocalDate {
                    year: 2026,
                    month: 11,
                    day: 8,
                },
                hour: 9,
                minute: 0,
            },
            saved_path: None,
        };
        store.add_capture(session, &image, &capture, 16).unwrap()
    }

    fn session_ids(store: &Store, session: SessionId) -> Vec<CaptureId> {
        let captures = store.session_captures(session).unwrap();
        captures.iter().map(|stored| stored.id).collect()
    }

    fn file_name(path: &Path) -> &str {
        path.file_name().unwrap().to_str().unwrap()
    }

    #[test]
    fn saving_as_png_writes_the_exact_image_and_marks_it_saved() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");

        let saved = save_capture(&store, id, &dir.folder(), SaveFormat::Png)
            .unwrap()
            .unwrap();

        assert_eq!(saved.path, dir.folder().join("shot.png"));
        assert_eq!(saved.bytes, fs::metadata(&saved.path).unwrap().len());
        assert_eq!(image::open(&saved.path).unwrap().to_rgba8(), frame());
        assert_eq!(store.capture(id).unwrap().saved_path, Some(saved.path));
        assert!(!store.image_path(id).exists());
    }

    #[test]
    fn saving_as_webp_is_lossless_and_really_webp() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");

        let saved = save_capture(&store, id, &dir.folder(), SaveFormat::WebP)
            .unwrap()
            .unwrap();

        assert_eq!(file_name(&saved.path), "shot.webp");
        let bytes = fs::read(&saved.path).unwrap();
        assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::WebP);
        assert_eq!(image::open(&saved.path).unwrap().to_rgba8(), frame());
    }

    #[test]
    fn saving_as_jpg_is_really_a_jpeg_of_the_same_size() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");

        let saved = save_capture(&store, id, &dir.folder(), SaveFormat::Jpg)
            .unwrap()
            .unwrap();

        assert_eq!(file_name(&saved.path), "shot.jpg");
        let bytes = fs::read(&saved.path).unwrap();
        assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Jpeg);
        let image = image::open(&saved.path).unwrap();
        assert_eq!((image.width(), image.height()), (32, 20));
    }

    #[test]
    fn saving_twice_writes_one_file() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");

        assert!(
            save_capture(&store, id, &dir.folder(), SaveFormat::Png)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            save_capture(&store, id, &dir.folder(), SaveFormat::Png).unwrap(),
            None
        );
        assert_eq!(fs::read_dir(dir.folder()).unwrap().count(), 1);
    }

    #[test]
    fn an_existing_file_is_kept_and_the_next_free_name_is_used() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");
        fs::create_dir_all(dir.folder()).unwrap();
        fs::write(dir.folder().join("shot.png"), b"the user's file").unwrap();
        fs::write(dir.folder().join("SHOT_2.PNG"), b"another").unwrap();

        let saved = save_capture(&store, id, &dir.folder(), SaveFormat::Png)
            .unwrap()
            .unwrap();

        assert_eq!(file_name(&saved.path), "shot_3.png");
        assert_eq!(
            fs::read(dir.folder().join("shot.png")).unwrap(),
            b"the user's file"
        );
    }

    #[test]
    fn a_folder_that_cannot_be_written_leaves_the_capture_unsaved() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");
        // A file standing where a folder is needed, as a deleted or broken folder would fail.
        let blocker = dir.0.join("blocker");
        fs::write(&blocker, b"not a folder").unwrap();

        let result = save_capture(&store, id, &blocker.join("Screenshots"), SaveFormat::Png);

        assert!(matches!(result, Err(StoreError::Io(_))));
        assert_eq!(store.capture(id).unwrap().saved_path, None);
        assert!(
            store.image_path(id).is_file(),
            "the capture must not be lost"
        );
    }

    #[test]
    fn a_deleted_capture_writes_nothing() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");
        store.delete(id).unwrap();

        let result = save_capture(&store, id, &dir.folder(), SaveFormat::Png);

        assert!(matches!(result, Err(StoreError::NotFound)));
        assert!(!dir.folder().exists());
    }

    #[test]
    fn a_copy_set_writes_unsaved_captures_under_their_names_and_lists_saved_files() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "unsaved");
        let saved_id = add(&store, session, "saved");
        let saved = save_capture(&store, saved_id, &dir.folder(), SaveFormat::Png)
            .unwrap()
            .unwrap();

        let set = prepare_copy_set(&store, &session_ids(&store, session), SaveFormat::Png).unwrap();

        assert_eq!(set.missing, 0);
        assert_eq!(set.files.len(), 2);
        assert!(set.files.contains(&saved.path.display().to_string()));
        let unsaved = set.folder.join("unsaved.png");
        assert!(set.files.contains(&unsaved.display().to_string()));
        assert_eq!(image::open(unsaved).unwrap().to_rgba8(), frame());
    }

    #[test]
    fn a_copy_set_uses_the_chosen_format() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "shot");

        let set =
            prepare_copy_set(&store, &session_ids(&store, session), SaveFormat::WebP).unwrap();

        let bytes = fs::read(set.folder.join("shot.webp")).unwrap();
        assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::WebP);
    }

    #[test]
    fn a_copy_set_leaves_out_a_saved_file_that_has_gone() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "unsaved");
        let id = add(&store, session, "saved");
        let saved = save_capture(&store, id, &dir.folder(), SaveFormat::Png)
            .unwrap()
            .unwrap();
        fs::remove_file(&saved.path).unwrap();

        let set = prepare_copy_set(&store, &session_ids(&store, session), SaveFormat::Png).unwrap();

        assert_eq!(set.missing, 1);
        assert_eq!(set.files.len(), 1);
    }

    #[test]
    fn a_failed_copy_set_leaves_no_folder_behind() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        let id = add(&store, session, "shot");
        fs::remove_file(store.image_path(id)).unwrap();

        assert!(prepare_copy_set(&store, &session_ids(&store, session), SaveFormat::Png).is_err());
        let copies = fs::read_dir(dir.0.join("app").join("copies")).unwrap();
        assert_eq!(copies.count(), 0);
    }

    #[test]
    fn captures_with_the_same_name_keep_separate_files_in_a_copy_set() {
        let dir = TestDir::new();
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "shot");
        add(&store, session, "shot");

        let set = prepare_copy_set(&store, &session_ids(&store, session), SaveFormat::Png).unwrap();

        assert_eq!(set.files.len(), 2);
        assert!(set.folder.join("shot.png").is_file());
        assert!(set.folder.join("shot_2.png").is_file());
    }
}
