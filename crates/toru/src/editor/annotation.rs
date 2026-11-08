//! The marks the tools leave (DESIGN.md §5): arrows, lines, rectangles, ellipses, pen and
//! highlighter trails, which are dragged out, and text, number badges and stickers, which are
//! placed with a click. Each is a shape, its points in picture pixels and a style, and knows how
//! to draw itself with `tiny-skia`, where its handles are, how to move and resize, and whether a
//! point hits it. Everything here is plain arithmetic on picture pixels, so the same code serves
//! the canvas, the saved file and the tests.

use gpui::{Bounds, Point, Size, point, size};
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, IntRect, LineCap, LineJoin, Paint, Path,
    PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, StrokeDash, Transform,
};

use super::sticker::{self, Part, Sticker};
use super::text::{self, Font};
use super::tools::{Head, LineStyle, Width};

/// How many picture pixels wide the highlighter is for each `Width`, against the other tools' 1.
const HIGHLIGHTER_WIDTHS: f32 = 6.;
/// A sketchy line is wobbled every this many pixels, by up to `SKETCH_AMPLITUDE` plus half the
/// stroke, and drawn twice so the two passes read as a rough stroke.
const SKETCH_STEP: f32 = 12.;
const SKETCH_AMPLITUDE: f32 = 1.2;
const SKETCH_PASSES: u32 = 2;
/// A dash and the gap after it, in stroke widths.
const DASH: f32 = 3.;
const GAP: f32 = 2.;
/// An arrow head's length in stroke widths, and the least it may be.
const HEAD_WIDTHS: f32 = 4.;
const HEAD_MIN: f32 = 12.;
/// The long head is this much longer and this much narrower.
const LONG_HEAD: f32 = 1.6;
const HEAD_ANGLE: f32 = 0.52;
const LONG_HEAD_ANGLE: f32 = 0.35;
/// A curly arrow's shaft waves this far out, every this many pixels.
const CURL_AMPLITUDE: f32 = 4.;
const CURL_LENGTH: f32 = 28.;
/// The ellipse is drawn as a polygon of this many sides when sketchy.
const ELLIPSE_SIDES: usize = 48;
/// For hit testing, the ellipse's polygon has enough sides to stay within this many pixels of
/// the true curve.
const ELLIPSE_HIT_ERROR: f32 = 0.5;
/// A mark's layer reaches this far past the picture it is drawn on. The rasterizer shades a
/// pixel at a layer's edge a little differently from one inside, so the edge is kept out of
/// sight and a region of the picture comes out the same as the whole.
const LAYER_MARGIN: i32 = 2;
/// How far past its line box text may paint, as a share of its size: a glyph's side bearing
/// can be negative and a swash can rise above the ascent.
const TEXT_OVERHANG: f32 = 0.25;

/// How a mark is drawn: the panel's choices, with the swatch resolved to its colour.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Style {
    /// The stroke, the text, a badge's disc or a sticker's fill.
    pub color: Color,
    /// A badge's digits and a sticker's outline; the strokes and text have no use for it.
    pub second: Color,
    pub width: Width,
    pub line: LineStyle,
    /// 0 to 1.
    pub opacity: f32,
}

/// A text annotation's content and how it is set, in picture pixels.
#[derive(Clone, PartialEq, Debug)]
pub struct Text {
    pub content: String,
    pub font: Font,
    /// The font size in picture pixels.
    pub size: f32,
    /// Set in capitals, as NeoBrutalism's `.anno.text` is.
    pub upper: bool,
}

impl Text {
    /// The content as it is set: in capitals if the look asks.
    pub fn shown(&self) -> String {
        if self.upper {
            self.content.to_uppercase()
        } else {
            self.content.clone()
        }
    }

    fn layout(&self) -> text::Layout {
        text::layout(&self.shown(), &self.font, self.size)
    }
}

/// A number badge: a disc with its number in it.
#[derive(Clone, PartialEq, Debug)]
pub struct Badge {
    pub number: u32,
    /// The disc's diameter in picture pixels; the digits are half as tall.
    pub diameter: f32,
    pub font: Font,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Shape {
    Arrow(Head),
    Line,
    Rectangle,
    Ellipse,
    Pen,
    Highlighter,
    /// Its point is the text's top-left corner.
    Text(Text),
    /// Its point is the disc's centre.
    Number(Badge),
    /// Its two points are the sticker's box, always square.
    Sticker(Sticker),
}

/// A shape's kind alone, for what the panel shows and the options a tool keeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Pen,
    Highlighter,
    Text,
    Number,
    Sticker,
}

impl Shape {
    pub fn kind(&self) -> Kind {
        match self {
            Shape::Arrow(_) => Kind::Arrow,
            Shape::Line => Kind::Line,
            Shape::Rectangle => Kind::Rectangle,
            Shape::Ellipse => Kind::Ellipse,
            Shape::Pen => Kind::Pen,
            Shape::Highlighter => Kind::Highlighter,
            Shape::Text(_) => Kind::Text,
            Shape::Number(_) => Kind::Number,
            Shape::Sticker(_) => Kind::Sticker,
        }
    }

    /// Whether the mark is a trail of points rather than two ends.
    pub fn is_trail(&self) -> bool {
        matches!(self, Shape::Pen | Shape::Highlighter)
    }

    /// Whether the mark's two points are a line's ends, which the handles sit on, rather than a
    /// box's corners.
    fn is_line(&self) -> bool {
        matches!(self, Shape::Arrow(_) | Shape::Line)
    }

    /// Whether the mark is placed with a click and has a size of its own, rather than dragged
    /// out: text, a badge or a sticker.
    pub fn is_placed(&self) -> bool {
        matches!(self, Shape::Text(_) | Shape::Number(_) | Shape::Sticker(_))
    }
}

/// Which of a mark's handles: the index into `handles()`.
pub type HandleIndex = usize;

#[derive(Clone, PartialEq, Debug)]
pub struct Annotation {
    pub shape: Shape,
    /// The two ends of a line or two corners of a box, or every point of a trail, in picture
    /// pixels.
    pub points: Vec<Point<f32>>,
    pub style: Style,
}

