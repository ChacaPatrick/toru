use gpui::{Point, point};
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, IntRect, LineCap, LineJoin, Paint, Path, PathBuilder,
    Pixmap, PixmapPaint, Rect, Stroke, StrokeDash, Transform,
};

use super::outline::{CURL_AMPLITUDE, HEAD_MIN, HEAD_WIDTHS, LONG_HEAD};
use super::sketch::{SKETCH_AMPLITUDE, SKETCH_PASSES, wobble};
use super::{Annotation, Shape};
use crate::editor::sticker::{self, Part};
use crate::editor::tools::LineStyle;
use crate::editor::{block, text};

/// A dash and the gap after it, in stroke widths.
const DASH: f32 = 3.;

const GAP: f32 = 2.;

/// A mark's layer reaches this far past the picture it is drawn on. The rasterizer shades a
/// pixel at a layer's edge a little differently from one inside, so the edge is kept out of
/// sight and a region of the picture comes out the same as the whole.
const LAYER_MARGIN: i32 = 2;

/// How far past its line box text may paint, as a share of its size: a glyph's side bearing
/// can be negative and a swash can rise above the ascent.
const TEXT_OVERHANG: f32 = 0.25;

impl Annotation {
    /// How far past its points the mark may paint: half its stroke, and an arrow's head, the
    /// curl of a curly shaft and the sketchy wobble, with a pixel over for anti-aliasing. Text
    /// may overhang its line box by a little of its size; a sticker by half its outline. A
    /// block fills its box and nothing more.
    pub fn reach(&self) -> f32 {
        match &self.shape {
            Shape::Text(text) => text.size * TEXT_OVERHANG + 1.,
            Shape::Number(_) => 1.,
            Shape::Sticker(_) => self.sticker_unit() * sticker::OUTLINE / 2. + 1.,
            Shape::Block(_) => 0.,
            _ => {
                let width = self.pixel_width();
                let head = (width * HEAD_WIDTHS).max(HEAD_MIN) * LONG_HEAD;
                width / 2. + head + CURL_AMPLITUDE + SKETCH_AMPLITUDE + 1.
            }
        }
    }

    /// The whole pixels the mark may paint.
    pub fn paint_rect(&self) -> Option<IntRect> {
        let b = self.bounds();
        let reach = self.reach();
        IntRect::from_ltrb(
            (b.left() - reach).floor() as i32,
            (b.top() - reach).floor() as i32,
            (b.right() + reach).ceil() as i32,
            (b.bottom() + reach).ceil() as i32,
        )
    }

    /// A block's pixels over `base`, the picture with the blocks under this one applied, and
    /// the whole-picture rectangle they fill. `None` for any other mark, or a block off the
    /// picture.
    pub fn block_pixels(&self, base: &Pixmap) -> Option<(IntRect, Pixmap)> {
        let Shape::Block(kind) = self.shape else {
            return None;
        };
        block::render(kind, base, self.paint_rect()?, self.style.color)
    }

    /// Draws the mark on a picture whose top-left pixel is picture pixel `(0, 0)`.
    #[cfg(test)]
    pub fn draw(&self, picture: &mut Pixmap) {
        self.draw_at(picture, point(0, 0));
    }

    /// Draws the mark on `picture`, whose top-left pixel is picture pixel `origin`: the whole
    /// picture, or a region of it rendered on its own. The mark is stroked opaque on a layer of
    /// its own and that is laid on at its opacity, so the passes of a sketchy stroke and an
    /// arrow's head over its shaft don't darken where they cross. A block draws nothing here:
    /// its pixels depend on the picture under it, so `Document` takes them from `block_pixels`.
    pub fn draw_at(&self, picture: &mut Pixmap, origin: Point<i32>) {
        if self.shape.is_block() {
            return;
        }
        let Some(area) = IntRect::from_xywh(
            origin.x - LAYER_MARGIN,
            origin.y - LAYER_MARGIN,
            picture.width() + 2 * LAYER_MARGIN as u32,
            picture.height() + 2 * LAYER_MARGIN as u32,
        ) else {
            return;
        };
        let Some(rect) = self.paint_rect().and_then(|rect| rect.intersect(&area)) else {
            return;
        };
        let Some(mut layer) = Pixmap::new(rect.width(), rect.height()) else {
            return;
        };
        self.paint(
            &mut layer,
            Transform::from_translate(-rect.x() as f32, -rect.y() as f32),
        );
        let paint = PixmapPaint {
            opacity: self.style.opacity,
            blend_mode: if self.shape == Shape::Highlighter {
                BlendMode::Multiply
            } else {
                BlendMode::SourceOver
            },
            quality: FilterQuality::Nearest,
        };
        picture.draw_pixmap(
            rect.x() - origin.x,
            rect.y() - origin.y,
            layer.as_ref(),
            &paint,
            Transform::identity(),
            None,
        );
    }

