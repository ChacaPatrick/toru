//! What the editor edits: the capture's pixels, untouched, plus the list of annotations laid
//! over them, with undo and redo over that list. `render` draws both into one bitmap with
//! `tiny-skia`, and that same bitmap is what the canvas shows and what Save (8.5) writes, so the
//! screen and the file can't differ.

use std::{path::Path, sync::Arc};

use gpui::{RenderImage, Size, size};
use tiny_skia::{IntSize, Pixmap};
use xcap::image::{Frame, RgbaImage};

/// Undo and redo over a value: every edit keeps the value before it.
pub struct History<T> {
    present: T,
    past: Vec<T>,
    future: Vec<T>,
}

impl<T: Clone> History<T> {
    pub fn new(present: T) -> History<T> {
        History {
            present,
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    pub fn get(&self) -> &T {
        &self.present
    }

    /// Changes the value as one undoable step. Anything redoable is forgotten, as in any editor.
    pub fn edit(&mut self, change: impl FnOnce(&mut T)) {
        self.past.push(self.present.clone());
        self.future.clear();
        change(&mut self.present);
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Steps back; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.past.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.present, previous);
        self.future.push(current);
        true
    }

    /// Steps forward again; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.present, next);
        self.past.push(current);
        true
    }
}

/// One mark on the picture. Sections 8.2 to 8.4 add the shapes; each knows how to draw itself
/// onto a `Pixmap` in `Document::render`.
#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {}

impl Annotation {
    fn draw(&self, _picture: &mut Pixmap) {
        match *self {}
    }
}

pub struct Document {
    base: Pixmap,
    annotations: History<Vec<Annotation>>,
}

impl Document {
    pub fn new(base: Pixmap) -> Document {
        Document {
            base,
            annotations: History::new(Vec::new()),
        }
    }

    pub fn width(&self) -> u32 {
        self.base.width()
    }

    pub fn height(&self) -> u32 {
        self.base.height()
    }

    pub fn size(&self) -> Size<f32> {
        size(self.width() as f32, self.height() as f32)
    }

    /// Whether any annotation is on the picture.
    pub fn is_edited(&self) -> bool {
        !self.annotations.get().is_empty()
    }

    pub fn can_undo(&self) -> bool {
        self.annotations.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.annotations.can_redo()
    }

    pub fn undo(&mut self) -> bool {
        self.annotations.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.annotations.redo()
    }

    /// Reset edits: takes every annotation off, as one undoable step. `false` when there were
    /// none.
    pub fn reset(&mut self) -> bool {
        if !self.is_edited() {
            return false;
        }
        self.annotations.edit(Vec::clear);
        true
    }

    /// The picture with its annotations, as the canvas shows it and a save writes it.
    pub fn render(&self) -> Pixmap {
        let mut picture = self.base.clone();
        for annotation in self.annotations.get() {
            annotation.draw(&mut picture);
        }
        picture
    }
}

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

/// A capture's file as the document's base, or `None` if it can't be read. Screenshots are
/// opaque, so the pixels are already what `tiny-skia`'s premultiplied form expects.
pub fn load_picture(path: &Path) -> Option<Pixmap> {
    pixmap_from_rgba(xcap::image::open(path).ok()?.to_rgba8())
}

fn pixmap_from_rgba(image: RgbaImage) -> Option<Pixmap> {
    let size = IntSize::from_wh(image.width(), image.height())?;
    let mut data = image.into_raw();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Pixmap::from_vec(data, size)
}

#[cfg(test)]
mod tests {
    use tiny_skia::Color;

    use super::*;

    #[test]
    fn edits_undo_and_redo_in_order() {
        let mut history = History::new(vec![1]);
        history.edit(|v| v.push(2));
        history.edit(|v| v.push(3));
        assert_eq!(history.get(), &[1, 2, 3]);
        assert!(history.undo());
        assert_eq!(history.get(), &[1, 2]);
        assert!(history.undo());
        assert_eq!(history.get(), &[1]);
        assert!(!history.undo());
        assert!(history.redo());
        assert!(history.redo());
        assert_eq!(history.get(), &[1, 2, 3]);
        assert!(!history.redo());
    }

    #[test]
    fn an_edit_after_undo_drops_the_redo_branch() {
        let mut history = History::new(0);
        history.edit(|v| *v = 1);
        history.edit(|v| *v = 2);
        assert!(history.undo());
        history.edit(|v| *v = 9);
        assert!(!history.can_redo());
        assert!(history.undo());
        assert_eq!(*history.get(), 1);
    }

    #[test]
    fn a_fresh_document_is_unedited_with_nothing_to_undo() {
        let document = Document::new(Pixmap::new(4, 3).unwrap());
        assert_eq!((document.width(), document.height()), (4, 3));
        assert!(!document.is_edited());
        assert!(!document.can_undo());
        assert!(!document.can_redo());
    }

    #[test]
    fn reset_does_nothing_on_an_unedited_document() {
        let mut document = Document::new(Pixmap::new(2, 2).unwrap());
        assert!(!document.reset());
        assert!(!document.undo());
        assert!(!document.redo());
    }

    #[test]
    fn render_keeps_the_base_pixels() {
        let mut base = Pixmap::new(2, 1).unwrap();
        base.fill(Color::from_rgba8(10, 20, 30, 255));
        let document = Document::new(base.clone());
        assert_eq!(document.render().data(), base.data());
    }

    #[test]
    fn the_render_image_is_bgra() {
        let mut base = Pixmap::new(1, 1).unwrap();
        base.fill(Color::from_rgba8(10, 20, 30, 255));
        let image = render_image(&base);
        assert_eq!(image.size(0), size(1.into(), 1.into()));
        // The frame's bytes are BGRA: blue first.
        let mut swapped = base.data().to_vec();
        swapped.swap(0, 2);
        assert_eq!(swapped, [30, 20, 10, 255]);
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
