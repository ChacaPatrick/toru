use gpui::{Bounds, Point, point};

use super::{Annotation, Shape};
use crate::editor::tools::Head;

/// An arrow head's length in stroke widths, and the least it may be.
pub(super) const HEAD_WIDTHS: f32 = 4.;

pub(super) const HEAD_MIN: f32 = 12.;

/// The long head is this much longer and this much narrower.
pub(super) const LONG_HEAD: f32 = 1.6;

const HEAD_ANGLE: f32 = 0.52;

const LONG_HEAD_ANGLE: f32 = 0.35;

/// A curly arrow's shaft waves this far out, every this many pixels.
pub(super) const CURL_AMPLITUDE: f32 = 4.;

const CURL_LENGTH: f32 = 28.;

/// The ellipse is drawn as a polygon of this many sides when sketchy.
pub(super) const ELLIPSE_SIDES: usize = 48;

impl Annotation {
    /// The mark as straight segments, each with whether it closes: what the sketchy style
    /// wobbles and what the solid and dashed styles draw as they are.
    pub(super) fn polylines(&self) -> Vec<(Vec<Point<f32>>, bool)> {
        let b = self.bounds();
        match &self.shape {
            Shape::Line => vec![(self.points.clone(), false)],
            Shape::Arrow(head) => arrow(self.points[0], self.points[1], *head, self.pixel_width()),
            Shape::Rectangle | Shape::Block(_) => vec![(
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

/// The ellipse inscribed in `b` as a polygon of `sides` sides.
pub(super) fn ellipse(b: Bounds<f32>, sides: usize) -> Vec<Point<f32>> {
    let centre = b.center();
    let (rx, ry) = (b.size.width / 2., b.size.height / 2.);
    (0..sides)
        .map(|i| {
            let angle = i as f32 / sides as f32 * std::f32::consts::TAU;
            point(centre.x + rx * angle.cos(), centre.y + ry * angle.sin())
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::annotation::test_support::*;

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
}
