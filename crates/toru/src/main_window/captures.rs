//! The capture pipeline: a frame comes in through `add_capture`, gets its name, goes into the
//! store, becomes a card and, when the setting says so, lands on the clipboard.
//!
//! Slow work stays off the main thread, and everything that writes runs on one queue
//! (`enqueue`): names are chosen one at a time so two captures can't get the same `_2`, and
//! clipboard writes can't overlap and mix formats (`docs/spikes.md` §1.4).

use std::{error::Error, fs, path::PathBuf};

use clipboard_rs::{
    Clipboard, ClipboardContext,
    common::{RustImage, RustImageData},
};
use gpui::{AsyncApp, Context, SharedString, WeakEntity, prelude::*};
use toru_core::{
    capture::{Capture, CaptureMode},
    export::save_capture,
    naming::{NameError, next_free_name},
    settings::SaveFormat,
    store::{CaptureId, Result as StoreResult, SessionId, Store, StoreError},
    time::LocalDateTime,
};
use xcap::{
    Monitor,
    image::{self, DynamicImage, RgbaImage},
};

use super::{
    MainWindow,
    gallery::{CardFiles, NewCard},
    settings::pattern_message,
};
use crate::platform;
use crate::theme::ActiveTheme;
use crate::ui::Toast;

const CAPTURE_FAILED: &str = "Couldn't capture that display";
const STORE_FAILED: &str = "Couldn't store that capture";
pub(super) const COPY_FAILED: &str = "Couldn't copy to the clipboard. Another app may be using it.";
const RENAME_FAILED: &str = "Couldn't rename that capture";
const FILE_GONE: &str = "Couldn't read that image. Its file may have moved.";
const AUTO_SAVE_FAILED: &str =
    "Couldn't auto-save. The capture is still here, unsaved. Check the Save to folder.";

pub(super) type CopyResult = Result<(), Box<dyn Error + Send + Sync>>;

impl MainWindow {
    /// Where every capture enters the app, whichever way it was taken.
    pub fn add_capture(&mut self, image: RgbaImage, mode: CaptureMode, cx: &mut Context<Self>) {
        let taken_at = platform::local_now();
        let pattern = self.settings.name_pattern.clone();
        let copy = self.settings.copy_on_capture;
        let auto_save = self.auto_save_target();
        let store = self.store.clone();
        let session = self.session;
        let thumbnail_width = self.gallery.read(cx).thumbnail_width();
        self.enqueue(cx, async move |this, cx| {
            let (stored, auto_saved, image) = cx
                .background_spawn(async move {
                    let shot = Shot {
                        mode,
                        taken_at,
                        thumbnail_width,
                    };
                    let mut stored = store_capture(&store, session, &image, shot, &pattern);
                    let auto_saved = match (&mut stored, auto_save) {
                        (Ok(card), Some(target)) => auto_save_card(&store, card, target),
                        _ => Ok(()),
                    };
                    (stored, auto_saved, image)
                })
                .await;
            let kept = stored.is_ok() && auto_saved.is_ok();
            this.update(cx, |this, cx| {
                match stored {
                    Ok(card) => this.show_card(card, cx),
                    Err(_) => this.show_toast(Toast::failed(STORE_FAILED), cx),
                }
                if auto_saved.is_err() {
                    this.show_toast(Toast::failed(AUTO_SAVE_FAILED), cx);
                }
            })
            .ok();
            // The clipboard doesn't depend on the store, so a capture that couldn't be kept can
            // still be pasted. A failure from above stays up unless the copy failed too.
            if copy {
                let copied = cx.background_spawn(async move { copy_image(image) }).await;
                if kept || copied.is_err() {
                    this.update(cx, |this, cx| this.report_copy(copied, cx))
                        .ok();
                }
            }
        });
    }

