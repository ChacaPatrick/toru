use std::{path::Path, sync::Arc};

use gpui::{Hsla, RenderImage, Rgba};
use tiny_skia::{Color, IntSize, Pixmap};
use xcap::image::{Frame, RgbaImage};

/// A `tiny-skia` bitmap as GPUI draws it. `tiny-skia` holds premultiplied RGBA and GPUI wants
/// BGRA, so the channels are swapped on the way; the picture is opaque, so premultiplying
/// changed nothing.
pub fn render_image(pixmap: &Pixmap) -> Arc<RenderImage> {
    let mut data = pixmap.data().to_vec();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let buffer = RgbaImage::from_raw(pixmap.width(), pixmap.height(), data)
        .expect("a pixmap's data is width × height × 4 bytes");
    Arc::new(RenderImage::new([Frame::new(buffer)]))
}

/// A `tiny-skia` bitmap as an image to encode or copy. The picture is opaque, so premultiplying
/// changed nothing.
pub fn to_rgba(pixmap: &Pixmap) -> RgbaImage {
    RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data().to_vec())
        .expect("a pixmap's data is width × height × 4 bytes")
}

/// A theme colour as `tiny-skia` takes it.
pub fn skia_color(color: Hsla) -> Color {
    let Rgba { r, g, b, a } = color.into();
    Color::from_rgba(r, g, b, a).expect("GPUI colours are within 0..=1")
}

/// A capture's file as the document's base, or `None` if it can't be read. Screenshots are
/// opaque, so the pixels are already what `tiny-skia`'s premultiplied form expects.
pub fn load_picture(path: &Path) -> Option<Pixmap> {
    pixmap_from_rgba(xcap::image::open(path).ok()?.to_rgba8())
}

pub(super) fn pixmap_from_rgba(image: RgbaImage) -> Option<Pixmap> {
    let size = IntSize::from_wh(image.width(), image.height())?;
    let mut data = image.into_raw();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Pixmap::from_vec(data, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_render_image_is_bgra() {
        let mut base = Pixmap::new(1, 1).unwrap();
        base.fill(Color::from_rgba8(10, 20, 30, 255));
        let image = render_image(&base);
        assert_eq!(image.size(0), gpui::size(1.into(), 1.into()));
        // The frame's bytes are BGRA: blue first.
        let mut swapped = base.data().to_vec();
        swapped.swap(0, 2);
        assert_eq!(swapped, [30, 20, 10, 255]);
    }

    #[test]
    fn theme_colours_convert_channel_for_channel() {
        let color = skia_color(gpui::rgb(0x2b2a26).into());
        let rgba: Rgba = gpui::Hsla::from(gpui::rgb(0x2b2a26)).into();
        assert!((color.red() - rgba.r).abs() < 1e-6);
        assert!((color.blue() - rgba.b).abs() < 1e-6);
        assert_eq!(color.alpha(), 1.);
    }

    #[test]
    fn a_capture_frame_becomes_an_opaque_pixmap() {
        let image = RgbaImage::from_fn(2, 1, |x, _| xcap::image::Rgba([x as u8, 20, 30, 0]));
        let pixmap = pixmap_from_rgba(image).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (2, 1));
        assert_eq!(pixmap.data(), [0, 20, 30, 255, 1, 20, 30, 255]);
        assert!(pixmap_from_rgba(RgbaImage::new(0, 0)).is_none());
    }
}
