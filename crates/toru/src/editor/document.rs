//! What the editor edits: the capture's pixels, untouched, plus the list of annotations laid
//! over them and the crop that chooses what of the picture is kept, with undo and redo over
//! both. `render` draws them into one bitmap with `tiny-skia`, and that same bitmap is what the
//! canvas shows and what Save writes, so the screen and the file can't differ. The crop is a
//! rectangle of the whole picture and the marks stay in the whole picture's pixels, so cropping,
//! and undoing it, moves nothing. A picture edited before opens flattened, with the capture as
//! it was taken held beside it for Reset edits.

use std::{path::Path, sync::Arc};

use gpui::{Hsla, Point, RenderImage, Rgba, Size, point, size};
use tiny_skia::{
    BlendMode, Color, FilterQuality, IntRect, IntSize, Pixmap, PixmapPaint, Transform,
};
use xcap::image::{Frame, RgbaImage};

use toru_core::store::Redaction;

use super::{
    annotation::{self, Annotation, Shape},
    text,
};

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

/// The undoable state: the picture under the marks, the marks, bottom to top, and the crop, if
/// any, as a rectangle of the whole picture. Reset edits changes the picture, so undo can bring
/// back one flattened earlier.
#[derive(Clone)]
pub struct Edits {
    base: Arc<Pixmap>,
    annotations: Vec<Annotation>,
    crop: Option<IntRect>,
}

impl PartialEq for Edits {
    /// A picture is only ever swapped whole, so which one it is says whether it changed.
    fn eq(&self, other: &Edits) -> bool {
        Arc::ptr_eq(&self.base, &other.base)
            && self.annotations == other.annotations
            && self.crop == other.crop
    }
}

impl Edits {
    /// Whether some text holds a character no font can draw, which the picture leaves blank.
    pub fn has_blanks(&self) -> bool {
        self.annotations
            .iter()
            .filter_map(Annotation::text)
            .any(|text| text::has_blanks(&text.shown(), &text.font))
    }
}

/// The result of the edits, taken for Copy, Save or going Back.
#[derive(Clone)]
pub struct Flattened {
    pub picture: Arc<Pixmap>,
    /// The capture as it was taken, for Reset edits.
    pub original: Arc<Pixmap>,
    /// Whether `picture` differs from `original`.
    pub is_edited: bool,
    /// Whether `picture` hides some of the capture under blur, pixelate or solid boxes, so
    /// saving it deletes the original.
    pub redacted: bool,
    /// The state flattened, to say so once it is written.
    pub edits: Edits,
}

pub struct Document {
    /// The capture as it was taken: what Reset edits returns to.
    original: Arc<Pixmap>,
    edits: History<Edits>,
    /// The state the card's picture and thumbnail show, so Back writes only what changed.
    written: Edits,
    /// The state the user's saved file shows, if there is a file and it shows one of these.
    saved: Option<Edits>,
    /// The picture as opened, if it already hides some of the capture.
    redacted_base: Option<Arc<Pixmap>>,
    /// The original was deleted when a picture hiding some of it was saved: no Reset edits.
    original_removed: bool,
}

impl Document {
    #[cfg(test)]
    pub fn new(base: Pixmap) -> Document {
        Document::opened(base, None, false)
    }

    /// `base` is the picture as the card shows it, and `original` the capture as it was taken,
    /// if that was kept apart from it. `in_file` says the saved file shows `base`.
    pub fn opened(base: Pixmap, original: Option<Pixmap>, in_file: bool) -> Document {
        let base = Arc::new(base);
        let original = original.map_or_else(|| base.clone(), Arc::new);
        let start = Edits {
            base,
            annotations: Vec::new(),
            crop: None,
        };
        Document {
            original,
            written: start.clone(),
            saved: in_file.then(|| start.clone()),
            edits: History::new(start),
            redacted_base: None,
            original_removed: false,
        }
    }

    /// What the store says of the capture: whether the picture as opened already hides some of
    /// it, and whether its original is gone.
    pub fn with_redaction(mut self, redaction: Redaction) -> Document {
        self.redacted_base = redaction.redacted.then(|| self.edits.get().base.clone());
        self.original_removed = redaction.original_removed;
        self
    }

