//! The stickers (DESIGN.md §5): a pointing hand at eight angles, a check, a cross and a warning,
//! each drawn as vector shapes of our own in the theme's `sticker` fill and `sticker_line`
//! outline. A sticker is designed in a 24-unit box, as the mockups' icons are, and laid into
//! whatever box the annotation has, so it scales without a bitmap.

use gpui::Bounds;
use tiny_skia::{Path, PathBuilder, Transform};

/// The design box's side.
pub const UNITS: f32 = 24.;
/// The outline around a sticker's body, in design units: the mockup's `stroke-width`.
pub const OUTLINE: f32 = 1.2;
/// The check, cross and exclamation strokes, in design units.
pub const MARK: f32 = 2.4;
const CENTRE: f32 = UNITS / 2.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sticker {
    /// A hand pointing in one of eight directions: `0` is up, each step 45° clockwise.
    Hand(u8),
    Check,
    Cross,
    Warning,
}

/// What a part of a sticker is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    /// Filled with the sticker colour and outlined in the line colour.
    Body,
    /// Stroked in the line colour, `MARK` units wide.
    Mark,
    /// Filled in the line colour.
    Dot,
}

impl Sticker {
    /// Every sticker, in the tray's order.
    pub const ALL: [Sticker; 11] = [
        Sticker::Hand(0),
        Sticker::Hand(1),
        Sticker::Hand(2),
        Sticker::Hand(3),
        Sticker::Hand(4),
        Sticker::Hand(5),
        Sticker::Hand(6),
        Sticker::Hand(7),
        Sticker::Check,
        Sticker::Cross,
        Sticker::Warning,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Sticker::Hand(0) => "Hand pointing up",
            Sticker::Hand(1) => "Hand pointing up and right",
            Sticker::Hand(2) => "Hand pointing right",
            Sticker::Hand(3) => "Hand pointing down and right",
            Sticker::Hand(4) => "Hand pointing down",
            Sticker::Hand(5) => "Hand pointing down and left",
            Sticker::Hand(6) => "Hand pointing left",
            Sticker::Hand(_) => "Hand pointing up and left",
            Sticker::Check => "Check mark",
            Sticker::Cross => "Cross",
            Sticker::Warning => "Warning",
        }
    }

    /// The sticker's parts laid into `b`, in the order they are drawn.
    pub fn parts(self, b: Bounds<f32>) -> Vec<(Path, Part)> {
        let scale = b.size.width / UNITS;
        let mut place = Transform::from_translate(b.origin.x, b.origin.y).pre_scale(scale, scale);
        if let Sticker::Hand(step) = self {
            place = place.pre_rotate_at(step as f32 * 45., CENTRE, CENTRE);
        }
        self.design()
            .into_iter()
            .filter_map(|(path, part)| Some((path.transform(place)?, part)))
            .collect()
    }

    /// The sticker in its design box.
    fn design(self) -> Vec<(Path, Part)> {
        match self {
            Sticker::Hand(_) => vec![(hand(), Part::Body)],
            Sticker::Check => vec![
                (disc(), Part::Body),
                (
                    polyline(&[(7., 12.3), (10.6, 15.9), (17.2, 8.6)]),
                    Part::Mark,
                ),
            ],
            Sticker::Cross => vec![
                (disc(), Part::Body),
                (polyline(&[(8., 8.), (16., 16.)]), Part::Mark),
                (polyline(&[(16., 8.), (8., 16.)]), Part::Mark),
            ],
            Sticker::Warning => vec![
                (triangle(), Part::Body),
                (polyline(&[(12., 8.6), (12., 14.2)]), Part::Mark),
                (
                    PathBuilder::from_circle(12., 17.6, 1.5).expect("a positive radius"),
                    Part::Dot,
                ),
            ],
        }
    }
}

/// The badge behind a check or cross.
fn disc() -> Path {
    PathBuilder::from_circle(CENTRE, CENTRE, 10.4).expect("a positive radius")
}