    /// Captures the chosen display. Toru's own windows are taken out of what captures see for the
    /// moment it takes, without moving, so nothing flickers and there's nothing to bring back.
    pub(super) fn capture_full_screen(&mut self, cx: &mut Context<Self>) {
        if self.capturing {
            return;
        }
        let (displays, chosen) = self.displays(cx);
        let Some(display) = displays.get(chosen) else {
            self.show_toast(Toast::failed(CAPTURE_FAILED), cx);
            return;
        };
        // `xcap`'s monitor id is GPUI's `DisplayId`, cut to 32 bits (`docs/spikes.md` §1.2).
        let monitor_id = u64::from(display.id) as u32;
        self.capturing = true;
        platform::exclude_windows_from_capture(true);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(platform::EXCLUDE_SETTLE)
                .await;
            let frame = cx
                .background_spawn(async move { capture_monitor(monitor_id) })
                .await;
            platform::exclude_windows_from_capture(false);
            this.update(cx, |this, cx| {
                this.capturing = false;
                match frame {
                    Some(image) => this.add_capture(image, CaptureMode::FullScreen, cx),
                    None => this.show_toast(Toast::failed(CAPTURE_FAILED), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    /// A card's Copy: the full image, read back from where it is kept, on the clipboard. Where
    /// that is gets asked when the job runs, since a Save queued ahead of it moves the image.
    pub(super) fn copy_card(&mut self, card: usize, capture: CaptureId, cx: &mut Context<Self>) {
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let copied = cx
                .background_spawn(async move {
                    let image = image::open(store.full_image_path(capture).ok()?).ok()?;
                    Some(copy_image(image.to_rgba8()))
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(copied) = copied else {
                    this.show_toast(Toast::failed(FILE_GONE), cx);
                    return;
                };
                if copied.is_ok() {
                    this.gallery
                        .update(cx, |gallery, cx| gallery.show_copied(card, cx));
                }
                this.report_copy(copied, cx);
            })
            .ok();
        });
    }

    /// Takes a typed name to the store, which refuses anything invalid or already used.
    pub(super) fn rename_capture(
        &mut self,
        capture: CaptureId,
        name: String,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let wanted = name.clone();
            let renamed = cx
                .background_spawn(async move { store.rename(capture, &wanted) })
                .await;
            this.update(cx, |this, cx| {
                this.finish_rename(capture, name, renamed, cx)
            })
            .ok();
        });
    }

    fn finish_rename(
        &mut self,
        capture: CaptureId,
        name: String,
        renamed: StoreResult<()>,
        cx: &mut Context<Self>,
    ) {
        let gallery = self.gallery.clone();
        match renamed {
            Ok(()) => {
                let saved =
                    gallery.update(cx, |gallery, cx| gallery.accept_name(capture, name, cx));
                if saved {
                    self.show_toast(
                        Toast::new("Renamed here. The saved file keeps its name."),
                        cx,
                    );
                }
            }
            Err(error) => match refusal(&error) {
                Some(message) => {
                    self.window
                        .update(cx, |_, window, cx| {
                            gallery.update(cx, |gallery, cx| {
                                gallery.refuse_name(capture, message, window, cx)
                            })
                        })
                        .ok();
                }
                None => self.show_toast(Toast::failed(RENAME_FAILED), cx),
            },
        }
    }

    /// Queues a change to the store that only needs mentioning if it fails.
    pub(super) fn write_store(
        &mut self,
        failure: impl Into<SharedString>,
        cx: &mut Context<Self>,
        write: impl FnOnce(&Store) -> StoreResult<()> + Send + 'static,
    ) {
        let failure = failure.into();
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let written = cx.background_spawn(async move { write(&store) }).await;
            if written.is_err() {
                this.update(cx, |this, cx| this.show_toast(Toast::failed(failure), cx))
                    .ok();
            }
        });
    }

    pub(super) fn report_copy(&mut self, copied: CopyResult, cx: &mut Context<Self>) {
        let toast = match copied {
            Ok(()) => Toast::new(cx.theme().strings.copied),
            Err(_) => Toast::failed(COPY_FAILED),
        };
        self.show_toast(toast, cx);
    }

    fn auto_save_target(&self) -> Option<AutoSave> {
        self.settings.auto_save.then(|| AutoSave {
            folder: self.save_folder(),
            format: self.settings.format,
        })
    }

    /// Runs `job` once everything queued before it has finished.
    pub(super) fn enqueue(
        &mut self,
        cx: &mut Context<Self>,
        job: impl AsyncFnOnce(WeakEntity<Self>, &mut AsyncApp) + 'static,
    ) {
        let previous = self.queue.take();
        self.queue = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            job(this, cx).await;
        }));
    }

    fn show_card(&mut self, card: NewCard, cx: &mut Context<Self>) {
        let gallery = self.gallery.clone();
        self.window
            .update(cx, |_, window, cx| {
                gallery.update(cx, |gallery, cx| gallery.add_session_card(card, window, cx))
            })
            .ok();
    }
}