    /// Whether the picture as it stands hides some of the capture.
    fn is_redacted(&self) -> bool {
        let edits = self.edits.get();
        self.redacted_base
            .as_ref()
            .is_some_and(|base| Arc::ptr_eq(base, &edits.base))
            || edits
                .annotations
                .iter()
                .any(|mark| matches!(mark.shape, Shape::Block(_)))
    }

    /// Whether Reset edits can go back to the capture as it was taken.
    pub fn can_reset(&self) -> bool {
        !self.original_removed
    }

    /// A save of `picture`, flattened from `saved` and hiding some of the capture, has deleted
    /// the original. Reset edits is off, and undo can't go back past the save, so what was
    /// hidden can't come back. If nothing has changed since, `picture` becomes the picture,
    /// with no marks or crop over it, and `to_card` says the card shows it too (Save, not Save
    /// as). If something has, the saved marks go into the whole picture and what came since
    /// stays over it: a mark changed since is added again on top, its saved self staying in
    /// the picture. After a Reset since, the saved picture is taken as it is, and what came
    /// since goes.
    pub fn forget_original(&mut self, picture: Arc<Pixmap>, saved: &Edits, to_card: bool) {
        self.original_removed = true;
        let present = self.edits.get().clone();
        let start = if present != *saved && Arc::ptr_eq(&present.base, &saved.base) {
            self.edits.present = Edits {
                crop: None,
                ..saved.clone()
            };
            let baked = Arc::new(self.render());
            Edits {
                base: baked,
                annotations: (present.annotations.into_iter())
                    .filter(|mark| !saved.annotations.contains(mark))
                    .collect(),
                crop: present.crop,
            }
        } else {
            let start = Edits {
                base: picture,
                annotations: Vec::new(),
                crop: None,
            };
            if to_card && present == *saved {
                self.written = start.clone();
                self.saved = Some(start.clone());
            }
            start
        };
        self.original = start.base.clone();
        self.redacted_base = Some(start.base.clone());
        self.edits = History::new(start);
    }

    fn base(&self) -> &Pixmap {
        &self.edits.get().base
    }

    /// The width of what is shown and saved: the crop's, or the whole picture's.
    pub fn width(&self) -> u32 {
        self.crop().width()
    }

    pub fn height(&self) -> u32 {
        self.crop().height()
    }

    /// The whole picture, as captured.
    pub fn whole(&self) -> IntRect {
        IntRect::from_xywh(0, 0, self.base().width(), self.base().height())
            .expect("a pixmap is at least one pixel")
    }

    /// The whole picture's size in picture pixels: what a mark may be moved within, crop or no.
    pub fn size(&self) -> Size<f32> {
        size(self.base().width() as f32, self.base().height() as f32)
    }

    /// The part of the picture that is kept: the crop, or the whole picture without one.
    pub fn crop(&self) -> IntRect {
        self.edits.get().crop.unwrap_or_else(|| self.whole())
    }

    /// Whether a crop is set.
    pub fn is_cropped(&self) -> bool {
        self.edits.get().crop.is_some()
    }

    /// Crops to `crop`, or uncrops with `None`, as one undoable step. The rectangle is held
    /// within the picture; one that covers it all is no crop. Returns whether anything changed.
    pub fn set_crop(&mut self, crop: Option<IntRect>) -> bool {
        let crop = crop
            .and_then(|crop| crop.intersect(&self.whole()))
            .filter(|crop| *crop != self.whole());
        if crop == self.edits.get().crop {
            return false;
        }
        self.edits.edit(|edits| edits.crop = crop);
        true
    }

    /// Whether the picture differs from the capture as it was taken.
    pub fn is_edited(&self) -> bool {
        let edits = self.edits.get();
        !edits.annotations.is_empty()
            || edits.crop.is_some()
            || !Arc::ptr_eq(&edits.base, &self.original)
    }

    /// Whether the card's picture shows something other than this.
    pub fn is_unwritten(&self) -> bool {
        *self.edits.get() != self.written
    }