    /// Paints the mark opaque onto `layer`, through `transform`: a stroke's paths stroked, text
    /// and a badge filled, a sticker's parts each its own way.
    fn paint(&self, layer: &mut Pixmap, transform: Transform) {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.style.color);
        let mut second = paint.clone();
        second.set_color(self.style.second);
        let fill = |layer: &mut Pixmap, path: &Path, paint: &Paint| {
            layer.fill_path(path, paint, FillRule::Winding, transform, None);
        };
        match &self.shape {
            Shape::Text(text) => {
                if let Some(path) =
                    text::outline(&text.shown(), &text.font, text.size, self.points[0])
                {
                    fill(layer, &path, &paint);
                }
                return;
            }
            Shape::Number(badge) => {
                let c = self.points[0];
                if let Some(disc) = PathBuilder::from_circle(c.x, c.y, badge.diameter / 2.) {
                    fill(layer, &disc, &paint);
                }
                // The digits, half the disc tall, centred on it as the mockup's flex box does.
                let digits = badge.number.to_string();
                let size = badge.diameter / 2.;
                let layout = text::layout(&digits, &badge.font, size);
                let origin = point(c.x - layout.size.width / 2., c.y - layout.size.height / 2.);
                if let Some(path) = text::outline(&digits, &badge.font, size, origin) {
                    fill(layer, &path, &second);
                }
                return;
            }
            Shape::Sticker(sticker) => {
                let unit = self.sticker_unit();
                let line = |width: f32| Stroke {
                    width: width * unit,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                for (path, part) in sticker.parts(self.bounds()) {
                    match part {
                        Part::Body => {
                            fill(layer, &path, &paint);
                            layer.stroke_path(
                                &path,
                                &second,
                                &line(sticker::OUTLINE),
                                transform,
                                None,
                            );
                        }
                        Part::Mark => {
                            layer.stroke_path(&path, &second, &line(sticker::MARK), transform, None)
                        }
                        Part::Dot => fill(layer, &path, &second),
                    }
                }
                return;
            }
            _ => {}
        }
        let width = self.pixel_width();
        let mut stroke = Stroke {
            width,
            line_cap: if self.shape == Shape::Highlighter {
                LineCap::Butt
            } else {
                LineCap::Round
            },
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        if self.style.line == LineStyle::Dashed {
            stroke.dash = StrokeDash::new(vec![width * DASH, width * GAP], 0.);
        }
        for path in self.paths() {
            layer.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    /// The paths that make the mark up, each stroked alike. A sketchy mark comes as its passes.
    fn paths(&self) -> Vec<Path> {
        let polylines = self.polylines();
        let sketchy = self.style.line == LineStyle::Sketchy;
        if !sketchy {
            if self.shape == Shape::Ellipse {
                let b = self.bounds();
                return Rect::from_xywh(b.origin.x, b.origin.y, b.size.width, b.size.height)
                    .and_then(PathBuilder::from_oval)
                    .into_iter()
                    .collect();
            }
            if self.shape.is_trail() {
                return smooth(&self.points).into_iter().collect();
            }
            return polylines
                .iter()
                .filter_map(|(points, closed)| polyline(points, *closed))
                .collect();
        }
        let amplitude = SKETCH_AMPLITUDE + self.pixel_width() / 2.;
        let origin = self.bounds().origin;
        (0..SKETCH_PASSES)
            .flat_map(|pass| {
                polylines.iter().filter_map(move |(points, closed)| {
                    polyline(&wobble(points, *closed, origin, amplitude, pass), *closed)
                })
            })
            .collect()
    }
}

/// Lays `pixels`, which fill whole-picture rectangle `rect`, on `picture`, whose top-left pixel
/// is picture pixel `origin`, replacing what was there.
pub fn paint_pixels(picture: &mut Pixmap, origin: Point<i32>, rect: IntRect, pixels: &Pixmap) {
    picture.draw_pixmap(
        rect.x() - origin.x,
        rect.y() - origin.y,
        pixels.as_ref(),
        &PixmapPaint {
            opacity: 1.,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );
}

/// A polyline, closed or open, as a path; `None` when it has no segment.
fn polyline(points: &[Point<f32>], closed: bool) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut path = PathBuilder::new();
    path.move_to(first.x, first.y);
    if rest.is_empty() {
        path.line_to(first.x, first.y);
    }
    for p in rest {
        path.line_to(p.x, p.y);
    }
    if closed {
        path.close();
    }
    path.finish()
}

/// A trail as a smooth curve: each segment bends through the midpoints of its neighbours.
fn smooth(points: &[Point<f32>]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut path = PathBuilder::new();
    path.move_to(first.x, first.y);
    match rest {
        [] => path.line_to(first.x, first.y),
        [only] => path.line_to(only.x, only.y),
        _ => {
            for pair in rest.windows(2) {
                let (control, next) = (pair[0], pair[1]);
                let mid = point((control.x + next.x) / 2., (control.y + next.y) / 2.);
                path.quad_to(control.x, control.y, mid.x, mid.y);
            }
            let last = rest[rest.len() - 1];
            path.line_to(last.x, last.y);
        }
    }
    path.finish()
}

#[cfg(test)]
mod tests {
    use tiny_skia::Color;

    use super::super::Style;
    use super::*;
    use crate::editor::annotation::test_support::*;
    use crate::editor::sticker::Sticker;
    use crate::editor::text::Font;
    use crate::editor::tools::{Head, Width};

    #[test]
    fn a_solid_line_colours_its_pixels_and_nothing_else() {
        let mut picture = white(40, 20);
        mark(Shape::Line, (5., 10.), (35., 10.)).draw(&mut picture);
        assert!(is_red(&picture, 20, 10));
        assert!(is_red(&picture, 6, 10));
        assert!(is_white(&picture, 20, 3));
        assert!(is_white(&picture, 2, 10));
    }

    #[test]
    fn a_dashed_line_has_gaps() {
        let mut picture = white(100, 10);
        let mut line = mark(Shape::Line, (0., 5.), (100., 5.));
        line.style.line = LineStyle::Dashed;
        line.draw(&mut picture);
        let painted: Vec<bool> = (0..100).map(|x| is_red(&picture, x, 5)).collect();
        assert!(painted.iter().any(|p| *p));
        assert!(painted.iter().any(|p| !*p));
        // 3 widths on, 2 off at width 3: the first gap begins around pixel 9.
        assert!(painted[4]);
        assert!(!painted[12]);
    }

    #[test]
    fn opacity_is_the_marks_not_each_pass_of_it() {
        // Two sketchy passes over each other at half strength stay at half strength.
        let mut picture = white(120, 30);
        let mut line = mark(Shape::Line, (10., 15.), (110., 15.));
        line.style.line = LineStyle::Sketchy;
        line.style.width = Width::Thick;
        line.style.opacity = 0.5;
        line.draw(&mut picture);
        let lightest = (0..120)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .map(|(x, y)| picture.pixel(x, y).unwrap().green())
            .min()
            .unwrap();
        assert!(lightest >= 125, "green fell to {lightest}");
        assert!(is_red(&picture, 60, 15) || picture.pixel(60, 15).unwrap().green() < 200);
        // So does an arrow's head where it meets the shaft.
        let mut picture = white(120, 60);
        let mut arrow = mark(Shape::Arrow(Head::Single), (10., 30.), (110., 30.));
        arrow.style.opacity = 0.5;
        arrow.draw(&mut picture);
        assert!(picture.pixel(108, 30).unwrap().green() >= 125);
    }

    #[test]
    fn a_region_draws_the_same_pixels_as_the_whole() {
        let mut whole = white(60, 40);
        let mut line = mark(Shape::Arrow(Head::Curly), (5., 8.), (55., 32.));
        line.style.line = LineStyle::Sketchy;
        line.style.opacity = 0.6;
        line.draw(&mut whole);
        for (ox, oy) in [(20, 10), (0, 0), (41, 21), (-3, 17)] {
            let mut region = white(20, 20);
            line.draw_at(&mut region, point(ox, oy));
            for y in 0..20 {
                for x in 0..20 {
                    let (wx, wy) = (x as i32 + ox, y as i32 + oy);
                    let expected = if (0..60).contains(&wx) && (0..40).contains(&wy) {
                        whole.pixel(wx as u32, wy as u32).unwrap()
                    } else {
                        // Past the picture there is nothing to compare with: the region shows
                        // what the whole would, were it bigger, so at least it is painted.
                        continue;
                    };
                    assert_eq!(
                        region.pixel(x, y).unwrap(),
                        expected,
                        "pixel ({x}, {y}) of the region at ({ox}, {oy})"
                    );
                }
            }
        }
        // Nothing paints outside the mark's reach.
        let rect = line.paint_rect().unwrap();
        for y in 0..40 {
            for x in 0..60 {
                let inside = (x as i32) >= rect.left()
                    && (x as i32) < rect.right()
                    && (y as i32) >= rect.top()
                    && (y as i32) < rect.bottom();
                assert!(
                    inside || is_white(&whole, x, y),
                    "({x}, {y}) painted out of reach"
                );
            }
        }
    }

    #[test]
    fn boxes_draw_their_outline_only() {
        let mut picture = white(60, 40);
        mark(Shape::Rectangle, (5., 5.), (55., 35.)).draw(&mut picture);
        assert!(is_red(&picture, 30, 5));
        assert!(is_red(&picture, 5, 20));
        assert!(is_white(&picture, 30, 20));
        let mut picture = white(60, 40);
        mark(Shape::Ellipse, (5., 5.), (55., 35.)).draw(&mut picture);
        assert!(is_red(&picture, 30, 5));
        assert!(is_red(&picture, 55, 20));
        assert!(is_white(&picture, 30, 20));
        assert!(is_white(&picture, 6, 6));
    }

    #[test]
    fn the_highlighter_is_wide_and_lets_the_picture_through() {
        let mut picture = white(60, 60);
        let mut mark = mark(Shape::Highlighter, (5., 30.), (55., 30.));
        mark.style.opacity = 0.5;
        mark.style.color = Color::from_rgba8(255, 220, 0, 255);
        mark.draw(&mut picture);
        assert_eq!(mark.pixel_width(), 18.);
        let p = picture.pixel(30, 30).unwrap();
        // Multiplied over white at half strength: red stays, blue is halved.
        assert_eq!(p.red(), 255);
        assert!(p.blue() > 100 && p.blue() < 160);
        assert!(!is_white(&picture, 30, 22));
        assert!(is_white(&picture, 30, 19));
    }

    #[test]
    fn opacity_fades_the_stroke() {
        let mut picture = white(20, 20);
        let mut line = mark(Shape::Line, (0., 10.), (20., 10.));
        line.style.opacity = 0.5;
        line.draw(&mut picture);
        let p = picture.pixel(10, 10).unwrap();
        assert!(p.green() > 100 && p.green() < 160);
    }

    #[test]
    fn a_single_point_pen_mark_draws_a_dot() {
        let mut picture = white(20, 20);
        let mut dot = Annotation::new(Shape::Pen, point(10., 10.), style(LineStyle::Solid));
        dot.style.width = Width::Thick;
        dot.draw(&mut picture);
        assert!(is_red(&picture, 10, 10));
    }

    /// Writes every shape in every style, with text in each bundled face, badges and the
    /// stickers, to the PNG named by `TORU_SAMPLE_SHEET`, for looking at the drawing by eye.
    /// Run with `--ignored`.
    #[test]
    #[ignore]
    fn writes_a_sample_sheet() {
        let Some(path) = std::env::var_os("TORU_SAMPLE_SHEET") else {
            return;
        };
        let mut picture = white(760, 640);
        let ink = Color::from_rgba8(0x2b, 0x2a, 0x26, 255);
        let red = Color::from_rgba8(0x7a, 0x5c, 0x3e, 255);
        let blue = Color::from_rgba8(0x2e, 0x86, 0xde, 255);
        let yellow = Color::from_rgba8(0xf1, 0xc4, 0x0f, 255);
        let styled = |color, width, line, opacity| Style {
            color,
            second: Color::WHITE,
            width,
            line,
            opacity,
        };
        let shapes = [
            Shape::Arrow(Head::Single),
            Shape::Arrow(Head::Double),
            Shape::Arrow(Head::Long),
            Shape::Arrow(Head::Curly),
            Shape::Line,
            Shape::Rectangle,
            Shape::Ellipse,
        ];
        let lines = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Sketchy];
        for (row, shape) in shapes.iter().enumerate() {
            for (column, line) in lines.iter().enumerate() {
                let x = 20. + column as f32 * 250.;
                let y = 20. + row as f32 * 60.;
                let color = [ink, red, blue][column];
                let width = Width::ALL[row % 3];
                let mut mark = Annotation::new(
                    shape.clone(),
                    point(x, y + 36.),
                    styled(color, width, *line, 1.),
                );
                mark.extend(point(x + 200., y + 4.), false, PICTURE);
                mark.draw(&mut picture);
            }
        }
        let trail: Vec<Point<f32>> = (0..=40)
            .map(|i| {
                let t = i as f32 / 40.;
                point(
                    20. + t * 320.,
                    445. - (t * std::f32::consts::TAU * 2.).sin() * 14.,
                )
            })
            .collect();
        let mut pen = Annotation::new(
            Shape::Pen,
            trail[0],
            styled(ink, Width::Medium, LineStyle::Solid, 1.),
        );
        for p in &trail[1..] {
            pen.extend(*p, false, PICTURE);
        }
        pen.draw(&mut picture);
        let mut highlighter = Annotation::new(
            Shape::Highlighter,
            point(400., 445.),
            styled(yellow, Width::Medium, LineStyle::Solid, 0.5),
        );
        highlighter.extend(point(740., 445.), false, PICTURE);
        let mut underline = Annotation::new(
            Shape::Line,
            point(410., 445.),
            styled(ink, Width::Thin, LineStyle::Dashed, 1.),
        );
        underline.extend(point(730., 445.), false, PICTURE);
        underline.draw(&mut picture);
        highlighter.draw(&mut picture);
        // Text in each bundled family, at the mockup's size and double it.
        let mut x = 20.;
        for font in crate::theme::FONTS.iter().step_by(2) {
            let mut label = text("Total", (x, 470.), 16.);
            if let Shape::Text(t) = &mut label.shape {
                t.font = Font {
                    family: font.family.into(),
                    weight: font.weight,
                };
            }
            label.style.color = red;
            label.draw(&mut picture);
            x += label.bounds().size.width + 24.;
        }
        let mut big = text("Wrong 123", (20., 500.), 32.);
        big.style.color = ink;
        big.draw(&mut picture);
        for n in 1..=3 {
            let mut badge = badge(n, (300. + n as f32 * 40., 520.), 28.);
            badge.style.color = red;
            badge.draw(&mut picture);
        }
        let fill = Color::from_rgba8(0xc4, 0x85, 0x5a, 255);
        for (i, kind) in Sticker::ALL.iter().enumerate() {
            let mut sticker = sticker(*kind, (50. + i as f32 * 64., 590.), 56.);
            sticker.style.color = fill;
            sticker.style.second = ink;
            sticker.draw(&mut picture);
        }
        picture.save_png(path).unwrap();
    }

    #[test]
    fn text_a_badge_and_a_sticker_draw_in_their_colours() {
        let mut picture = white(120, 60);
        text("Hello", (5., 5.), 24.).draw(&mut picture);
        let b = text("Hello", (5., 5.), 24.).bounds();
        let inked = (0..120)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|(x, y)| is_red(&picture, *x, *y))
            .collect::<Vec<_>>();
        assert!(inked.len() > 50);
        assert!(inked.iter().all(|(x, y)| {
            (*x as f32) >= b.left() - 1.
                && (*x as f32) <= b.right() + 1.
                && (*y as f32) <= b.bottom() + 1.
        }));
        // The badge is a red disc with white digits in the middle.
        let mut picture = white(60, 60);
        badge(1, (30., 30.), 28.).draw(&mut picture);
        assert!(is_red(&picture, 20, 30));
        assert!(is_white(&picture, 2, 2));
        assert!((26..35).any(|x| {
            let p = picture.pixel(x, 30).unwrap();
            p.red() > 200 && p.green() > 200
        }));
        // Capitals when asked.
        let lower = text("ab", (0., 0.), 20.).bounds().size.width;
        let mut upper = text("ab", (0., 0.), 20.);
        if let Shape::Text(t) = &mut upper.shape {
            t.upper = true;
        }
        assert!(upper.bounds().size.width > lower);
        // A sticker fills with its colour, outlined in the second.
        let mut picture = white(80, 80);
        let mut hand = sticker(Sticker::Hand(0), (40., 40.), 64.);
        hand.style.second = Color::from_rgba8(0, 0, 255, 255);
        hand.draw(&mut picture);
        assert!(is_red(&picture, 40, 55), "the fist is filled");
        assert!(is_white(&picture, 70, 20), "nothing outside the hand");
        let blue = (0..80)
            .flat_map(|x| (0..80).map(move |y| (x, y)))
            .filter(|(x, y)| {
                let p = picture.pixel(*x, *y).unwrap();
                p.blue() > 150 && p.red() < 100
            })
            .count();
        assert!(blue > 50, "the outline shows");
        let mut picture = white(80, 80);
        sticker(Sticker::Warning, (40., 40.), 64.).draw(&mut picture);
        // Above the exclamation mark the triangle is solid; at design (12, 17.6) the mark's dot
        // is in the second colour, here white.
        assert!(is_red(&picture, 40, 23), "the triangle is filled");
        assert!(is_red(&picture, 30, 52), "the triangle is filled");
        assert!(
            is_white(&picture, 40, 55),
            "the exclamation is in the second colour"
        );
        assert!(is_white(&picture, 40, 75), "nothing below the triangle");
    }
}
