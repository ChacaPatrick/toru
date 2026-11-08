//! What the editor edits: the capture's pixels, untouched, plus the list of annotations laid
//! over them, with undo and redo over that list. `render` draws both into one bitmap with
//! `tiny-skia`, and that same bitmap is what the canvas shows and what Save (8.5) writes, so the
//! screen and the file can't differ.

use std::{path::Path, sync::Arc};

use gpui::{Hsla, Point, RenderImage, Rgba, Size, point, size};
use tiny_skia::{
    BlendMode, Color, FilterQuality, IntRect, IntSize, Pixmap, PixmapPaint, Transform,
};
use xcap::image::{Frame, RgbaImage};

use super::annotation::Annotation;

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

    /// Changes the value as part of the last step, so undo takes both back together: a slider
    /// drag changes the value on every move but is one step.
    pub fn amend(&mut self, change: impl FnOnce(&mut T)) {
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

    /// The marks on the picture, bottom to top.
    pub fn annotations(&self) -> &[Annotation] {
        self.annotations.get()
    }

    /// Puts `annotation` on top, as one undoable step. Returns its index.
    pub fn add(&mut self, annotation: Annotation) -> usize {
        self.annotations.edit(|all| all.push(annotation));
        self.annotations.get().len() - 1
    }

    /// Replaces the mark at `index`, as one undoable step. Returns whether anything changed:
    /// nothing happens if the mark is already so.
    pub fn replace(&mut self, index: usize, annotation: Annotation) -> bool {
        if index >= self.annotations.get().len() || self.annotations.get()[index] == annotation {
            return false;
        }
        self.annotations.edit(|all| all[index] = annotation);
        true
    }

    /// Replaces the mark at `index` within the last undo step, for the later moves of a slider
    /// drag whose first move was a `replace`.
    pub fn amend(&mut self, index: usize, annotation: Annotation) {
        if index < self.annotations.get().len() {
            self.annotations.amend(|all| all[index] = annotation);
        }
    }

    /// Takes the mark at `index` off, as one undoable step.
    pub fn remove(&mut self, index: usize) {
        if index < self.annotations.get().len() {
            self.annotations.edit(|all| {
                all.remove(index);
            });
        }
    }

    /// The topmost mark within `tolerance` picture pixels of `at`, as its index.
    pub fn hit_test(&self, at: Point<f32>, tolerance: f32) -> Option<usize> {
        self.annotations
            .get()
            .iter()
            .rposition(|annotation| annotation.hit(at, tolerance))
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
        self.render_with(None, None)
    }

    /// The picture as `render` draws it, but for a drag's preview: the mark at `index` is left
    /// out, or drawn as `preview` in its place, under the same marks as before, so a mark being
    /// moved or resized keeps its depth. A `preview` with no `index` is a mark being drawn, on
    /// top. Committing the preview to the document renders the same pixels, so nothing shifts
    /// on release.
    pub fn render_with(&self, index: Option<usize>, preview: Option<&Annotation>) -> Pixmap {
        let mut picture = self.base.clone();
        self.draw_marks(&mut picture, point(0, 0), index, preview);
        picture
    }

    /// The pixels in `region` of what `render_with` would draw, as a picture of their own, so a
    /// drag only redraws where the mark is. `None` if the region misses the picture.
    pub fn render_region(
        &self,
        region: IntRect,
        index: Option<usize>,
        preview: Option<&Annotation>,
    ) -> Option<(IntRect, Pixmap)> {
        let whole = IntRect::from_xywh(0, 0, self.width(), self.height())?;
        let region = region.intersect(&whole)?;
        let mut picture = Pixmap::new(region.width(), region.height())?;
        picture.draw_pixmap(
            -region.x(),
            -region.y(),
            self.base.as_ref(),
            &PixmapPaint {
                opacity: 1.,
                blend_mode: BlendMode::Source,
                quality: FilterQuality::Nearest,
            },
            Transform::identity(),
            None,
        );
        self.draw_marks(&mut picture, point(region.x(), region.y()), index, preview);
        Some((region, picture))
    }

    /// Draws the marks, bottom to top, on `picture`, whose top-left pixel is picture pixel
    /// `origin`, with the drag's preview as `render_with` describes.
    fn draw_marks(
        &self,
        picture: &mut Pixmap,
        origin: Point<i32>,
        index: Option<usize>,
        preview: Option<&Annotation>,
    ) {
        for (i, annotation) in self.annotations.get().iter().enumerate() {
            if Some(i) == index {
                if let Some(preview) = preview {
                    preview.draw_at(picture, origin);
                }
            } else {
                annotation.draw_at(picture, origin);
            }
        }
        if index.is_none()
            && let Some(preview) = preview
        {
            preview.draw_at(picture, origin);
        }
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

    fn red_line(from: f32, to: f32, y: f32) -> Annotation {
        use super::super::annotation::{Shape, Style};
        use super::super::tools::{LineStyle, Width};
        let mut line = Annotation::new(
            Shape::Line,
            gpui::point(from, y),
            Style {
                color: Color::from_rgba8(255, 0, 0, 255),
                second: Color::WHITE,
                width: Width::Medium,
                line: LineStyle::Solid,
                opacity: 1.,
            },
        );
        line.extend(gpui::point(to, y), false, gpui::size(100., 100.));
        line
    }

    #[test]
    fn adding_replacing_and_removing_are_each_one_undo_step() {
        let mut document = Document::new(Pixmap::new(40, 20).unwrap());
        let index = document.add(red_line(0., 40., 10.));
        assert_eq!(index, 0);
        assert!(document.is_edited());
        assert!(document.replace(0, red_line(0., 40., 5.)));
        assert_eq!(document.annotations()[0].points[0].y, 5.);
        // Replacing with the same mark is not an edit, nor is replacing what isn't there.
        assert!(!document.replace(0, red_line(0., 40., 5.)));
        assert!(!document.replace(1, red_line(0., 40., 5.)));
        document.remove(0);
        assert!(!document.is_edited());
        assert!(document.undo());
        assert!(document.undo());
        assert_eq!(document.annotations()[0].points[0].y, 10.);
        assert!(document.undo());
        assert!(!document.undo());
        assert!(document.redo());
        assert!(document.redo());
        assert!(document.redo());
        assert!(!document.is_edited());
    }

    #[test]
    fn hit_testing_finds_the_topmost_mark() {
        let mut document = Document::new(Pixmap::new(40, 20).unwrap());
        document.add(red_line(0., 40., 10.));
        document.add(red_line(0., 40., 12.));
        assert_eq!(document.hit_test(gpui::point(20., 11.), 2.), Some(1));
        assert_eq!(document.hit_test(gpui::point(20., 8.), 1.), Some(0));
        assert_eq!(document.hit_test(gpui::point(20., 2.), 1.), None);
    }

    #[test]
    fn amending_folds_into_the_last_step() {
        let mut document = Document::new(Pixmap::new(40, 20).unwrap());
        document.add(red_line(0., 40., 10.));
        assert!(document.replace(0, red_line(0., 40., 5.)));
        document.amend(0, red_line(0., 40., 7.));
        document.amend(0, red_line(0., 40., 8.));
        assert_eq!(document.annotations()[0].points[0].y, 8.);
        assert!(document.undo());
        assert_eq!(document.annotations()[0].points[0].y, 10.);
        assert!(document.redo());
        assert_eq!(document.annotations()[0].points[0].y, 8.);
        // Amending past the end does nothing.
        document.amend(3, red_line(0., 40., 1.));
        assert_eq!(document.annotations().len(), 1);
    }

    #[test]
    fn a_previewed_mark_keeps_its_place_under_the_marks_above_it() {
        let mut base = Pixmap::new(40, 20).unwrap();
        base.fill(Color::WHITE);
        let mut document = Document::new(base);
        let mut green = red_line(0., 40., 10.);
        green.style.color = Color::from_rgba8(0, 255, 0, 255);
        document.add(green);
        document.add(red_line(0., 40., 12.));
        // The green line dragged onto the red one stays beneath it: the overlap is red.
        let moved = {
            let mut line = red_line(0., 40., 12.);
            line.style.color = Color::from_rgba8(0, 255, 0, 255);
            line
        };
        let preview = document.render_with(Some(0), Some(&moved));
        let overlap = preview.pixel(20, 12).unwrap();
        assert!(overlap.red() > 200 && overlap.green() < 50);
        document.replace(0, moved);
        assert_eq!(preview.data(), document.render().data());
        // Left out, the mark is gone and what was under it shows.
        let without = document.render_with(Some(1), None);
        let shown = without.pixel(20, 12).unwrap();
        assert!(shown.green() > 200 && shown.red() < 50);
    }

    #[test]
    fn a_region_matches_the_whole_render() {
        let mut base = Pixmap::new(40, 20).unwrap();
        for x in 0..40 {
            for y in 0..20 {
                let c = Color::from_rgba8(x as u8 * 6, y as u8 * 12, 128, 255);
                base.fill_rect(
                    tiny_skia::Rect::from_xywh(x as f32, y as f32, 1., 1.).unwrap(),
                    &tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(c),
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
        }
        let mut document = Document::new(base);
        document.add(red_line(0., 40., 10.));
        let drawing = red_line(5., 30., 14.);
        let whole = document.render_with(None, Some(&drawing));
        let (region, part) = document
            .render_region(
                IntRect::from_ltrb(-5, 8, 25, 30).unwrap(),
                None,
                Some(&drawing),
            )
            .unwrap();
        assert_eq!(region, IntRect::from_ltrb(0, 8, 25, 20).unwrap());
        for y in 0..region.height() {
            for x in 0..region.width() {
                assert_eq!(
                    part.pixel(x, y).unwrap(),
                    whole.pixel(x, y + 8).unwrap(),
                    "pixel ({x}, {y})"
                );
            }
        }
        assert!(
            document
                .render_region(IntRect::from_xywh(50, 0, 5, 5).unwrap(), None, None)
                .is_none()
        );
    }

    #[test]
    fn a_preview_renders_the_same_pixels_as_the_committed_mark() {
        let mut base = Pixmap::new(40, 20).unwrap();
        base.fill(Color::WHITE);
        let mut document = Document::new(base);
        document.add(red_line(0., 40., 10.));
        let moved = red_line(0., 40., 15.);
        let preview = document.render_with(Some(0), Some(&moved));
        document.replace(0, moved);
        assert_eq!(preview.data(), document.render().data());
        assert!(preview.pixel(20, 15).unwrap().red() > 200);
        assert_eq!(preview.pixel(20, 10).unwrap().red(), 255);
        assert_eq!(preview.pixel(20, 10).unwrap().green(), 255);
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