impl Annotation {
    /// A mark just begun at `at`.
    pub fn new(shape: Shape, at: Point<f32>, style: Style) -> Annotation {
        Annotation {
            shape,
            points: vec![at, at],
            style,
        }
    }

    /// Follows the pointer to `to`: the second point of a line or box, or the next point of a
    /// trail. `constrain` is Shift: a line snaps to 45°, a box becomes a square, a trail is drawn
    /// straight from its start. The mark stays within a picture `within` big: the pointer is held
    /// at its edge, and a snapped or squared end is cut short so it fits too.
    pub fn extend(&mut self, to: Point<f32>, constrain: bool, within: Size<f32>) {
        let to = in_picture(within, to);
        let start = self.points[0];
        if self.shape.is_trail() && !constrain {
            if self.points.len() == 2 && self.points[1] == start {
                self.points.pop();
            }
            self.points.push(to);
            return;
        }
        let end = if !constrain || self.shape.is_trail() {
            to
        } else if self.shape.is_line() {
            fit(start, snap_angle(start, to), within)
        } else {
            fit(start, square(start, to), within)
        };
        self.points.truncate(1);
        self.points.push(end);
    }

    /// A text, badge or sticker placed with a click: text at its top-left corner, a badge and a
    /// sticker centred on the click, the sticker `side` wide.
    pub fn placed(shape: Shape, at: Point<f32>, side: f32, style: Style) -> Annotation {
        let points = match &shape {
            Shape::Sticker(_) => vec![
                point(at.x - side / 2., at.y - side / 2.),
                point(at.x + side / 2., at.y + side / 2.),
            ],
            _ => vec![at],
        };
        Annotation {
            shape,
            points,
            style,
        }
    }

    /// Whether the mark is big enough to keep: a click without a drag draws nothing, but a
    /// placed mark has its own size.
    pub fn is_drawn(&self) -> bool {
        let bounds = self.bounds();
        self.shape.is_placed() || bounds.size.width >= 1. || bounds.size.height >= 1.
    }

    /// The box around the mark: its points, or what a placed mark takes up.
    pub fn bounds(&self) -> Bounds<f32> {
        match &self.shape {
            Shape::Text(text) => Bounds::new(self.points[0], text.layout().size),
            Shape::Number(badge) => {
                let r = badge.diameter / 2.;
                Bounds::new(
                    point(self.points[0].x - r, self.points[0].y - r),
                    size(badge.diameter, badge.diameter),
                )
            }
            _ => {
                let (mut min, mut max) = (self.points[0], self.points[0]);
                for p in &self.points {
                    min = point(min.x.min(p.x), min.y.min(p.y));
                    max = point(max.x.max(p.x), max.y.max(p.y));
                }
                Bounds::new(min, size(max.x - min.x, max.y - min.y))
            }
        }
    }

