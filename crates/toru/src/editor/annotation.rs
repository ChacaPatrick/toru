mod dragging;
mod draw;
mod hit;
mod outline;
mod sketch;
#[cfg(test)]
mod test_support;

use gpui::{Bounds, Point, point, size};
use tiny_skia::Color;

use super::block::BlockKind;
use super::sticker::{self, Sticker};
use super::text::{self, Font};
use super::tools::{Head, LineStyle, Width};

pub use dragging::in_picture;
pub use draw::paint_pixels;

const HIGHLIGHTER_WIDTH_SCALE: f32 = 6.;

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

#[derive(Clone, PartialEq, Debug)]
pub struct Text {
    pub content: String,
    pub font: Font,
    /// In picture pixels.
    pub size: f32,
    /// Set in capitals, as NeoBrutalism's `.anno.text` is.
    pub upper: bool,
}

impl Text {
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

#[derive(Clone, PartialEq, Debug)]
pub struct Badge {
    pub number: u32,
    /// In picture pixels; the digits are half as tall.
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
    Block(BlockKind),
}

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
    Block,
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
            Shape::Block(_) => Kind::Block,
        }
    }

    pub fn is_trail(&self) -> bool {
        matches!(self, Shape::Pen | Shape::Highlighter)
    }

    fn is_line(&self) -> bool {
        matches!(self, Shape::Arrow(_) | Shape::Line)
    }

    /// Placed with a click and sized on its own, rather than dragged out.
    pub fn is_placed(&self) -> bool {
        matches!(self, Shape::Text(_) | Shape::Number(_) | Shape::Sticker(_))
    }

    pub fn is_block(&self) -> bool {
        matches!(self, Shape::Block(_))
    }
}

/// Index into `Annotation::handles()`.
pub type HandleIndex = usize;

#[derive(Clone, PartialEq, Debug)]
pub struct Annotation {
    pub shape: Shape,
    /// The two ends of a line or two corners of a box, or every point of a trail. In
    /// whole-picture pixels, so a crop leaves every mark where it was.
    pub points: Vec<Point<f32>>,
    pub style: Style,
}

impl Annotation {
    pub fn new(shape: Shape, at: Point<f32>, style: Style) -> Annotation {
        Annotation {
            shape,
            points: vec![at, at],
            style,
        }
    }

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

    /// A click without a drag draws nothing, and nor does a box dragged along one axis only,
    /// which would be a line or a crop of no size.
    pub fn is_drawn(&self) -> bool {
        let size = self.bounds().size;
        if self.shape.is_placed() {
            true
        } else if self.shape.is_line() || self.shape.is_trail() {
            size.width >= 1. || size.height >= 1.
        } else {
            size.width >= 1. && size.height >= 1.
        }
    }

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

    pub fn text(&self) -> Option<&Text> {
        match &self.shape {
            Shape::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn with_content(&self, content: String) -> Annotation {
        let mut out = self.clone();
        if let Shape::Text(text) = &mut out.shape {
            text.content = content;
        }
        out
    }

    /// The caret's top after the last character, and its height, in picture pixels.
    pub fn caret(&self) -> Option<(Point<f32>, f32)> {
        let layout = self.text()?.layout();
        Some((self.points[0] + layout.caret, layout.line_height))
    }

    /// A line's two ends, else the box's corners in reading order, as the mockup draws them.
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

    /// In picture pixels.
    pub fn pixel_width(&self) -> f32 {
        let scale = match self.shape {
            Shape::Highlighter => HIGHLIGHTER_WIDTH_SCALE,
            _ if self.shape.is_placed() || self.shape.is_block() => 0.,
            _ => 1.,
        };
        self.style.width.pixels() * scale
    }

    /// One sticker design unit, in picture pixels.
    fn sticker_unit(&self) -> f32 {
        self.bounds().size.width / sticker::UNITS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::annotation::test_support::*;

    #[test]
    fn a_click_draws_nothing_but_a_drag_does() {
        let dot = Annotation::new(
            Shape::Arrow(Head::Single),
            point(4., 4.),
            style(LineStyle::Solid),
        );
        assert!(!dot.is_drawn());
        assert!(mark(Shape::Arrow(Head::Single), (4., 4.), (9., 4.)).is_drawn());
        // A box needs both sides; a line or trail along one axis is drawn.
        assert!(!mark(Shape::Rectangle, (4., 4.), (40., 4.)).is_drawn());
        assert!(!mark(Shape::Block(BlockKind::Solid), (4., 4.), (4., 40.)).is_drawn());
        assert!(mark(Shape::Rectangle, (4., 4.), (40., 5.)).is_drawn());
        assert!(mark(Shape::Pen, (4., 4.), (40., 4.)).is_drawn());
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
}
