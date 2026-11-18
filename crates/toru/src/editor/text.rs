//! `tiny-skia` has no text of its own, so glyph outlines are read with `ttf-parser` from the
//! files GPUI sets the interface in, and filled as paths.

use std::{
    path::PathBuf,
    sync::{LazyLock, OnceLock},
};

use gpui::{FontWeight, Point, SharedString, Size, point};
use tiny_skia::{Path, PathBuilder};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::platform;
use crate::theme::{FONTS, bundled_font};

/// The file is looked up each time it is needed: the lookup is a walk over ten entries, and
/// parsing a face only reads its table directory.
#[derive(Clone, PartialEq, Debug)]
pub struct Font {
    pub family: SharedString,
    pub weight: FontWeight,
}

impl Font {
    /// A family that isn't bundled, which the themes never ask for, is set in the first bundled
    /// face rather than not at all.
    fn face(&self) -> Face<'static> {
        let font = bundled_font(&self.family, self.weight).unwrap_or(&FONTS[0]);
        Face::parse(font.data, 0).expect("the bundled fonts are valid TrueType files")
    }
}

/// The languages written with Han characters, which each draw many of them their own way, so
/// a Japanese text wants its kanji from a Japanese font.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cjk {
    Simplified,
    Traditional,
    Japanese,
    Korean,
}

impl Cjk {
    fn of_locale(locale: &str) -> Option<Cjk> {
        let locale = locale.to_ascii_lowercase();
        let mut parts = locale.split(['-', '_', '.']);
        match parts.next()? {
            "ja" => Some(Cjk::Japanese),
            "ko" => Some(Cjk::Korean),
            "zh" => {
                let rest: Vec<&str> = parts.collect();
                let traditional = !rest.contains(&"hans")
                    && rest
                        .iter()
                        .any(|part| ["hant", "tw", "hk", "mo"].contains(part));
                Some(if traditional {
                    Cjk::Traditional
                } else {
                    Cjk::Simplified
                })
            }
            _ => None,
        }
    }

    /// Kana mean Japanese, as only Japanese is written in them.
    fn preferred(content: &str, languages: &[String]) -> Cjk {
        if content.chars().any(is_kana) {
            return Cjk::Japanese;
        }
        languages
            .iter()
            .find_map(|language| Cjk::of_locale(language))
            .unwrap_or(Cjk::Simplified)
    }
}

/// Hiragana and katakana, full- and half-width.
fn is_kana(ch: char) -> bool {
    matches!(ch, '\u{3041}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}')
}

/// `fonts` in the order they are tried for text that prefers `cjk`: the fonts for `cjk` move up
/// to where the first CJK font is, and the rest keep their order.
fn ordered<T>(fonts: &[T], cjk: Cjk, language: impl Fn(&T) -> Option<Cjk>) -> Vec<&T> {
    let first = fonts
        .iter()
        .position(|font| language(font).is_some())
        .unwrap_or(fonts.len());
    let (before, after) = fonts.split_at(first);
    let preferred = |font: &&T| language(font) == Some(cjk);
    before
        .iter()
        .chain(after.iter().filter(preferred))
        .chain(after.iter().filter(|font| !preferred(font)))
        .collect()
}

/// Mapped rather than read, so asking one whether it has a character costs a few pages of it.
/// Kept for the app's life.
struct Fallback {
    path: PathBuf,
    cjk: Option<Cjk>,
    face: OnceLock<Option<Face<'static>>>,
}

impl Fallback {
    fn face(&self) -> Option<&Face<'static>> {
        self.face
            .get_or_init(|| Face::parse(platform::map_file(&self.path)?, 0).ok())
            .as_ref()
    }
}

static FALLBACKS: LazyLock<Vec<Fallback>> = LazyLock::new(|| {
    platform::fallback_fonts()
        .into_iter()
        .map(|(path, cjk)| Fallback {
            path,
            cjk,
            face: OnceLock::new(),
        })
        .collect()
});

static LANGUAGES: LazyLock<Vec<String>> = LazyLock::new(platform::user_languages);

struct Faces {
    main: Face<'static>,
    fallbacks: Vec<&'static Fallback>,
}

impl Faces {
    fn new(font: &Font, content: &str) -> Faces {
        let cjk = Cjk::preferred(content, &LANGUAGES);
        Faces {
            main: font.face(),
            fallbacks: ordered(&FALLBACKS, cjk, |font| font.cjk),
        }
    }

    /// A later fallback is only opened when the ones before it lack the character.
    fn glyph(&self, ch: char) -> Option<(&Face<'static>, GlyphId)> {
        if let Some(glyph) = self.main.glyph_index(ch) {
            return Some((&self.main, glyph));
        }
        self.fallbacks
            .iter()
            .filter_map(|fallback| fallback.face())
            .find_map(|face| Some((face, face.glyph_index(ch)?)))
    }
}