struct Shot {
    mode: CaptureMode,
    taken_at: LocalDateTime,
    thumbnail_width: u32,
}

/// Where the Auto-save setting sends a new capture. No folder means Windows can't say where
/// Pictures is, which fails like any other unreachable folder.
struct AutoSave {
    folder: Option<PathBuf>,
    format: SaveFormat,
}

/// Saves a capture the moment it is stored. On failure the capture stays stored and unsaved.
fn auto_save_card(store: &Store, card: &mut NewCard, target: AutoSave) -> StoreResult<()> {
    let folder = target
        .folder
        .ok_or_else(|| StoreError::Io(std::io::ErrorKind::NotFound.into()))?;
    if let Some(file) = save_capture(store, card.files.id, &folder, target.format)? {
        card.bytes = file.bytes;
        card.capture.saved_path = Some(file.path);
    }
    Ok(())
}

/// What to tell the user about a name the store turned down, shown on the card. `None` for a
/// failure that has nothing to do with the name.
fn refusal(error: &StoreError) -> Option<String> {
    match error {
        StoreError::InvalidName(NameError::Empty) => Some("Type a name.".to_string()),
        StoreError::InvalidName(error) => Some(pattern_message(error)),
        StoreError::NameTaken => Some("Another capture that day has this name.".to_string()),
        _ => None,
    }
}

/// Names the capture and stores it. Must only run from the queue, so that the name it picks can't
/// be taken before the capture is in the store.
fn store_capture(
    store: &Store,
    session: SessionId,
    image: &RgbaImage,
    shot: Shot,
    pattern: &str,
) -> StoreResult<NewCard> {
    let date = shot.taken_at.date;
    let taken = store.names_on(date)?;
    let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
    let capture = Capture {
        name: next_free_name(pattern, date, &taken),
        mode: shot.mode,
        width: image.width(),
        height: image.height(),
        taken_at: shot.taken_at,
        saved_path: None,
    };
    let id = store.add_capture(session, image, &capture, shot.thumbnail_width)?;
    let bytes = fs::metadata(store.image_path(id))?.len();
    Ok(NewCard {
        files: CardFiles {
            id,
            thumbnail: store.thumbnail_path(id),
        },
        capture,
        bytes,
    })
}

fn capture_monitor(id: u32) -> Option<RgbaImage> {
    Monitor::all()
        .ok()?
        .into_iter()
        .find(|monitor| monitor.id().is_ok_and(|monitor_id| monitor_id == id))?
        .capture_image()
        .ok()
}

/// `set_image` fails if another window holds the clipboard open, which is the failure the message
/// covers. It already retries for about 150 ms first (`docs/spikes.md` §1.4).
fn copy_image(image: RgbaImage) -> CopyResult {
    ClipboardContext::new()?.set_image(RustImageData::from_dynamic_image(DynamicImage::ImageRgba8(
        image,
    )))
}

#[cfg(test)]
pub(super) mod tests {
    use std::{env, path::Path, process};

    use toru_core::{
        naming::DEFAULT_PATTERN,
        store::StoreError,
        time::{LocalDate, LocalDateTime},
    };
    use xcap::image::Rgba;

    use super::*;

    const TAKEN: LocalDateTime = LocalDateTime {
        date: LocalDate {
            year: 2026,
            month: 11,
            day: 8,
        },
        hour: 14,
        minute: 31,
    };
    const THUMBNAIL_WIDTH: u32 = 32;
    const BASE: &str = "screenshot_11_08_2026";

    pub(in super::super) struct TestDir(pub(in super::super) PathBuf);

    impl TestDir {
        pub(in super::super) fn new(name: &str) -> TestDir {
            TestDir(env::temp_dir().join(format!("toru-captures-{}-{name}", process::id())))
        }

