//! The custom palette's background photo behind the main window: read and scaled on a
//! background thread, drawn to cover the window under a scrim of the background colour, and
//! let go of on the GPU when the theme stops asking for it. A photo that can't be read is noted
//! and the window shows the plain background, with no dialog, so a moved file can't nag at
//! every launch.

use std::{path::PathBuf, sync::Arc};

use gpui::{
    AnyElement, Context, ObjectFit, RenderImage, StyledImage, Window, div, img, prelude::*,
};
use xcap::image::{
    DynamicImage, Frame, GenericImageView, ImageDecoder, ImageReader, imageops::FilterType,
    metadata::Orientation,
};

use super::MainWindow;
use crate::theme::{ActiveTheme, backdrop_scrim};

/// The longest side a photo is kept at: enough to cover the window sharply, without putting a
/// camera-sized picture in the GPU atlas.
const MAX_SIDE: u32 = 2560;

/// What the window has for the theme's photo: the path it was asked for and, if it read, the
/// picture. A `None` picture means the file couldn't be read, and isn't retried until the path
/// changes.
pub struct Backdrop {
    path: PathBuf,
    image: Option<Arc<RenderImage>>,
}

impl MainWindow {
    /// Called each frame: starts reading the theme's photo when it changes, and takes the old
    /// one out of the GPU. The read runs on a background thread and the picture arrives in a
    /// later frame.
    pub(super) fn sync_backdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for image in self.stale_backdrops.drain(..) {
            window.drop_image(image).ok();
        }
        let wanted = cx.theme().background.clone();
        let shown = self.backdrop.as_ref().map(|backdrop| &backdrop.path);
        if shown == wanted.as_ref() || (wanted.is_some() && self.backdrop_loading == wanted) {
            return;
        }
        if let Some(Backdrop {
            image: Some(image), ..
        }) = self.backdrop.take()
        {
            window.drop_image(image).ok();
        }
        self.backdrop_loading = wanted.clone();
        let Some(path) = wanted else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_spawn({
                    let path = path.clone();
                    async move { load(&path) }
                })
                .await;
            this.update(cx, |this, cx| {
                // The theme may have moved on while the file was read.
                if this.backdrop_loading.as_ref() != Some(&path) {
                    this.stale_backdrops.extend(image);
                    return;
                }
                this.backdrop_loading = None;
                this.backdrop = Some(Backdrop { path, image });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The photo scaled to cover the window, with the scrim over it, as absolute children laid
    /// under everything else in the frame. Nothing when there is no photo to show.
    pub(super) fn backdrop_layers(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let Some(image) = self
            .backdrop
            .as_ref()
            .and_then(|backdrop| backdrop.image.clone())
        else {
            return Vec::new();
        };
        let bg = cx.theme().colors.bg;
        vec![
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            div()
                .absolute()
                .inset_0()
                .bg(backdrop_scrim(bg))
                .into_any_element(),
        ]
    }
}

/// Reads the photo at `path` as GPUI draws it: turned the way its EXIF orientation says the
/// camera was held, and scaled down to `MAX_SIDE` if it is larger. `None` if the file is
/// missing or isn't a picture.
fn load(path: &std::path::Path) -> Option<Arc<RenderImage>> {
    let mut decoder = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut picture = DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    let (width, height) = picture.dimensions();
    if width.max(height) > MAX_SIDE {
        let scale = MAX_SIDE as f32 / width.max(height) as f32;
        picture = picture.resize(
            (width as f32 * scale).round().max(1.) as u32,
            (height as f32 * scale).round().max(1.) as u32,
            FilterType::Triangle,
        );
    }
    // GPUI wants premultiplied BGRA; the photo is drawn opaque, so only the channel order changes.
    let mut buffer = picture.to_rgba8();
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    Some(Arc::new(RenderImage::new([Frame::new(buffer)])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcap::image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("toru-backdrop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn a_missing_or_unreadable_file_is_none() {
        assert!(load(&temp("not-there.png")).is_none());
        let path = temp("not-a-picture.png");
        std::fs::write(&path, b"plain text").unwrap();
        assert!(load(&path).is_none());
        std::fs::remove_file(path).unwrap();
    }

    /// A JPEG whose EXIF says the camera was turned 90° shows upright: its sides swap.
    #[test]
    fn exif_orientation_is_applied() {
        use xcap::image::{ImageEncoder, codecs::jpeg::JpegEncoder};
        let path = temp("turned.jpg");
        let mut file = std::fs::File::create(&path).unwrap();
        let mut encoder = JpegEncoder::new(&mut file);
        // A little-endian TIFF with one IFD entry: tag 0x0112 Orientation, SHORT, count 1,
        // the value, its padding, and no next IFD.
        let mut exif =
            b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00".to_vec();
        exif.extend_from_slice(&u16::to_le_bytes(u16::from(
            Orientation::Rotate90.to_exif(),
        )));
        exif.extend_from_slice(&[0; 6]);
        encoder.set_exif_metadata(exif).unwrap();
        RgbImage::from_pixel(80, 20, Rgb([10, 20, 30]))
            .write_with_encoder(encoder)
            .unwrap();
        drop(file);
        let image = load(&path).unwrap();
        assert_eq!((image.size(0).width.0, image.size(0).height.0), (20, 80));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_large_photo_is_scaled_to_the_longest_side() {
        let path = temp("wide.png");
        RgbaImage::from_pixel(MAX_SIDE * 2, 100, Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let image = load(&path).unwrap();
        assert_eq!(image.size(0).width.0, MAX_SIDE as i32);
        assert_eq!(image.size(0).height.0, 50);
        std::fs::remove_file(path).unwrap();
    }
}