    /// The text of a text mark, to show or edit.
    pub fn text(&self) -> Option<&Text> {
        match &self.shape {
            Shape::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The text mark with `content` in place of its own.
    pub fn with_content(&self, content: String) -> Annotation {
        let mut out = self.clone();
        if let Shape::Text(text) = &mut out.shape {
            text.content = content;
        }
        out
    }

    /// Where the caret stands after a text mark's last character: its top, and its height, in
    /// picture pixels.
    pub fn caret(&self) -> Option<(Point<f32>, f32)> {
        let layout = self.text()?.layout();
        Some((self.points[0] + layout.caret, layout.line_height))
    }

    /// Where the handles are: a line's two ends, else the four corners of the box, top-left,
    /// top-right, bottom-left, bottom-right as the mockup draws them.
    pub fn handles(&self) -> Vec<Point<f32>> {
        if self.shape.is_line() {
            return self.points.clone();
        }
        let b = self.bounds();
        vec![
            b.origin,
            point(b.right(), b.top()),
            point(b.left(), b.bottom()),
            point(b.right(), b.bottom()),
        ]
    }

    /// The mark shifted by `delta`, kept within a picture `within` big. `constrain` is Shift,
    /// which keeps the move to a multiple of 45°: a snapped move that would leave the picture is
    /// cut short along its direction, where a free one slides along the edge.
    pub fn moved(&self, delta: Point<f32>, constrain: bool, within: Size<f32>) -> Annotation {
        let b = self.bounds();
        // How far the box may go each way on each axis without leaving the picture.
        let (left, right) = (-b.left(), within.width - b.right());
        let (up, down) = (-b.top(), within.height - b.bottom());
        let delta = if constrain {
            let delta = snap_angle(point(0., 0.), delta);
            let room = |d: f32, back: f32, forward: f32| {
                if d > 0. {
                    forward.max(0.) / d
                } else if d < 0. {
                    back.min(0.) / d
                } else {
                    1.
                }
            };
            let t = room(delta.x, left, right)
                .min(room(delta.y, up, down))
                .clamp(0., 1.);
            point(delta.x * t, delta.y * t)
        } else {
            // A box bigger than the picture has no room either way and moves freely.
            let held = |d: f32, back: f32, forward: f32| {
                if back <= forward {
                    d.clamp(back, forward)
                } else {
                    d
                }
            };
            point(held(delta.x, left, right), held(delta.y, up, down))
        };
        Annotation {
            points: self.points.iter().map(|p| *p + delta).collect(),
            ..self.clone()
        }
    }

    /// A placed mark scaled to fill `b`, which keeps its proportions: text by its font size, a
    /// badge by its diameter, a sticker by its box.
    fn fitted_to(&self, b: Bounds<f32>) -> Annotation {
        let old = self.bounds();
        let mut out = self.clone();
        match &mut out.shape {
            Shape::Text(text) => {
                text.size *= b.size.width / old.size.width;
                out.points = vec![b.origin];
            }
            Shape::Number(badge) => {
                badge.diameter = b.size.width;
                out.points = vec![b.center()];
            }
            _ => out.points = vec![b.origin, b.bottom_right()],
        }
        out
    }

    /// The mark with handle `handle` dragged to `to`: a line's end moves, a box's corner moves
    /// with the opposite corner still, and a trail is stretched to fit its new box. `constrain`
    /// is Shift, as when drawing: the line's end snaps to 45° from the other, the box stays
    /// square, and the trail keeps its proportions. The mark stays within a picture `within`
    /// big, and the end or corner that stays still stays still: a constrained end that would
    /// leave the picture is cut short instead.
    pub fn resized(
        &self,
        handle: HandleIndex,
        to: Point<f32>,
        constrain: bool,
        within: Size<f32>,
    ) -> Annotation {
        let to = in_picture(within, to);
        if self.shape.is_line() {
            let mut points = self.points.clone();
            let other = points[1 - handle];
            points[handle] = if constrain {
                fit(other, snap_angle(other, to), within)
            } else {
                to
            };
            return Annotation {
                points,
                ..self.clone()
            };
        }
        let old = self.bounds();
        // Handles 0 and 2 are on the left, 0 and 1 on the top.
        let (left, top) = (handle.is_multiple_of(2), handle < 2);
        let anchor = point(
            if left { old.right() } else { old.left() },
            if top { old.bottom() } else { old.top() },
        );
        if self.shape.is_placed() {
            // A placed mark keeps its proportions, Shift or not; text with nothing in it yet has
            // no width to scale by.
            if old.size.width < 1. || old.size.height < 1. {
                return self.clone();
            }
            let to = fit(anchor, proportional(anchor, to, old.size), within);
            return self.fitted_to(Bounds::from_corners(
                point(anchor.x.min(to.x), anchor.y.min(to.y)),
                point(anchor.x.max(to.x), anchor.y.max(to.y)),
            ));
        }
        if !self.shape.is_trail() {
            let to = if constrain {
                fit(anchor, square(anchor, to), within)
            } else {
                to
            };
            return Annotation {
                points: vec![anchor, to],
                ..self.clone()
            };
        }
        let to = if constrain {
            fit(anchor, proportional(anchor, to, old.size), within)
        } else {
            to
        };
        let stretch = |v: f32, from: f32, to: f32, anchor: f32| {
            let span = from - anchor;
            if span.abs() < f32::EPSILON {
                v
            } else {
                anchor + (v - anchor) * (to - anchor) / span
            }
        };
        let moving = point(
            if left { old.left() } else { old.right() },
            if top { old.top() } else { old.bottom() },
        );
        Annotation {
            points: self
                .points
                .iter()
                .map(|p| {
                    point(
                        stretch(p.x, moving.x, to.x, anchor.x),
                        stretch(p.y, moving.y, to.y, anchor.y),
                    )
                })
                .collect(),
            ..self.clone()
        }
    }

    /// Whether `at` is on the mark's stroke, within `tolerance` pixels of it: on any of the
    /// segments the mark is drawn from, so an arrow's head counts as well as its shaft. The
    /// ellipse is tested against a polygon fine enough to pass for the curve.
    pub fn hit(&self, at: Point<f32>, tolerance: f32) -> bool {
        if let Shape::Number(badge) = &self.shape {
            let c = self.points[0];
            return ((at.x - c.x).powi(2) + (at.y - c.y).powi(2)).sqrt()
                <= badge.diameter / 2. + tolerance;
        }
        if self.shape.is_placed() {
            // Text and stickers are solid: anywhere in their box picks them.
            let b = self.bounds();
            return at.x >= b.left() - tolerance
                && at.x <= b.right() + tolerance
                && at.y >= b.top() - tolerance
                && at.y <= b.bottom() + tolerance;
        }
        let reach = tolerance + self.pixel_width() / 2.;
        let outline = if self.shape == Shape::Ellipse {
            let b = self.bounds();
            let r = b.size.width.max(b.size.height) / 2.;
            let sides = if r > ELLIPSE_HIT_ERROR {
                let step = 2. * (1. - ELLIPSE_HIT_ERROR / r).acos();
                (std::f32::consts::TAU / step).ceil() as usize
            } else {
                0
            };
            vec![(ellipse(b, sides.max(ELLIPSE_SIDES)), true)]
        } else {
            self.polylines()
        };
        outline
            .iter()
            .any(|(points, closed)| polyline_distance(at, points, *closed) <= reach)
    }

    /// The stroke's width in picture pixels; a placed mark has none.
    pub fn pixel_width(&self) -> f32 {
        let widths = match self.shape {
            Shape::Highlighter => HIGHLIGHTER_WIDTHS,
            _ if self.shape.is_placed() => 0.,
            _ => 1.,
        };
        self.style.width.pixels() * widths
    }

    /// How far past its points the mark may paint: half its stroke, and an arrow's head, the
    /// curl of a curly shaft and the sketchy wobble, with a pixel over for anti-aliasing. Text
    /// may overhang its line box by a little of its size; a sticker by half its outline.
    pub fn reach(&self) -> f32 {
        match &self.shape {
            Shape::Text(text) => text.size * TEXT_OVERHANG + 1.,
            Shape::Number(_) => 1.,
            Shape::Sticker(_) => self.sticker_unit() * sticker::OUTLINE / 2. + 1.,
            _ => {
                let width = self.pixel_width();
                let head = (width * HEAD_WIDTHS).max(HEAD_MIN) * LONG_HEAD;
                width / 2. + head + CURL_AMPLITUDE + SKETCH_AMPLITUDE + 1.
            }
        }
    }

    /// A sticker's design unit in picture pixels: its box over the design box.
    fn sticker_unit(&self) -> f32 {
        self.bounds().size.width / sticker::UNITS
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

    /// Draws the mark on a picture whose top-left pixel is picture pixel `(0, 0)`.
    #[cfg(test)]
    pub fn draw(&self, picture: &mut Pixmap) {
        self.draw_at(picture, point(0, 0));
    }

    /// Draws the mark on `picture`, whose top-left pixel is picture pixel `origin`: the whole
    /// picture, or a region of it rendered on its own. The mark is stroked opaque on a layer of
    /// its own and that is laid on at its opacity, so the passes of a sketchy stroke and an
    /// arrow's head over its shaft don't darken where they cross.
    pub fn draw_at(&self, picture: &mut Pixmap, origin: Point<i32>) {
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

    /// The mark as straight segments, each with whether it closes: what the sketchy style
    /// wobbles and what the solid and dashed styles draw as they are.
    fn polylines(&self) -> Vec<(Vec<Point<f32>>, bool)> {
        let b = self.bounds();
        match &self.shape {
            Shape::Line => vec![(self.points.clone(), false)],
            Shape::Arrow(head) => arrow(self.points[0], self.points[1], *head, self.pixel_width()),
            Shape::Rectangle => vec![(
                vec![
                    b.origin,
                    point(b.right(), b.top()),
                    point(b.right(), b.bottom()),
                    point(b.left(), b.bottom()),
                ],
                true,
            )],
            Shape::Ellipse => vec![(ellipse(b, ELLIPSE_SIDES), true)],
            Shape::Pen | Shape::Highlighter => vec![(self.points.clone(), false)],
            Shape::Text(_) | Shape::Number(_) | Shape::Sticker(_) => vec![],
        }
    }
}

/// `to` snapped so the line from `from` runs at a multiple of 45°.
fn snap_angle(from: Point<f32>, to: Point<f32>) -> Point<f32> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0. {
        return to;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let angle = (dy.atan2(dx) / step).round() * step;
    point(from.x + length * angle.cos(), from.y + length * angle.sin())
}

/// `to` pulled in so the box from `from` is square, towards the pointer's side on each axis.
fn square(from: Point<f32>, to: Point<f32>) -> Point<f32> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let side = dx.abs().max(dy.abs());
    point(
        from.x + side.copysign(if dx == 0. { 1. } else { dx }),
        from.y + side.copysign(if dy == 0. { 1. } else { dy }),
    )
}

/// `to` pulled in so the box from `from` keeps the proportions of `size`, as large as fits
/// towards the pointer on each axis.
fn proportional(from: Point<f32>, to: Point<f32>, size: Size<f32>) -> Point<f32> {
    if size.width < 1. || size.height < 1. {
        return to;
    }
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let scale = (dx.abs() / size.width).min(dy.abs() / size.height);
    point(
        from.x + (size.width * scale).copysign(if dx == 0. { 1. } else { dx }),
        from.y + (size.height * scale).copysign(if dy == 0. { 1. } else { dy }),
    )
}

/// `at` held within a picture `picture` big: a drag that leaves it draws along its edge, not
/// in the margin where nothing shows.
pub fn in_picture(picture: Size<f32>, at: Point<f32>) -> Point<f32> {
    point(
        at.x.clamp(0., picture.width),
        at.y.clamp(0., picture.height),
    )
}

/// `to` pulled back towards `from` until it is within a picture `within` big, so a square from
/// `from` stays square and a snapped line keeps its angle, each as large as fits. `from` is
/// expected to be inside already.
fn fit(from: Point<f32>, to: Point<f32>, within: Size<f32>) -> Point<f32> {
    // How much of the way from `from` to `to` fits along one axis.
    let room = |from: f32, delta: f32, extent: f32| {
        if delta > 0. {
            (extent - from) / delta
        } else if delta < 0. {
            from / -delta
        } else {
            1.
        }
    };
    let delta = to - from;
    let t = room(from.x, delta.x, within.width)
        .min(room(from.y, delta.y, within.height))
        .clamp(0., 1.);
    in_picture(within, point(from.x + delta.x * t, from.y + delta.y * t))
}

/// The ellipse inscribed in `b` as a polygon of `sides` sides.
fn ellipse(b: Bounds<f32>, sides: usize) -> Vec<Point<f32>> {
    let centre = b.center();
    let (rx, ry) = (b.size.width / 2., b.size.height / 2.);
    (0..sides)
        .map(|i| {
            let angle = i as f32 / sides as f32 * std::f32::consts::TAU;
            point(centre.x + rx * angle.cos(), centre.y + ry * angle.sin())
        })
        .collect()
}

/// How far `p` is from the polyline, closed or open; a lone point counts as a dot.
fn polyline_distance(p: Point<f32>, points: &[Point<f32>], closed: bool) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return f32::INFINITY;
    };
    let closing = closed.then_some([*last, *first]);
    points
        .windows(2)
        .chain(closing.as_ref().map(|pair| &pair[..]))
        .map(|pair| segment_distance(p, pair[0], pair[1]))
        .chain(std::iter::once(segment_distance(p, *first, *first)))
        .fold(f32::INFINITY, f32::min)
}

/// How far `p` is from the segment `a`–`b`.
pub fn segment_distance(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let length2 = abx * abx + aby * aby;
    let t = if length2 == 0. {
        0.
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / length2).clamp(0., 1.)
    };
    let (cx, cy) = (a.x + abx * t, a.y + aby * t);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// An arrow from `from` to `to` as polylines: the shaft, then the open head at each pointed end.
fn arrow(from: Point<f32>, to: Point<f32>, head: Head, width: f32) -> Vec<(Vec<Point<f32>>, bool)> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0. {
        return vec![(vec![from, to], false)];
    }
    let (ux, uy) = (dx / length, dy / length);
    let mut head_length = (width * HEAD_WIDTHS).max(HEAD_MIN);
    let mut angle = HEAD_ANGLE;
    if head == Head::Long {
        head_length *= LONG_HEAD;
        angle = LONG_HEAD_ANGLE;
    }
    let head_length = head_length.min(length / 2.);
    let shaft = if head == Head::Curly {
        curl(from, to, (ux, uy), length)
    } else {
        vec![from, to]
    };
    let mut parts = vec![(shaft, false)];
    parts.push((chevron(to, (ux, uy), head_length, angle), false));
    if head == Head::Double {
        parts.push((chevron(from, (-ux, -uy), head_length, angle), false));
    }
    parts
}

