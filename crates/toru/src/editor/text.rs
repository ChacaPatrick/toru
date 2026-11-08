//! Text set from the bundled fonts' outlines, for the text and number badge annotations.
//! `tiny-skia` has no text of its own, so each glyph's outline is read with `ttf-parser` from the
//! same files GPUI sets the interface in, and filled as a path: the picture on screen and the
//! saved file show the same letters. Everything here is a plain function of the text and its
//! font, in picture pixels, so the tests can check the pixels.

use gpui::{FontWeight, Point, SharedString, Size, point};
use tiny_skia::{Path, PathBuilder};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::theme::{FONTS, bundled_font};

/// A face as GPUI names it. The file is looked up each time it is needed; the lookup is a walk
/// over ten entries and parsing a face only reads its table directory.
#[derive(Clone, PartialEq, Debug)]
pub struct Font {
    pub family: SharedString,
    pub weight: FontWeight,
}

impl Font {
    /// The bundled face nearest to this font. A family that isn't bundled, which the themes
    /// never ask for, is set in the first bundled face rather than not at all.
    fn face(&self) -> Face<'static> {
        let font = bundled_font(&self.family, self.weight).unwrap_or(&FONTS[0]);
        Face::parse(font.data, 0).expect("the bundled fonts are valid TrueType files")
    }
}

/// Where a text's lines land, in picture pixels from its top-left corner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    /// The box around every line: the widest line by the lines' heights. Empty text is one line
    /// tall and nothing wide, so a caret has somewhere to stand.
    pub size: Size<f32>,
    /// The top of the caret after the last character, which is `line_height` tall.
    pub caret: Point<f32>,
    pub line_height: f32,
}

/// Lays `content` out in `font` at `size` picture pixels, one line per `\n`, as CSS does at
/// `line-height: normal`: each line is the face's ascent, descent and line gap tall.
pub fn layout(content: &str, font: &Font, size: f32) -> Layout {
    let face = font.face();
    let metrics = Metrics::of(&face, size);
    let mut width: f32 = 0.;
    let mut last = 0.;
    let mut lines = 0;
    for line in content.split('\n') {
        last = metrics.advance(&face, line);
        width = width.max(last);
        lines += 1;
    }
    Layout {
        size: Size {
            width,
            height: lines as f32 * metrics.line_height,
        },
        caret: point(last, (lines - 1) as f32 * metrics.line_height),
        line_height: metrics.line_height,
    }
}

/// The outlines of `content` set as `layout` lays it out, with the top-left corner at
/// `origin`, as one path to fill. `None` when there is nothing to draw.
pub fn outline(content: &str, font: &Font, size: f32, origin: Point<f32>) -> Option<Path> {
    let face = font.face();
    let metrics = Metrics::of(&face, size);
    let mut builder = Outline {
        path: PathBuilder::new(),
        scale: metrics.scale,
        at: origin,
    };
    for (row, line) in content.split('\n').enumerate() {
        builder.at = point(
            origin.x,
            origin.y + row as f32 * metrics.line_height + metrics.baseline,
        );
        for ch in line.chars() {
            let Some(glyph) = face.glyph_index(ch) else {
                builder.at.x += metrics.missing;
                continue;
            };
            face.outline_glyph(glyph, &mut builder);
            builder.at.x += metrics.advance_of(&face, glyph);
        }
    }
    builder.path.finish()
}

/// A face's vertical metrics scaled to a size.
struct Metrics {
    /// Picture pixels per font unit.
    scale: f32,
    line_height: f32,
    /// From a line's top to its baseline: half the line gap, then the ascent.
    baseline: f32,
    /// The advance of a character the face lacks: a space's, so the text keeps its rhythm.
    missing: f32,
}

impl Metrics {
    fn of(face: &Face, size: f32) -> Metrics {
        let scale = size / face.units_per_em() as f32;
        let gap = face.line_gap() as f32 * scale;
        let ascent = face.ascender() as f32 * scale;
        let descent = -face.descender() as f32 * scale;
        let mut metrics = Metrics {
            scale,
            line_height: ascent + descent + gap,
            baseline: gap / 2. + ascent,
            missing: 0.,
        };
        metrics.missing = face
            .glyph_index(' ')
            .map_or(size / 3., |space| metrics.advance_of(face, space));
        metrics
    }

    fn advance_of(&self, face: &Face, glyph: GlyphId) -> f32 {
        face.glyph_hor_advance(glyph).unwrap_or(0) as f32 * self.scale
    }

    /// How wide `line` is set.
    fn advance(&self, face: &Face, line: &str) -> f32 {
        line.chars()
            .map(|ch| {
                face.glyph_index(ch)
                    .map_or(self.missing, |glyph| self.advance_of(face, glyph))
            })
            .sum()
    }
}

