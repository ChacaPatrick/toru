use gpui::{Point, point};

/// A sketchy line is wobbled every this many pixels, by up to `SKETCH_AMPLITUDE` plus half the
/// stroke, and drawn twice so the two passes read as a rough stroke.
const SKETCH_STEP: f32 = 12.;

pub(super) const SKETCH_AMPLITUDE: f32 = 1.2;

pub(super) const SKETCH_PASSES: u32 = 2;

/// Each joint is nudged by a hash of where it is relative to `origin`, the mark's own corner, so
/// a mark wobbles the same way wherever it is moved to. The ends stay put so the mark still
/// meets what it points at.
pub(super) fn wobble(
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

/// In −1..=1, from the point's whole-pixel position.
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
    use super::super::Shape;
    use super::*;
    use crate::editor::annotation::test_support::*;
    use crate::editor::tools::LineStyle;

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
        // Moved, the mark wobbles the same way relative to itself.
        let moved = {
            let mut picture = white(130, 40);
            let mut line = mark(Shape::Line, (10., 15.), (110., 15.)).moved(
                point(7., 3.),
                false,
                PICTURE.size,
            );
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
    fn noise_is_bounded_and_depends_on_its_inputs() {
        let a = noise(point(10., 10.), 0);
        assert!((-1. ..=1.).contains(&a));
        assert_ne!(a, noise(point(10., 10.), 1));
        assert_ne!(a, noise(point(11., 10.), 0));
        assert_eq!(a, noise(point(10.2, 9.8), 0));
    }
}