/// The two strokes of an open arrow head at `tip`, pointing along `(ux, uy)`.
fn chevron(tip: Point<f32>, (ux, uy): (f32, f32), length: f32, angle: f32) -> Vec<Point<f32>> {
    let wing = |a: f32| {
        let (sin, cos) = a.sin_cos();
        point(
            tip.x - length * (ux * cos - uy * sin),
            tip.y - length * (uy * cos + ux * sin),
        )
    };
    vec![wing(angle), tip, wing(-angle)]
}

/// A curly arrow's shaft: a wave along the line, settling onto it at each end so the head sits
/// where the pointer is.
fn curl(from: Point<f32>, to: Point<f32>, (ux, uy): (f32, f32), length: f32) -> Vec<Point<f32>> {
    let steps = ((length / 3.).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let along = t * length;
            let fade = (t * std::f32::consts::PI).sin();
            let offset =
                (along / CURL_LENGTH * std::f32::consts::TAU).sin() * CURL_AMPLITUDE * fade;
            point(
                from.x + ux * along - uy * offset,
                from.y + uy * along + ux * offset,
            )
        })
        .chain(std::iter::once(to))
        .collect()
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

/// The polyline cut into short pieces, each joint nudged by a hash of where it is relative to
/// `origin`, the mark's own corner, so a mark wobbles the same way wherever it is moved to.
/// `pass` picks a different wobble per pass, and the ends stay put so the mark still meets what
/// it points at.
fn wobble(
    points: &[Point<f32>],
    closed: bool,
    origin: Point<f32>,
    amplitude: f32,
    pass: u32,
) -> Vec<Point<f32>> {
    let mut out = Vec::new();
    let count = points.len();
    let segments = if closed {
        count
    } else {
        count.saturating_sub(1)
    };
    for i in 0..segments {
        let a = points[i];
        let b = points[(i + 1) % count];
        let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let pieces = ((length / SKETCH_STEP).ceil() as usize).max(1);
        for k in 0..pieces {
            let t = k as f32 / pieces as f32;
            let p = point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let moves = closed || i > 0 || k > 0;
            out.push(if moves {
                nudge(p, origin, amplitude, pass)
            } else {
                p
            });
        }
    }
    if let (false, Some(last)) = (closed, points.last()) {
        out.push(*last);
    }
    out
}