    /// Whether the user's saved file shows exactly this.
    pub fn is_saved(&self) -> bool {
        self.saved.as_ref() == Some(self.edits.get())
    }

    /// `edits` have been written to the user's file, and so to the card.
    pub fn mark_saved(&mut self, edits: Edits) {
        self.written = edits.clone();
        self.saved = Some(edits);
    }

    /// The picture for the card if it shows something else, which from then on counts as
    /// written: Back, and quitting with the editor open, give the card this.
    pub fn take_unwritten(&mut self) -> Option<Flattened> {
        if !self.is_unwritten() {
            return None;
        }
        let flattened = self.flatten();
        self.written = flattened.edits.clone();
        Some(flattened)
    }

    /// The picture as it stands.
    pub fn flatten(&self) -> Flattened {
        Flattened {
            picture: Arc::new(self.render()),
            original: self.original.clone(),
            is_edited: self.is_edited(),
            redacted: self.is_redacted(),
            edits: self.edits.get().clone(),
        }
    }

    /// The marks on the picture, bottom to top.
    pub fn annotations(&self) -> &[Annotation] {
        &self.edits.get().annotations
    }

    /// Puts `annotation` on top, as one undoable step. Returns its index.
    pub fn add(&mut self, annotation: Annotation) -> usize {
        self.edits.edit(|edits| edits.annotations.push(annotation));
        self.annotations().len() - 1
    }

    /// Replaces the mark at `index`, as one undoable step. Returns whether anything changed:
    /// nothing happens if the mark is already so.
    pub fn replace(&mut self, index: usize, annotation: Annotation) -> bool {
        if self
            .annotations()
            .get(index)
            .is_none_or(|mark| *mark == annotation)
        {
            return false;
        }
        self.edits
            .edit(|edits| edits.annotations[index] = annotation);
        true
    }

    /// Replaces the mark at `index` within the last undo step, for the later moves of a slider
    /// drag whose first move was a `replace`.
    pub fn amend(&mut self, index: usize, annotation: Annotation) {
        if index < self.annotations().len() {
            self.edits
                .amend(|edits| edits.annotations[index] = annotation);
        }
    }

    /// Takes the mark at `index` off: as one undoable step, or within the last step if
    /// `amend`, so an eraser drag that takes several marks off is undone as one.
    pub fn remove(&mut self, index: usize, amend: bool) {
        if index >= self.annotations().len() {
            return;
        }
        let change = |edits: &mut Edits| {
            edits.annotations.remove(index);
        };
        if amend {
            self.edits.amend(change);
        } else {
            self.edits.edit(change);
        }
    }

    /// The topmost mark within `tolerance` picture pixels of `at`, as its index.
    pub fn hit_test(&self, at: Point<f32>, tolerance: f32) -> Option<usize> {
        self.annotations()
            .iter()
            .rposition(|annotation| annotation.hit(at, tolerance))
    }

    pub fn can_undo(&self) -> bool {
        self.edits.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.edits.can_redo()
    }

    pub fn undo(&mut self) -> bool {
        self.edits.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.edits.redo()
    }

    /// Reset edits: back to the capture as it was taken, with every annotation and the crop
    /// off, as one undoable step. `false` when it is already so.
    pub fn reset(&mut self) -> bool {
        if !self.is_edited() || !self.can_reset() {
            return false;
        }
        let base = self.original.clone();
        self.edits.edit(|edits| {
            *edits = Edits {
                base,
                annotations: Vec::new(),
                crop: None,
            }
        });
        true
    }

    /// The picture with its annotations, within the crop, as the canvas shows it and Save
    /// writes it.
    pub fn render(&self) -> Pixmap {
        self.render_region(self.crop(), None, None)
            .expect("the crop is within the picture")
            .1
    }