/// Collects glyph outlines, which the face gives in font units with y up, into one path in
/// picture pixels with y down, each glyph at its pen position `at` on the baseline.
struct Outline {
    path: PathBuilder,
    scale: f32,
    at: Point<f32>,
}

impl Outline {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        (self.at.x + x * self.scale, self.at.y - y * self.scale)
    }
}

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.map(x1, y1);
        let (x, y) = self.map(x, y);
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.map(x1, y1);
        let (x2, y2) = self.map(x2, y2);
        let (x, y) = self.map(x, y);
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;
    use tiny_skia::{Color, FillRule, Paint, Pixmap, Transform};

    fn inter() -> Font {
        Font {
            family: "Inter".into(),
            weight: FontWeight::SEMIBOLD,
        }
    }

    fn draw(content: &str, font: &Font, size: f32) -> Pixmap {
        let layout = layout(content, font, size);
        let mut picture = Pixmap::new(
            layout.size.width.ceil() as u32 + 4,
            layout.size.height.ceil() as u32 + 4,
        )
        .unwrap();
        picture.fill(Color::WHITE);
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.anti_alias = true;
        if let Some(path) = outline(content, font, size, point(2., 2.)) {
            picture.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        picture
    }

    fn dark_pixels(picture: &Pixmap) -> usize {
        picture.pixels().iter().filter(|p| p.red() < 128).count()
    }

    #[test]
    fn a_line_is_as_tall_as_the_face_and_as_wide_as_its_glyphs() {
        let font = inter();
        let one = layout("Hello", &font, 16.);
        assert!(one.size.width > 30. && one.size.width < 60.);
        assert!(one.size.height > 16. && one.size.height < 24.);
        assert_eq!(one.line_height, one.size.height);
        assert_eq!(one.caret, point(one.size.width, 0.));
        // Twice the size is twice as wide and tall.
        let big = layout("Hello", &font, 32.);
        assert!((big.size.width - one.size.width * 2.).abs() < 0.01);
        assert!((big.size.height - one.size.height * 2.).abs() < 0.01);
    }

    #[test]
    fn lines_stack_and_the_caret_follows_the_last() {
        let font = inter();
        let two = layout("Hello\nHi", &font, 16.);
        let hello = layout("Hello", &font, 16.);
        let hi = layout("Hi", &font, 16.);
        assert_eq!(two.size.width, hello.size.width);
        assert!((two.size.height - hello.line_height * 2.).abs() < 0.01);
        assert_eq!(two.caret, point(hi.size.width, hello.line_height));
        // Nothing typed yet still stands one line tall for the caret.
        let empty = layout("", &font, 16.);
        assert_eq!(empty.size, size(0., hello.line_height));
        assert_eq!(empty.caret, point(0., 0.));
    }

    #[test]
    fn glyphs_are_filled_inside_their_line_box_only() {
        let font = inter();
        let picture = draw("Hello", &font, 24.);
        assert!(dark_pixels(&picture) > 100);
        // The corners of the box, past the margin, stay white: nothing paints out of the layout.
        let (w, h) = (picture.width(), picture.height());
        assert!((0..w).all(|x| picture.pixel(x, 0).unwrap().red() == 255));
        assert!((0..h).all(|y| picture.pixel(0, y).unwrap().red() == 255));
        assert!((0..h).all(|y| picture.pixel(w - 1, y).unwrap().red() == 255));
        // An empty string draws nothing.
        assert!(outline("", &font, 24., point(0., 0.)).is_none());
    }

    #[test]
    fn a_character_the_face_lacks_takes_a_spaces_room() {
        let font = Font {
            family: "VT323".into(),
            weight: FontWeight::NORMAL,
        };
        // VT323's subset has no 文; the text keeps its rhythm and draws nothing for it.
        let with = layout("a文b", &font, 20.);
        let space = layout("a b", &font, 20.);
        assert_eq!(with.size.width, space.size.width);
        assert_eq!(
            dark_pixels(&draw("文", &font, 20.)),
            0,
            "a missing glyph draws nothing"
        );
    }

    #[test]
    fn each_bundled_family_sets_text_and_an_unknown_one_falls_back() {
        for font in &FONTS {
            let font = Font {
                family: font.family.into(),
                weight: font.weight,
            };
            assert!(dark_pixels(&draw("Ag", &font, 20.)) > 20, "{}", font.family);
        }
        let unknown = Font {
            family: "No Such Face".into(),
            weight: FontWeight::NORMAL,
        };
        let fallback = Font {
            family: FONTS[0].family.into(),
            weight: FONTS[0].weight,
        };
        assert_eq!(layout("Ag", &unknown, 20.), layout("Ag", &fallback, 20.));
    }
}