fn nudge(p: Point<f32>, origin: Point<f32>, amplitude: f32, pass: u32) -> Point<f32> {
    let relative = p - origin;
    let dx = noise(relative, pass * 2) * amplitude;
    let dy = noise(relative, pass * 2 + 1) * amplitude;
    point(p.x + dx, p.y + dy)
}

/// A value in −1..=1 from the point's whole-pixel position and `salt`, with no state anywhere.
fn noise(p: Point<f32>, salt: u32) -> f32 {
    let mut h = (p.x.round() as i32 as u32).wrapping_mul(0x9E37_79B9);
    h ^= (p.y.round() as i32 as u32).wrapping_mul(0x85EB_CA6B);
    h ^= salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 32767.5 - 1.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(line: LineStyle) -> Style {
        Style {
            color: Color::from_rgba8(200, 0, 0, 255),
            second: Color::WHITE,
            width: Width::Medium,
            line,
            opacity: 1.,
        }
    }

    /// A picture big enough for everything the tests draw.
    const PICTURE: Size<f32> = Size {
        width: 10_000.,
        height: 10_000.,
    };

    fn mark(shape: Shape, from: (f32, f32), to: (f32, f32)) -> Annotation {
        let mut a = Annotation::new(shape, point(from.0, from.1), style(LineStyle::Solid));
        a.extend(point(to.0, to.1), false, PICTURE);
        a
    }

    fn is_red(picture: &Pixmap, x: u32, y: u32) -> bool {
        let p = picture.pixel(x, y).unwrap();
        p.red() > 150 && p.green() < 100
    }

    fn is_white(picture: &Pixmap, x: u32, y: u32) -> bool {
        let p = picture.pixel(x, y).unwrap();
        p.red() == 255 && p.green() == 255 && p.blue() == 255
    }

    fn white(width: u32, height: u32) -> Pixmap {
        let mut picture = Pixmap::new(width, height).unwrap();
        picture.fill(Color::WHITE);
        picture
    }

    #[test]
    fn shift_snaps_a_line_and_squares_a_box() {
        let mut line = Annotation::new(Shape::Line, point(0., 0.), style(LineStyle::Solid));
        line.extend(point(100., 10.), true, PICTURE);
        assert!((line.points[1].y).abs() < 1e-3);
        assert!((line.points[1].x - (100f32.powi(2) + 100.).sqrt()).abs() < 1e-3);
        let mut rect = Annotation::new(Shape::Rectangle, point(10., 10.), style(LineStyle::Solid));
        rect.extend(point(50., 20.), true, PICTURE);
        assert_eq!(rect.points[1], point(50., 50.));
        rect.extend(point(0., 20.), true, PICTURE);
        assert_eq!(rect.points[1], point(0., 20.));
    }

    #[test]
    fn shift_keeps_a_drawing_within_the_picture() {
        let picture = size(1000., 600.);
        // The square that fits from the start, not the pointer's 500 pushed off the bottom.
        let mut rect =
            Annotation::new(Shape::Rectangle, point(100., 500.), style(LineStyle::Solid));
        rect.extend(point(600., 550.), true, picture);
        assert_eq!(rect.points, vec![point(100., 500.), point(200., 600.)]);
        // A pointer past the edge is held at it first, then squared and fitted.
        rect.extend(point(1200., 560.), true, picture);
        assert_eq!(rect.points[1], point(200., 600.));
        // A snapped line stops at the edge along its 45°.
        let mut line = Annotation::new(Shape::Line, point(100., 500.), style(LineStyle::Solid));
        line.extend(point(300., 650.), true, picture);
        assert!((line.points[1].x - 200.).abs() < 1e-3);
        assert!((line.points[1].y - 600.).abs() < 1e-3);
        // Without Shift the pointer is simply held at the edge.
        let mut pen = Annotation::new(Shape::Pen, point(100., 500.), style(LineStyle::Solid));
        pen.extend(point(1100., 700.), false, picture);
        assert_eq!(pen.points[1], point(1000., 600.));
    }

    #[test]
    fn shift_keeps_a_resize_within_the_picture_and_its_anchor_still() {
        let picture = size(1000., 600.);
        let rect = mark(Shape::Rectangle, (900., 300.), (950., 350.));
        let squared = rect.resized(3, point(1100., 700.), true, picture);
        assert_eq!(
            squared.bounds(),
            Bounds::new(point(900., 300.), size(100., 100.))
        );
        // The top-left handle past the top edge: the pointer is held at it, and the square is
        // the 350 that reaches it from the bottom-right, which stays put.
        let squared = rect.resized(0, point(500., -50.), true, picture);
        assert_eq!(
            squared.bounds(),
            Bounds::new(point(600., 0.), size(350., 350.))
        );
        let line = mark(Shape::Line, (500., 100.), (600., 200.));
        let snapped = line.resized(1, point(1100., 150.), true, picture);
        assert_eq!(snapped.points[0], point(500., 100.));
        assert!((snapped.points[1].x - 1000.).abs() < 1e-3);
        assert!((snapped.points[1].y - 100.).abs() < 1e-3);
        let pen = mark(Shape::Pen, (800., 500.), (900., 550.));
        let scaled = pen.resized(3, point(1100., 700.), true, picture);
        assert_eq!(
            scaled.bounds(),
            Bounds::new(point(800., 500.), size(200., 100.))
        );
    }

    #[test]
    fn a_trail_collects_points_and_straightens_under_shift() {
        let mut pen = Annotation::new(Shape::Pen, point(0., 0.), style(LineStyle::Solid));
        pen.extend(point(5., 5.), false, PICTURE);
        pen.extend(point(10., 0.), false, PICTURE);
        assert_eq!(pen.points.len(), 3);
        // Straight from the start to the pointer itself, not squared or snapped away from it.
        pen.extend(point(20., 5.), true, PICTURE);
        assert_eq!(pen.points, vec![point(0., 0.), point(20., 5.)]);
        pen.extend(point(30., 5.), false, PICTURE);
        assert_eq!(pen.points.len(), 3);
    }

    #[test]
    fn a_click_draws_nothing_but_a_drag_does() {
        let dot = Annotation::new(
            Shape::Arrow(Head::Single),
            point(4., 4.),
            style(LineStyle::Solid),
        );
        assert!(!dot.is_drawn());
        assert!(mark(Shape::Arrow(Head::Single), (4., 4.), (9., 4.)).is_drawn());
    }

    #[test]
    fn handles_sit_on_a_lines_ends_and_a_boxs_corners() {
        let line = mark(Shape::Line, (10., 20.), (30., 40.));
        assert_eq!(line.handles(), vec![point(10., 20.), point(30., 40.)]);
        // Drawn from bottom-right to top-left, the corners are still in reading order.
        let rect = mark(Shape::Rectangle, (30., 40.), (10., 20.));
        assert_eq!(
            rect.handles(),
            vec![
                point(10., 20.),
                point(30., 20.),
                point(10., 40.),
                point(30., 40.)
            ]
        );
    }

    #[test]
    fn moving_shifts_every_point_and_shift_keeps_it_to_45_degrees() {
        let line = mark(Shape::Line, (10., 10.), (20., 20.));
        let moved = line.moved(point(5., -5.), false, PICTURE);
        assert_eq!(moved.points, vec![point(15., 5.), point(25., 15.)]);
        let constrained = line.moved(point(20., 3.), true, PICTURE);
        assert!((constrained.points[0].y - 10.).abs() < 1e-3);
        assert!((constrained.points[0].x - 10. - (400f32 + 9.).sqrt()).abs() < 1e-3);
    }

    #[test]
    fn resizing_keeps_the_opposite_corner_and_stretches_a_trail() {
        let rect = mark(Shape::Rectangle, (10., 10.), (30., 30.));
        // Dragging the top-left handle leaves the bottom-right where it was.
        let resized = rect.resized(0, point(0., 0.), false, PICTURE);
        assert_eq!(resized.bounds(), Bounds::new(point(0., 0.), size(30., 30.)));
        // The bottom-right handle leaves the top-left.
        let resized = rect.resized(3, point(50., 40.), false, PICTURE);
        assert_eq!(
            resized.bounds(),
            Bounds::new(point(10., 10.), size(40., 30.))
        );
        let line = mark(Shape::Arrow(Head::Single), (0., 0.), (10., 0.));
        assert_eq!(
            line.resized(1, point(20., 5.), false, PICTURE).points[1],
            point(20., 5.)
        );
        let mut pen = Annotation::new(Shape::Pen, point(0., 0.), style(LineStyle::Solid));
        pen.extend(point(10., 10.), false, PICTURE);
        pen.extend(point(20., 0.), false, PICTURE);
        let stretched = pen.resized(3, point(40., 10.), false, PICTURE);
        assert_eq!(stretched.points[1], point(20., 10.));
        assert_eq!(stretched.points[2], point(40., 0.));
        assert_eq!(stretched.bounds().size, size(40., 10.));
        // A flat trail keeps its flat axis rather than divide by nothing.
        let flat =
            mark(Shape::Pen, (0., 5.), (10., 5.)).resized(3, point(20., 20.), false, PICTURE);
        assert_eq!(flat.points[1], point(20., 5.));
    }

    #[test]
    fn shift_constrains_a_resize_as_it_does_a_drawing() {
        // A box's corner keeps it square, from the corner that stays.
        let rect = mark(Shape::Rectangle, (10., 10.), (30., 30.));
        let squared = rect.resized(3, point(50., 40.), true, PICTURE);
        assert_eq!(
            squared.bounds(),
            Bounds::new(point(10., 10.), size(40., 40.))
        );
        // A line's end snaps to 45° from its other end.
        let line = mark(Shape::Line, (0., 0.), (10., 0.));
        let snapped = line.resized(1, point(20., 3.), true, PICTURE);
        assert!(snapped.points[1].y.abs() < 1e-3);
        assert_eq!(snapped.points[0], point(0., 0.));
        // A trail keeps its proportions: twice as wide means twice as tall.
        let pen = mark(Shape::Pen, (0., 0.), (20., 10.));
        let scaled = pen.resized(3, point(40., 50.), true, PICTURE);
        assert_eq!(scaled.bounds().size, size(40., 20.));
    }

    #[test]
    fn hits_land_on_strokes_not_in_empty_boxes() {
        let rect = mark(Shape::Rectangle, (10., 10.), (110., 60.));
        assert!(rect.hit(point(60., 11.), 3.));
        assert!(rect.hit(point(108., 30.), 3.));
        assert!(!rect.hit(point(60., 35.), 3.));
        let ellipse = mark(Shape::Ellipse, (0., 0.), (100., 50.));
        assert!(ellipse.hit(point(100., 25.), 2.));
        assert!(ellipse.hit(point(50., 1.), 2.));
        assert!(!ellipse.hit(point(50., 25.), 2.));
        // A flat ellipse: along its long axis the outline is far away, even near the ends.
        let flat = mark(Shape::Ellipse, (0., 0.), (200., 20.));
        assert!(!flat.hit(point(170., 10.), 2.));
        assert!(!flat.hit(point(185., 10.), 2.));
        assert!(flat.hit(point(198., 10.), 2.));
        assert!(flat.hit(point(100., 1.), 2.));
        // A big one is hit along its curve, not just at the axes.
        let big = mark(Shape::Ellipse, (0., 0.), (2000., 1000.));
        let on = point(1000. + 1000. * 0.6f32.cos(), 500. + 500. * 0.6f32.sin());
        assert!(big.hit(on, 1.));
        assert!(!big.hit(point(on.x, on.y + 6.), 1.));
        let line = mark(Shape::Line, (0., 0.), (100., 0.));
        assert!(line.hit(point(50., 4.), 3.));
        assert!(!line.hit(point(50., 8.), 3.));
        assert!(!line.hit(point(110., 0.), 3.));
        assert_eq!(
            segment_distance(point(3., 4.), point(0., 0.), point(0., 0.)),
            5.
        );
    }

    #[test]
    fn an_arrows_head_can_be_hit_as_well_as_its_shaft() {
        let arrow = mark(Shape::Arrow(Head::Single), (10., 30.), (110., 30.));
        // The wings reach about 6 pixels above and below the shaft, 10 back from the tip.
        assert!(arrow.hit(point(100., 24.5), 1.));
        assert!(arrow.hit(point(100., 35.5), 1.));
        assert!(!arrow.hit(point(50., 24.5), 1.));
        let double = mark(Shape::Arrow(Head::Double), (10., 30.), (110., 30.));
        assert!(double.hit(point(20., 24.5), 1.));
        assert_eq!(polyline_distance(point(5., 5.), &[], false), f32::INFINITY);
        assert_eq!(
            polyline_distance(point(0., 0.), &[point(0., 10.), point(10., 0.)], true),
            segment_distance(point(0., 0.), point(10., 0.), point(0., 10.))
        );
    }

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
    fn a_sketchy_line_is_deterministic_and_stays_near_the_line() {
        let a = {
            let mut picture = white(120, 30);
            let mut line = mark(Shape::Line, (10., 15.), (110., 15.));
            line.style.line = LineStyle::Sketchy;
            line.draw(&mut picture);
            picture
        };
        let b = {
            let mut picture = white(120, 30);
            let mut line = mark(Shape::Line, (10., 15.), (110., 15.));
            line.style.line = LineStyle::Sketchy;
            line.draw(&mut picture);
            picture
        };
        assert_eq!(a.data(), b.data());
        assert!((0..120).all(|x| is_white(&a, x, 2) && is_white(&a, x, 27)));
        assert!((20..100).any(|x| is_red(&a, x, 15) || is_red(&a, x, 14) || is_red(&a, x, 16)));
        // Moved, the mark wobbles the same way relative to itself: every pixel is where it was,
        // shifted by the move.
        let moved = {
            let mut picture = white(130, 40);
            let mut line =
                mark(Shape::Line, (10., 15.), (110., 15.)).moved(point(7., 3.), false, PICTURE);
            line.style.line = LineStyle::Sketchy;
            line.draw(&mut picture);
            picture
        };
        for y in 0..30 {
            for x in 0..120 {
                assert_eq!(
                    a.pixel(x, y).unwrap(),
                    moved.pixel(x + 7, y + 3).unwrap(),
                    "pixel ({x}, {y})"
                );
            }
        }
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
    fn an_arrow_has_a_head_at_its_tip_and_a_double_one_at_both() {
        let mut picture = white(120, 60);
        mark(Shape::Arrow(Head::Single), (10., 30.), (110., 30.)).draw(&mut picture);
        // The head's wings reach above and below the shaft near the tip, not near the tail.
        assert!((95..110).any(|x| is_red(&picture, x, 24)));
        assert!((95..110).any(|x| is_red(&picture, x, 36)));
        assert!((12..30).all(|x| is_white(&picture, x, 22)));
        let mut picture = white(120, 60);
        mark(Shape::Arrow(Head::Double), (10., 30.), (110., 30.)).draw(&mut picture);
        assert!((10..25).any(|x| is_red(&picture, x, 24)));
    }

    #[test]
    fn a_curly_arrow_leaves_the_straight_line_and_a_long_head_is_longer() {
        let heads = |head| arrow(point(0., 0.), point(100., 0.), head, 3.);
        let tip_to_wing = |parts: &Vec<(Vec<Point<f32>>, bool)>| {
            let wing = parts[1].0[0];
            (wing.x - 100.).abs()
        };
        let single = heads(Head::Single);
        let long = heads(Head::Long);
        assert!(tip_to_wing(&long) > tip_to_wing(&single));
        let curly = heads(Head::Curly);
        assert!(curly[0].0.iter().any(|p| p.y.abs() > 1.));
        assert_eq!(*curly[0].0.last().unwrap(), point(100., 0.));
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

    fn inter() -> Font {
        Font {
            family: "Inter".into(),
            weight: gpui::FontWeight::SEMIBOLD,
        }
    }

    fn text(content: &str, at: (f32, f32), size: f32) -> Annotation {
        let shape = Shape::Text(Text {
            content: content.into(),
            font: inter(),
            size,
            upper: false,
        });
        Annotation::placed(shape, point(at.0, at.1), 0., style(LineStyle::Solid))
    }

    fn badge(number: u32, at: (f32, f32), diameter: f32) -> Annotation {
        let shape = Shape::Number(Badge {
            number,
            diameter,
            font: inter(),
        });
        Annotation::placed(shape, point(at.0, at.1), 0., style(LineStyle::Solid))
    }

    fn sticker(kind: Sticker, at: (f32, f32), side: f32) -> Annotation {
        Annotation::placed(
            Shape::Sticker(kind),
            point(at.0, at.1),
            side,
            style(LineStyle::Solid),
        )
    }

    #[test]
    fn a_shift_move_near_the_edge_keeps_its_angle() {
        let picture = size(1000., 600.);
        let rect = mark(Shape::Rectangle, (900., 100.), (950., 150.));
        // Only 50 fit to the right, so the 45° move stops at 50 on both axes.
        let moved = rect.moved(point(100., 100.), true, picture);
        assert_eq!(moved.bounds().origin, point(950., 150.));
        // A free move slides along the edge instead.
        let slid = rect.moved(point(100., 100.), false, picture);
        assert_eq!(slid.bounds().origin, point(950., 200.));
        // Pulled far up and left, the move stops where the box meets the top, still at 45°.
        let back = rect.moved(point(-1000., -1000.), true, picture);
        assert_eq!(back.bounds().origin, point(800., 0.));
        // Within the picture, a snapped move is the snapped move.
        let free = rect.moved(point(-20., -3.), true, picture);
        assert!((free.bounds().origin.x - (900. - (400f32 + 9.).sqrt())).abs() < 1e-3);
        assert!((free.bounds().origin.y - 100.).abs() < 1e-3);
    }

    #[test]
    fn placed_marks_have_their_own_size_and_are_hit_inside() {
        let text = text("Hi", (10., 20.), 16.);
        assert!(text.is_drawn());
        let b = text.bounds();
        assert_eq!(b.origin, point(10., 20.));
        assert!(b.size.width > 10. && b.size.height > 16.);
        assert!(text.hit(b.center(), 0.));
        assert!(!text.hit(point(b.right() + 5., b.center().y), 2.));
        assert_eq!(text.handles().len(), 4);
        let (caret, height) = text.caret().unwrap();
        assert_eq!(caret, point(b.right(), 20.));
        assert_eq!(height, b.size.height);
        // Empty text still has a line of height, and a caret at its corner.
        let empty = text.with_content(String::new());
        assert!(empty.is_drawn());
        assert_eq!(empty.bounds().size.width, 0.);
        assert_eq!(empty.caret().unwrap().0, point(10., 20.));
        let badge = badge(3, (50., 50.), 28.);
        assert_eq!(badge.bounds(), Bounds::new(point(36., 36.), size(28., 28.)));
        // 14 from the centre is in; the corner of its box at 14.1 is not.
        assert!(badge.hit(point(59., 59.), 0.));
        assert!(!badge.hit(point(60., 60.), 0.));
        assert!(badge.hit(point(63., 63.), 6.));
        let sticker = sticker(Sticker::Check, (100., 100.), 64.);
        assert_eq!(
            sticker.bounds(),
            Bounds::new(point(68., 68.), size(64., 64.))
        );
        assert!(sticker.hit(point(70., 130.), 0.));
    }

    #[test]
    fn placed_marks_resize_in_proportion_from_the_opposite_corner() {
        let text = text("Hello", (10., 10.), 16.);
        let b = text.bounds();
        // The bottom-right handle dragged to twice the width: the size doubles, the top-left
        // stays, and the height follows.
        let bigger = text.resized(
            3,
            point(10. + b.size.width * 2., 10. + b.size.height * 2.),
            false,
            PICTURE,
        );
        assert_eq!(bigger.text().unwrap().size, 32.);
        assert_eq!(bigger.bounds().origin, point(10., 10.));
        assert!((bigger.bounds().size.height - b.size.height * 2.).abs() < 0.01);
        // The top-left handle keeps the bottom-right still.
        let smaller = text.resized(0, point(10. + b.size.width / 2., 0.), true, PICTURE);
        assert_eq!(smaller.bounds().bottom_right(), b.bottom_right());
        assert_eq!(smaller.text().unwrap().size, 8.);
        // Empty text has nothing to scale by and stays as it is.
        let empty = text.with_content(String::new());
        assert_eq!(empty.resized(3, point(100., 100.), false, PICTURE), empty);
        let badge = badge(1, (50., 50.), 20.);
        let grown = badge.resized(3, point(80., 70.), false, PICTURE);
        assert_eq!(grown.bounds(), Bounds::new(point(40., 40.), size(30., 30.)));
        let sticker = sticker(Sticker::Cross, (50., 50.), 20.);
        // Dragged 60 right but only 30 up, the square follows the shorter axis.
        let grown = sticker.resized(1, point(100., 30.), false, PICTURE);
        assert_eq!(grown.bounds(), Bounds::new(point(40., 30.), size(30., 30.)));
        // Moving shifts a placed mark like any other.
        assert_eq!(
            badge.moved(point(5., 5.), false, PICTURE).points,
            vec![point(55., 55.)]
        );
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

    #[test]
    fn noise_is_bounded_and_depends_on_its_inputs() {
        let a = noise(point(10., 10.), 0);
        assert!((-1. ..=1.).contains(&a));
        assert_ne!(a, noise(point(10., 10.), 1));
        assert_ne!(a, noise(point(11., 10.), 0));
        assert_eq!(a, noise(point(10.2, 9.8), 0));
    }
}
