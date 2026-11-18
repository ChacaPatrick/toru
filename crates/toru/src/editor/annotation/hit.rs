use gpui::Point;

use super::outline::{ELLIPSE_SIDES, ellipse};
use super::{Annotation, Shape};

/// For hit testing, the ellipse's polygon has enough sides to stay within this many pixels of
/// the true curve.
const ELLIPSE_HIT_ERROR: f32 = 0.5;

impl Annotation {
    /// Any segment the mark is drawn from counts, so an arrow's head is hit as well as its shaft.
    pub fn hit(&self, at: Point<f32>, tolerance: f32) -> bool {
        if let Shape::Number(badge) = &self.shape {
            let c = self.points[0];
            return ((at.x - c.x).powi(2) + (at.y - c.y).powi(2)).sqrt()
                <= badge.diameter / 2. + tolerance;
        }
        if self.shape.is_placed() || self.shape.is_block() {
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
}

/// A lone point counts as a dot.
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

#[cfg(test)]
mod tests {
    use gpui::{Bounds, point, size};

    use super::*;
    use crate::editor::annotation::test_support::*;
    use crate::editor::sticker::Sticker;
    use crate::editor::tools::Head;

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
}