/// Joiners and variation selectors, which only steer how their neighbours are drawn: where no
/// face has a glyph for them they take no room and aren't missed.
fn is_invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}'..='\u{200F}'
            | '\u{2060}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

pub fn has_blanks(content: &str, font: &Font) -> bool {
    let faces = Faces::new(font, content);
    content.chars().any(|ch| {
        !ch.is_whitespace() && !ch.is_control() && !is_invisible(ch) && faces.glyph(ch).is_none()
    })
}

/// In picture pixels from the text's top-left corner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    /// Empty text is one line tall and nothing wide, so a caret has somewhere to stand.
    pub size: Size<f32>,
    /// The caret's top, after the last character.
    pub caret: Point<f32>,
    pub line_height: f32,
}

/// Each line is the face's ascent, descent and line gap tall, as CSS does at
/// `line-height: normal`.
pub fn layout(content: &str, font: &Font, size: f32) -> Layout {
    let faces = Faces::new(font, content);
    let metrics = Metrics::of(&faces.main, size);
    let mut width: f32 = 0.;
    let mut last = 0.;
    let mut lines = 0;
    for line in content.split('\n') {
        last = line.chars().map(|ch| metrics.advance(&faces, ch)).sum();
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

pub fn outline(content: &str, font: &Font, size: f32, origin: Point<f32>) -> Option<Path> {
    let faces = Faces::new(font, content);
    let metrics = Metrics::of(&faces.main, size);
    let mut builder = Outline {
        path: PathBuilder::new(),
        scale: 0.,
        at: origin,
    };
    for (row, line) in content.split('\n').enumerate() {
        builder.at = point(
            origin.x,
            origin.y + row as f32 * metrics.line_height + metrics.baseline,
        );
        for ch in line.chars() {
            if let Some((face, glyph)) = faces.glyph(ch) {
                builder.scale = metrics.scale(face);
                face.outline_glyph(glyph, &mut builder);
            }
            builder.at.x += metrics.advance(&faces, ch);
        }
    }
    builder.path.finish()
}

struct Metrics {
    size: f32,
    line_height: f32,
    /// From a line's top to its baseline: half the line gap, then the ascent.
    baseline: f32,
    /// The advance of a character no face has: a space's, so the text keeps its rhythm.
    missing: f32,
}

impl Metrics {
    fn of(face: &Face, size: f32) -> Metrics {
        let mut metrics = Metrics {
            size,
            line_height: 0.,
            baseline: 0.,
            missing: size / 3.,
        };
        let scale = metrics.scale(face);
        let gap = face.line_gap() as f32 * scale;
        let ascent = face.ascender() as f32 * scale;
        let descent = -face.descender() as f32 * scale;
        metrics.line_height = ascent + descent + gap;
        metrics.baseline = gap / 2. + ascent;
        if let Some(space) = face.glyph_index(' ') {
            metrics.missing = metrics.advance_of(face, space);
        }
        metrics
    }

    /// Picture pixels per font unit of `face`.
    fn scale(&self, face: &Face) -> f32 {
        self.size / face.units_per_em() as f32
    }

    fn advance_of(&self, face: &Face, glyph: GlyphId) -> f32 {
        face.glyph_hor_advance(glyph).unwrap_or(0) as f32 * self.scale(face)
    }

    fn advance(&self, faces: &Faces, ch: char) -> f32 {
        match faces.glyph(ch) {
            Some((face, glyph)) => self.advance_of(face, glyph),
            None if is_invisible(ch) => 0.,
            None => self.missing,
        }
    }
}

/// Font units with y up in, picture pixels with y down out; `at` is the pen on the baseline.
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
        assert!(outline("", &font, 24., point(0., 0.)).is_none());
    }

    fn vt323() -> Font {
        Font {
            family: "VT323".into(),
            weight: FontWeight::NORMAL,
        }
    }

    #[test]
    fn a_character_no_face_has_takes_a_spaces_room_and_is_reported() {
        let font = vt323();
        // No font has a glyph for a private-use code point.
        let lost = '\u{10FFFD}';
        let with = layout(&format!("a{lost}b"), &font, 20.);
        let space = layout("a b", &font, 20.);
        assert_eq!(with.size.width, space.size.width);
        assert_eq!(
            dark_pixels(&draw(&lost.to_string(), &font, 20.)),
            0,
            "a missing glyph draws nothing"
        );
        assert!(has_blanks(&format!("a{lost}b"), &font));
        // Spaces, line breaks and joiners aren't drawn but aren't missed either.
        assert!(!has_blanks("a b\nc\u{200D}d\u{FE0F}", &font));
    }

    /// None of these is in VT323's subset.
    #[cfg(windows)]
    #[test]
    fn characters_the_face_lacks_come_from_system_fonts() {
        let font = vt323();
        for text in ["文", "あ", "한", "Ж", "😀", "ह"] {
            let face = font.face();
            let ch = text.chars().next().unwrap();
            assert!(face.glyph_index(ch).is_none(), "VT323 now has {text}");
            assert!(!has_blanks(text, &font), "{text} is a blank");
            assert!(
                dark_pixels(&draw(text, &font, 24.)) > 20,
                "{text} drew nothing"
            );
            assert!(
                layout(text, &font, 24.).size.width > 10.,
                "{text} has no width"
            );
        }
    }

    #[test]
    fn a_locale_names_its_cjk_language() {
        for (locale, cjk) in [
            ("ja-JP", Some(Cjk::Japanese)),
            ("ko-KR", Some(Cjk::Korean)),
            ("ko_KR.UTF-8", Some(Cjk::Korean)),
            ("zh-CN", Some(Cjk::Simplified)),
            ("zh-SG", Some(Cjk::Simplified)),
            ("zh", Some(Cjk::Simplified)),
            ("zh-TW", Some(Cjk::Traditional)),
            ("zh-HK", Some(Cjk::Traditional)),
            ("zh_MO", Some(Cjk::Traditional)),
            ("zh-Hant", Some(Cjk::Traditional)),
            ("zh-Hans-HK", Some(Cjk::Simplified)),
            ("JA-jp", Some(Cjk::Japanese)),
            ("en-US", None),
            ("jv-ID", None),
            ("", None),
        ] {
            assert_eq!(Cjk::of_locale(locale), cjk, "{locale}");
        }
    }

    #[test]
    fn kana_then_the_users_languages_pick_the_cjk_fonts() {
        let languages = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        // The display language comes first; the formats' locale counts when it isn't CJK.
        assert_eq!(
            Cjk::preferred("漢字", &languages(&["ja-JP", "zh-TW"])),
            Cjk::Japanese
        );
        assert_eq!(
            Cjk::preferred("漢字", &languages(&["en-US", "ko-KR"])),
            Cjk::Korean
        );
        assert_eq!(
            Cjk::preferred("漢字", &languages(&["zh-HK"])),
            Cjk::Traditional
        );
        assert_eq!(
            Cjk::preferred("漢字", &languages(&["en-GB", "fr-FR"])),
            Cjk::Simplified
        );
        assert_eq!(Cjk::preferred("漢字", &[]), Cjk::Simplified);
        for text in ["漢字かな", "カタカナ", "ｶﾀｶﾅ", "ㇰ"] {
            assert_eq!(
                Cjk::preferred(text, &languages(&["zh-CN"])),
                Cjk::Japanese,
                "{text}"
            );
        }
        // Hangul and Chinese punctuation are not kana.
        assert_eq!(
            Cjk::preferred("한국어。", &languages(&["zh-CN"])),
            Cjk::Simplified
        );
    }

    #[test]
    fn the_preferred_languages_fonts_move_up_to_the_first_cjk_font() {
        let fonts = [
            ("segoe", None),
            ("yahei", Some(Cjk::Simplified)),
            ("jhenghei", Some(Cjk::Traditional)),
            ("yugothic", Some(Cjk::Japanese)),
            ("meiryo", Some(Cjk::Japanese)),
            ("malgun", Some(Cjk::Korean)),
            ("nirmala", None),
        ];
        let order = |cjk| -> Vec<&str> {
            ordered(&fonts, cjk, |font| font.1)
                .into_iter()
                .map(|font| font.0)
                .collect()
        };
        assert_eq!(
            order(Cjk::Simplified),
            [
                "segoe", "yahei", "jhenghei", "yugothic", "meiryo", "malgun", "nirmala"
            ]
        );
        assert_eq!(
            order(Cjk::Japanese),
            [
                "segoe", "yugothic", "meiryo", "yahei", "jhenghei", "malgun", "nirmala"
            ]
        );
        assert_eq!(
            order(Cjk::Korean),
            [
                "segoe", "malgun", "yahei", "jhenghei", "yugothic", "meiryo", "nirmala"
            ]
        );
        assert_eq!(
            order(Cjk::Traditional),
            [
                "segoe", "jhenghei", "yahei", "yugothic", "meiryo", "malgun", "nirmala"
            ]
        );
    }

    #[cfg(windows)]
    fn family(face: &Face) -> String {
        face.names()
            .into_iter()
            .find(|name| {
                name.name_id == ttf_parser::name_id::FAMILY
                    && name.language() == ttf_parser::Language::English_UnitedStates
            })
            .and_then(|name| name.to_string())
            .unwrap_or_default()
    }

    #[cfg(windows)]
    #[test]
    fn kanji_with_kana_come_from_the_japanese_font() {
        let faces = Faces::new(&vt323(), "漢字かな");
        let (face, _) = faces.glyph('漢').unwrap();
        assert!(family(face).starts_with("Yu Gothic"), "{}", family(face));
        let (face, _) = faces.glyph('か').unwrap();
        assert!(family(face).starts_with("Yu Gothic"), "{}", family(face));
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