/// A warning triangle with softened corners, apex up.
fn triangle() -> Path {
    let mut p = PathBuilder::new();
    p.move_to(10.6, 3.2);
    p.quad_to(12., 1.8, 13.4, 3.2);
    p.line_to(21.4, 18.2);
    p.quad_to(22.4, 20.6, 20., 20.6);
    p.line_to(4., 20.6);
    p.quad_to(1.6, 20.6, 2.6, 18.2);
    p.close();
    p.finish().expect("a closed triangle")
}

/// A hand pointing up: the index finger raised from a fist, the other fingers curled over the
/// knuckles on the right and the thumb folded against the left. Traced clockwise from the base
/// of the finger.
fn hand() -> Path {
    let mut p = PathBuilder::new();
    p.move_to(8.3, 11.);
    p.line_to(8.3, 4.);
    p.cubic_to(8.3, 2.3, 11.7, 2.3, 11.7, 4.);
    p.line_to(11.7, 10.4);
    // Three curled fingers, each a knuckle bump, stepping down towards the little finger.
    p.cubic_to(11.9, 8.7, 14.7, 8.7, 14.8, 10.4);
    p.cubic_to(15., 9., 17.6, 9., 17.7, 10.8);
    p.cubic_to(17.9, 9.8, 20., 9.9, 20., 11.6);
    p.line_to(20., 17.4);
    p.cubic_to(20., 20., 18., 21.6, 15.4, 21.6);
    p.line_to(9.6, 21.6);
    p.cubic_to(7.2, 21.6, 5.6, 20.2, 5.6, 17.8);
    p.line_to(5.6, 15.4);
    // The thumb, folded across the fist, bulges out on the left.
    p.cubic_to(4.4, 15.2, 3.8, 14.3, 4.1, 13.2);
    p.cubic_to(4.4, 12., 5.7, 11.5, 6.7, 12.);
    p.cubic_to(7.3, 12.2, 7.9, 11.7, 8.3, 11.);
    p.close();
    p.finish().expect("a closed hand")
}

fn polyline(points: &[(f32, f32)]) -> Path {
    let mut p = PathBuilder::new();
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            p.move_to(*x, *y);
        } else {
            p.line_to(*x, *y);
        }
    }
    p.finish().expect("at least two points")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn every_sticker_fits_its_box() {
        let b = Bounds::new(point(10., 20.), size(48., 48.));
        for sticker in Sticker::ALL {
            let parts = sticker.parts(b);
            assert!(!parts.is_empty());
            for (path, _) in parts {
                let r = path.bounds();
                assert!(r.left() >= b.left() - 0.5, "{sticker:?} left {}", r.left());
                assert!(r.top() >= b.top() - 0.5, "{sticker:?} top {}", r.top());
                assert!(
                    r.right() <= b.right() + 0.5,
                    "{sticker:?} right {}",
                    r.right()
                );
                assert!(
                    r.bottom() <= b.bottom() + 0.5,
                    "{sticker:?} bottom {}",
                    r.bottom()
                );
            }
        }
    }

    #[test]
    fn the_hand_turns_about_the_boxs_centre() {
        let b = Bounds::new(point(0., 0.), size(24., 24.));
        let up = &Sticker::Hand(0).parts(b)[0].0;
        let down = &Sticker::Hand(4).parts(b)[0].0;
        // The finger tip at the top comes round to the bottom.
        assert!(up.bounds().top() < 3.);
        assert!(down.bounds().bottom() > 21.);
        assert!((up.bounds().top() - (24. - down.bounds().bottom())).abs() < 0.01);
        let right = &Sticker::Hand(2).parts(b)[0].0;
        assert!(right.bounds().right() > 21.);
    }

    #[test]
    fn marks_are_drawn_over_bodies() {
        for sticker in [Sticker::Check, Sticker::Cross, Sticker::Warning] {
            let parts = sticker.parts(Bounds::new(point(0., 0.), size(24., 24.)));
            assert_eq!(parts[0].1, Part::Body);
            assert!(parts[1..].iter().all(|(_, part)| *part != Part::Body));
        }
        assert_eq!(Sticker::ALL.len(), 11);
        assert_eq!(Sticker::Hand(7).label(), "Hand pointing up and left");
    }
}