    /// The pixels in `region` of the whole picture with its marks, as a picture of their own:
    /// the crop for the canvas, a mark's surroundings so a drag only redraws where the mark is,
    /// or the whole picture for the Crop tool. For a drag's preview, the mark at `index` is left
    /// out, or drawn as `preview` in its place, under the same marks as before, so a mark being
    /// moved or resized keeps its depth; a `preview` with no `index` is a mark being drawn, on
    /// top. Committing the preview to the document renders the same pixels, so nothing shifts
    /// on release. `None` if the region misses the picture.
    pub fn render_region(
        &self,
        region: IntRect,
        index: Option<usize>,
        preview: Option<&Annotation>,
    ) -> Option<(IntRect, Pixmap)> {
        let region = region.intersect(&self.whole())?;
        let mut picture = Pixmap::new(region.width(), region.height())?;
        picture.draw_pixmap(
            -region.x(),
            -region.y(),
            self.base().as_ref(),
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
    /// `origin`, with the drag's preview as `render_region` describes. A block reads the base
    /// with the blocks under it applied, so a block over another shows only what the first
    /// left and adding a block never reveals more; that base is built up, over the whole
    /// picture, only when a second block needs it.
    fn draw_marks(
        &self,
        picture: &mut Pixmap,
        origin: Point<i32>,
        index: Option<usize>,
        preview: Option<&Annotation>,
    ) {
        let marks = self
            .annotations()
            .iter()
            .enumerate()
            .filter_map(|(i, mark)| (Some(i) != index).then_some(mark))
            .chain(preview.filter(|_| index.is_none()));
        let mut marks: Vec<&Annotation> = marks.collect();
        if let (Some(i), Some(preview)) = (index, preview) {
            marks.insert(i.min(marks.len()), preview);
        }
        let blocks = marks.iter().filter(|mark| mark.shape.is_block()).count();
        let mut hidden = (blocks > 1).then(|| self.base().clone());
        for mark in marks {
            if !mark.shape.is_block() {
                mark.draw_at(picture, origin);
                continue;
            }
            let Some((rect, pixels)) = mark.block_pixels(hidden.as_ref().unwrap_or(self.base()))
            else {
                continue;
            };
            annotation::paint_pixels(picture, origin, rect, &pixels);
            if let Some(hidden) = hidden.as_mut() {
                annotation::paint_pixels(hidden, point(0, 0), rect, &pixels);
            }
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

    /// The crop's pixels with a drag's preview, as the canvas shows them.
    fn render_with(
        document: &Document,
        index: Option<usize>,
        preview: Option<&Annotation>,
    ) -> Pixmap {
        document
            .render_region(document.crop(), index, preview)
            .unwrap()
            .1
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
        line.extend(
            gpui::point(to, y),
            false,
            gpui::Bounds::new(gpui::point(0., 0.), gpui::size(100., 100.)),
        );
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
        document.remove(0, false);
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
        let preview = render_with(&document, Some(0), Some(&moved));
        let overlap = preview.pixel(20, 12).unwrap();
        assert!(overlap.red() > 200 && overlap.green() < 50);
        document.replace(0, moved);
        assert_eq!(preview.data(), document.render().data());
        // Left out, the mark is gone and what was under it shows.
        let without = render_with(&document, Some(1), None);
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
        let whole = render_with(&document, None, Some(&drawing));
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
        let preview = render_with(&document, Some(0), Some(&moved));
        document.replace(0, moved);
        assert_eq!(preview.data(), document.render().data());
        assert!(preview.pixel(20, 15).unwrap().red() > 200);
        assert_eq!(preview.pixel(20, 10).unwrap().red(), 255);
        assert_eq!(preview.pixel(20, 10).unwrap().green(), 255);
    }

    #[test]
    fn a_crop_changes_what_is_shown_and_is_one_undo_step() {
        let base = super::super::block::tests::text_like(64, 48);
        let mut document = Document::new(base.clone());
        assert_eq!(document.crop(), document.whole());
        document.add(red_line(0., 64., 10.));
        let crop = IntRect::from_xywh(20, 20, 30, 20).unwrap();
        assert!(document.set_crop(Some(crop)));
        assert!(document.is_cropped());
        assert_eq!((document.width(), document.height()), (30, 20));
        // The render is the crop's pixels of the whole picture: the line at y 10 is outside.
        let cropped = document.render();
        assert_eq!((cropped.width(), cropped.height()), (30, 20));
        let (_, whole) = document
            .render_region(document.whole(), None, None)
            .unwrap();
        for y in 0..20 {
            for x in 0..30 {
                assert_eq!(cropped.pixel(x, y), whole.pixel(x + 20, y + 20));
            }
        }
        assert!(cropped.pixels().iter().all(|p| p.red() == p.green()));
        assert!(whole.pixel(32, 10).unwrap().red() > 200);
        // The mark stays where it was, in the whole picture's pixels.
        assert_eq!(document.annotations()[0].points[0].y, 10.);
        // The same crop again, or one that is held to the picture's edge, changes nothing new.
        assert!(!document.set_crop(Some(crop)));
        assert!(document.set_crop(Some(IntRect::from_xywh(-10, -10, 200, 200).unwrap())));
        assert!(!document.is_cropped());
        assert!(document.undo());
        assert_eq!(document.crop(), crop);
        assert!(document.undo());
        assert!(!document.is_cropped());
        assert!(document.is_edited());
        assert!(document.redo());
        assert!(document.reset());
        assert!(!document.is_cropped());
        assert!(!document.is_edited());
        // A crop alone is an edit.
        let mut just_cropped = Document::new(base);
        assert!(just_cropped.set_crop(Some(crop)));
        assert!(just_cropped.is_edited());
        assert!(!Document::new(Pixmap::new(4, 4).unwrap()).set_crop(None));
    }

    #[test]
    fn an_eraser_sweep_is_one_undo_step() {
        let mut document = Document::new(Pixmap::new(40, 20).unwrap());
        document.add(red_line(0., 40., 5.));
        document.add(red_line(0., 40., 10.));
        document.add(red_line(0., 40., 15.));
        document.remove(2, false);
        document.remove(0, true);
        assert_eq!(document.annotations().len(), 1);
        assert_eq!(document.annotations()[0].points[0].y, 10.);
        assert!(document.undo());
        assert_eq!(document.annotations().len(), 3);
        // Removing what isn't there does nothing.
        document.remove(7, true);
        assert_eq!(document.annotations().len(), 3);
    }

    #[test]
    fn a_block_hides_the_marks_under_it_and_renders_the_same_as_a_region() {
        use super::super::block::BlockKind;
        let base = super::super::block::tests::text_like(96, 64);
        let mut document = Document::new(base);
        document.add(red_line(0., 96., 30.));
        for kind in [BlockKind::Blur, BlockKind::Pixelate, BlockKind::Solid] {
            let index = document.add(block(kind, (10., 10.), (80., 50.)));
            let whole = document.render();
            // The line under the block is hidden with the picture: no red within the block.
            for x in 10..80 {
                let p = whole.pixel(x, 30).unwrap();
                assert!(
                    p.red() as u16 <= p.blue() as u16 + 1,
                    "{kind:?} shows red at ({x}, 30)"
                );
            }
            assert!(whole.pixel(5, 30).unwrap().red() > 200);
            // A region of the block comes out the same as the whole, cells and all.
            let (region, part) = document
                .render_region(IntRect::from_xywh(30, 25, 40, 30).unwrap(), None, None)
                .unwrap();
            for y in 0..region.height() {
                for x in 0..region.width() {
                    assert_eq!(
                        part.pixel(x, y).unwrap(),
                        whole.pixel(x + 30, y + 25).unwrap(),
                        "{kind:?} pixel ({x}, {y})"
                    );
                }
            }
            document.remove(index, false);
        }
    }

    fn plain(width: u32, height: u32, red: u8) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        pixmap.fill(Color::from_rgba8(red, 0, 0, 255));
        pixmap
    }

    #[test]
    fn reset_returns_a_flattened_picture_to_the_original_and_undo_brings_it_back() {
        let mut document = Document::opened(plain(4, 3, 10), Some(plain(8, 6, 200)), false);
        assert!(document.is_edited());
        assert_eq!((document.width(), document.height()), (4, 3));

        assert!(document.reset());
        assert!(!document.is_edited());
        assert_eq!((document.width(), document.height()), (8, 6));
        assert_eq!(document.render().pixel(0, 0).unwrap().red(), 200);
        assert!(!document.reset());

        assert!(document.undo());
        assert_eq!((document.width(), document.height()), (4, 3));
        assert_eq!(document.render().pixel(0, 0).unwrap().red(), 10);
    }

    #[test]
    fn a_block_or_a_picture_opened_redacted_makes_the_flattened_picture_redacted() {
        use super::super::block::BlockKind;
        let mut document = Document::new(plain(96, 64, 0));
        assert!(!document.flatten().redacted);
        document.add(block(BlockKind::Pixelate, (10., 10.), (80., 50.)));
        assert!(document.flatten().redacted);
        document.undo();
        assert!(!document.flatten().redacted);

        // A picture stored with its redactions flattened stays redacted until Reset.
        let redacted = Redaction {
            redacted: true,
            original_removed: false,
        };
        let mut document = Document::opened(plain(4, 3, 10), Some(plain(4, 3, 200)), false)
            .with_redaction(redacted);
        assert!(document.flatten().redacted);
        assert!(document.reset());
        assert!(!document.flatten().redacted);
    }

    #[test]
    fn with_its_original_deleted_a_picture_cannot_be_reset() {
        let removed = Redaction {
            redacted: true,
            original_removed: true,
        };
        let mut document = Document::opened(plain(4, 3, 10), None, false).with_redaction(removed);
        document.set_crop(IntRect::from_xywh(0, 0, 2, 2));
        assert!(!document.can_reset());
        assert!(!document.reset());
        assert!(document.flatten().redacted);

        // Saving in this session deletes it just the same.
        let mut document = Document::opened(plain(4, 3, 10), Some(plain(4, 3, 200)), false);
        assert!(document.can_reset());
        let flattened = document.flatten();
        document.forget_original(flattened.picture, &flattened.edits, true);
        assert!(!document.reset());
    }

    #[test]
    fn after_a_redacted_save_undo_cannot_take_the_block_off() {
        use super::super::block::BlockKind;
        let mut document = Document::new(plain(96, 64, 0));
        document.add(block(BlockKind::Solid, (10., 10.), (80., 50.)));
        document.set_crop(IntRect::from_xywh(0, 0, 90, 60));
        let saved = document.flatten();
        let blocked = saved.picture.pixel(40, 30).unwrap();

        document.forget_original(saved.picture.clone(), &saved.edits, true);

        // The saved picture is now the picture, with nothing over it to take off.
        assert!(!document.can_undo());
        assert!(!document.undo());
        assert!(!document.reset());
        assert!(document.annotations().is_empty());
        assert_eq!((document.width(), document.height()), (90, 60));
        assert_eq!(document.render().pixel(40, 30).unwrap(), blocked);
        // The card and the file show it already, and it still counts as hiding something.
        assert!(document.is_saved());
        assert!(!document.is_unwritten());
        assert!(document.flatten().redacted);
    }

    #[test]
    fn after_a_redacted_save_as_the_card_still_needs_the_picture() {
        use super::super::block::BlockKind;
        let mut document = Document::new(plain(96, 64, 0));
        document.add(block(BlockKind::Blur, (10., 10.), (80., 50.)));
        let copy = document.flatten();

        document.forget_original(copy.picture, &copy.edits, false);

        assert!(!document.can_undo());
        assert!(document.is_unwritten());
        assert!(!document.is_saved());
        let back = document.take_unwritten().unwrap();
        assert!(back.redacted);
    }

    #[test]
    fn a_block_moved_while_saving_leaves_what_the_save_hid_hidden() {
        use super::super::block::BlockKind;
        // White and black pixels in turn, which a block never leaves behind.
        let image = RgbaImage::from_fn(96, 64, |x, y| {
            let on = if (x + y) % 2 == 0 { 255 } else { 0 };
            xcap::image::Rgba([on, on, on, 255])
        });
        let mut document = Document::new(pixmap_from_rgba(image).unwrap());
        document.add(block(BlockKind::Solid, (10., 10.), (40., 40.)));
        let saved = document.flatten();
        let hidden = saved.picture.pixel(25, 25).unwrap();
        // While the save runs: the block moves away and a crop goes on.
        document.replace(0, block(BlockKind::Solid, (60., 20.), (90., 50.)));
        document.set_crop(IntRect::from_xywh(0, 0, 94, 60));

        document.forget_original(saved.picture, &saved.edits, true);

        // The saved block is in the picture; the moved one is over it, and the crop stays.
        assert_eq!(document.render().pixel(25, 25).unwrap(), hidden);
        assert_eq!(document.annotations().len(), 1);
        assert_eq!((document.width(), document.height()), (94, 60));
        assert!(!document.can_undo());
        assert!(!document.can_reset());
        let back = document.take_unwritten().unwrap();
        assert!(back.redacted);
        // Taking the moved block off still leaves the saved one.
        document.remove(0, false);
        assert_eq!(document.render().pixel(25, 25).unwrap(), hidden);
    }

    #[test]
    fn a_reset_while_saving_gives_way_to_the_saved_picture() {
        use super::super::block::BlockKind;
        let mut document = Document::opened(plain(96, 64, 10), Some(plain(96, 64, 200)), false);
        document.add(block(BlockKind::Solid, (10., 10.), (40., 40.)));
        let saved = document.flatten();
        document.reset();

        document.forget_original(saved.picture.clone(), &saved.edits, true);

        assert!(document.annotations().is_empty());
        assert_eq!(document.render().data(), saved.picture.data());
        assert!(!document.can_undo());
    }

    #[test]
    fn a_picture_never_edited_has_nothing_to_reset() {
        let mut document = Document::opened(plain(4, 3, 10), None, false);
        assert!(!document.is_edited());
        assert!(!document.reset());
    }

    #[test]
    fn saved_and_unwritten_follow_the_edits() {
        // A saved capture opens showing what its file shows.
        let mut document = Document::opened(plain(40, 20, 0), None, true);
        assert!(document.is_saved());
        assert!(!document.is_unwritten());

        document.set_crop(IntRect::from_xywh(0, 0, 10, 10));
        assert!(!document.is_saved());
        assert!(document.is_unwritten());

        // Saved, then changed, then undone back to what was saved.
        let flattened = document.flatten();
        assert!(flattened.is_edited);
        document.mark_saved(flattened.edits);
        assert!(document.is_saved());
        assert!(!document.is_unwritten());
        document.set_crop(None);
        assert!(!document.is_saved());
        document.undo();
        assert!(document.is_saved());

        // What goes to the card counts as written, but not as saved.
        document.set_crop(IntRect::from_xywh(0, 0, 20, 10));
        assert!(document.take_unwritten().is_some());
        assert!(!document.is_unwritten());
        assert!(!document.is_saved());
        assert!(document.take_unwritten().is_none());

        // Edits waiting for Save, or a capture never saved, match no file.
        let opened = Document::opened(plain(4, 3, 0), None, false);
        assert!(!opened.is_saved());
        assert!(!opened.is_unwritten());
    }

    #[test]
    fn the_flattened_picture_is_cropped_and_what_a_block_covers_is_gone() {
        use super::super::block::BlockKind;
        // One pixel of white on black and the next of black on white, so any kind of block
        // leaves neither behind.
        let image = RgbaImage::from_fn(96, 64, |x, y| {
            let on = if (x + y) % 2 == 0 { 255 } else { 0 };
            xcap::image::Rgba([on, on, on, 255])
        });
        let base = pixmap_from_rgba(image).unwrap();
        for kind in [BlockKind::Blur, BlockKind::Pixelate, BlockKind::Solid] {
            let mut document = Document::new(base.clone());
            document.add(block(kind, (10., 10.), (80., 50.)));
            document.set_crop(IntRect::from_xywh(0, 0, 90, 60));
            let written = to_rgba(&document.flatten().picture);
            assert_eq!(written.dimensions(), (90, 60));
            for y in 20..40 {
                for x in 20..70 {
                    let [r, g, b, a] = written.get_pixel(x, y).0;
                    assert_eq!(a, 255, "{kind:?} is opaque");
                    assert!(
                        [r, g, b] != [255, 255, 255] && [r, g, b] != [0, 0, 0],
                        "{kind:?} left the picture at ({x}, {y})"
                    );
                }
            }
            // Outside the block the picture is as it was.
            assert_eq!(written.get_pixel(4, 3).0, [0, 0, 0, 255]);
        }
    }

    /// A blue block of `kind` dragged from `from` to `to` on a 96×64 picture.
    fn block(kind: super::super::block::BlockKind, from: (f32, f32), to: (f32, f32)) -> Annotation {
        use super::super::annotation::{Shape, Style};
        use super::super::tools::{LineStyle, Width};
        let mut block = Annotation::new(
            Shape::Block(kind),
            gpui::point(from.0, from.1),
            Style {
                color: Color::from_rgba8(0, 0, 255, 255),
                second: Color::BLACK,
                width: Width::Medium,
                line: LineStyle::Solid,
                opacity: 1.,
            },
        );
        block.extend(
            gpui::point(to.0, to.1),
            false,
            gpui::Bounds::new(gpui::point(0., 0.), gpui::size(96., 64.)),
        );
        block
    }

    #[test]
    fn a_block_over_a_block_reads_what_the_first_hid() {
        use super::super::block::BlockKind;
        let base = super::super::block::tests::text_like(96, 64);
        let is_blue = |p: tiny_skia::PremultipliedColorU8| p.blue() == 255 && p.red() == 0;
        for kind in [BlockKind::Blur, BlockKind::Pixelate] {
            // Solid over the text, then Blur or Pixelate over part of the solid: nothing of
            // the text comes back, the block is solid blue through and through.
            let mut document = Document::new(base.clone());
            document.add(block(BlockKind::Solid, (0., 0.), (96., 64.)));
            let over = block(kind, (10., 10.), (80., 50.));
            // As a preview, on top, and committed.
            let preview = render_with(&document, None, Some(&over));
            assert!(
                preview.pixels().iter().all(|p| is_blue(*p)),
                "{kind:?} preview"
            );
            let index = document.add(over.clone());
            let whole = document.render();
            assert!(whole.pixels().iter().all(|p| is_blue(*p)), "{kind:?}");
            // A region of the second block comes out the same as the whole.
            let (region, part) = document
                .render_region(IntRect::from_xywh(30, 25, 40, 30).unwrap(), None, None)
                .unwrap();
            for y in 0..region.height() {
                for x in 0..region.width() {
                    assert_eq!(part.pixel(x, y), whole.pixel(x + 30, y + 25), "{kind:?}");
                }
            }
            // The same block moved as a preview at its depth reads the solid too.
            let moved = block(kind, (12., 12.), (82., 52.));
            let dragged = render_with(&document, Some(index), Some(&moved));
            assert!(
                dragged.pixels().iter().all(|p| is_blue(*p)),
                "{kind:?} dragged"
            );
        }
        // A small Pixelate over a large one shows the large one's cells, not finer ones. Both
        // have 8px cells, the large block's anchored at (0, 0) and the small one's at (8, 8),
        // so they share a grid and every cell the small block averages is already one colour:
        // its pixels stay what the large one made them.
        let mut document = Document::new(base.clone());
        document.add(block(BlockKind::Pixelate, (0., 0.), (96., 32.)));
        let large = document.render();
        document.add(block(BlockKind::Pixelate, (8., 8.), (40., 24.)));
        assert_eq!(document.render().data(), large.data());
        // In the other order the small block is read by the large one, so nothing of what it
        // hid shows: the pixels where it was are averages of the small block's cells alone.
        let mut document = Document::new(base);
        document.add(block(BlockKind::Solid, (8., 8.), (40., 24.)));
        document.add(block(BlockKind::Pixelate, (0., 0.), (96., 32.)));
        let rendered = document.render();
        for y in 8..24 {
            for x in 8..40 {
                assert!(is_blue(rendered.pixel(x, y).unwrap()), "({x}, {y})");
            }
        }
    }

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