        pub(in super::super) fn store(&self) -> Store {
            Store::open(&self.0).unwrap()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn frame() -> RgbaImage {
        RgbaImage::from_fn(64, 40, |x, y| Rgba([x as u8, y as u8, 200, 255]))
    }

    fn shot() -> Shot {
        Shot {
            mode: CaptureMode::FullScreen,
            taken_at: TAKEN,
            thumbnail_width: THUMBNAIL_WIDTH,
        }
    }

    pub(in super::super) fn add(
        store: &Store,
        session: SessionId,
        pattern: &str,
    ) -> StoreResult<NewCard> {
        store_capture(store, session, &frame(), shot(), pattern)
    }

    fn png_dimensions(path: &Path) -> (u32, u32) {
        let image = image::open(path).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn a_capture_becomes_a_card_with_its_files_and_record() {
        let dir = TestDir::new("card");
        let store = dir.store();
        let session = store.new_session().unwrap();

        let card = add(&store, session, DEFAULT_PATTERN).unwrap();

        assert_eq!(card.capture.name, BASE);
        assert_eq!(card.capture.mode, CaptureMode::FullScreen);
        assert_eq!((card.capture.width, card.capture.height), (64, 40));
        let image_path = store.image_path(card.files.id);
        assert_eq!(card.bytes, fs::metadata(&image_path).unwrap().len());
        assert_eq!(image::open(&image_path).unwrap().to_rgba8(), frame());
        assert_eq!(png_dimensions(&card.files.thumbnail), (32, 20));

        let listed = store.session_captures(session).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, card.files.id);
        assert_eq!(listed[0].capture, card.capture);
    }

    #[test]
    fn later_captures_that_day_are_numbered() {
        let dir = TestDir::new("numbered");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let names: Vec<String> = (0..3)
            .map(|_| add(&store, session, DEFAULT_PATTERN).unwrap().capture.name)
            .collect();
        assert_eq!(names, [BASE, &format!("{BASE}_2"), &format!("{BASE}_3")]);
    }

    #[test]
    fn numbering_counts_every_session_that_day() {
        let dir = TestDir::new("sessions");
        let store = dir.store();
        let first = store.new_session().unwrap();
        add(&store, first, DEFAULT_PATTERN).unwrap();
        let second = store.new_session().unwrap();
        let card = add(&store, second, DEFAULT_PATTERN).unwrap();
        assert_eq!(card.capture.name, format!("{BASE}_2"));
    }

    #[test]
    fn the_setting_decides_the_name() {
        let dir = TestDir::new("pattern");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let card = add(&store, session, "shot-{yyyy}").unwrap();
        assert_eq!(card.capture.name, "shot-2026");
    }

    fn target(folder: Option<PathBuf>) -> AutoSave {
        AutoSave {
            folder,
            format: SaveFormat::Png,
        }
    }

    #[test]
    fn auto_save_writes_the_file_and_the_card_starts_saved() {
        let dir = TestDir::new("auto");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let folder = dir.0.join("Screenshots");
        let mut card = add(&store, session, DEFAULT_PATTERN).unwrap();

        auto_save_card(&store, &mut card, target(Some(folder.clone()))).unwrap();

        let path = folder.join(format!("{BASE}.png"));
        assert_eq!(card.capture.saved_path, Some(path.clone()));
        assert_eq!(card.bytes, fs::metadata(&path).unwrap().len());
        assert_eq!(image::open(path).unwrap().to_rgba8(), frame());
    }

    #[test]
    fn a_failed_auto_save_keeps_the_capture_unsaved() {
        let dir = TestDir::new("auto-failed");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let blocker = dir.0.join("blocker");
        fs::write(&blocker, b"not a folder").unwrap();
        let mut card = add(&store, session, DEFAULT_PATTERN).unwrap();

        assert!(auto_save_card(&store, &mut card, target(Some(blocker.join("x")))).is_err());
        assert!(auto_save_card(&store, &mut card, target(None)).is_err());

        assert_eq!(card.capture.saved_path, None);
        assert!(store.image_path(card.files.id).is_file());
        assert_eq!(
            store.session_captures(session).unwrap()[0].capture,
            card.capture
        );
    }

    #[test]
    fn refused_names_get_a_message_and_other_failures_do_not() {
        for (error, message) in [
            (NameError::Empty, "Type a name."),
            (NameError::ReservedCharacter(':'), "Can't use : in a name."),
            (
                NameError::ReservedDeviceName,
                "That's a reserved Windows name.",
            ),
        ] {
            assert_eq!(refusal(&StoreError::InvalidName(error)).unwrap(), message);
        }
        assert_eq!(
            refusal(&StoreError::NameTaken).unwrap(),
            "Another capture that day has this name."
        );
        assert_eq!(refusal(&StoreError::NotFound), None);
    }

    #[test]
    fn a_frame_too_small_to_store_is_an_error() {
        let dir = TestDir::new("empty");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let result = store_capture(
            &store,
            session,
            &RgbaImage::new(0, 0),
            shot(),
            DEFAULT_PATTERN,
        );
        assert!(matches!(result, Err(StoreError::BadDimensions)));
    }
}
