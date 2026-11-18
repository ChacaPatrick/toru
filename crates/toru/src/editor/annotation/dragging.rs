use gpui::{Bounds, Point, Size, point, size};

use super::{Annotation, HandleIndex, Shape};

/// In picture pixels: a line's length, each side of a box, the shorter side of a placed mark.
/// Smaller would be hard to see and to grab again.
const MIN_SIZE: f32 = 8.;

impl Annotation {
    /// `constrain` is Shift: a line snaps to 45°, a box becomes a square, a trail is drawn
    /// straight from its start. A snapped or squared end is cut short to stay within the picture.
    pub fn extend(&mut self, to: Point<f32>, constrain: bool, within: Bounds<f32>) {
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

    /// `constrain` is Shift, which keeps the move to a multiple of 45°: a snapped move that would
    /// leave the picture is cut short along its direction, where a free one slides along the edge.
    pub fn moved(&self, delta: Point<f32>, constrain: bool, within: Size<f32>) -> Annotation {
        let b = self.bounds();
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

    /// `b` must have the mark's proportions.
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

    /// The opposite end or corner stays still, and a trail is stretched to its new box.
    /// `constrain` is Shift, as in `extend`; a trail then keeps its proportions.
    pub fn resized(
        &self,
        handle: HandleIndex,
        to: Point<f32>,
        constrain: bool,
        within: Bounds<f32>,
    ) -> Annotation {
        let to = in_picture(within, to);
        if self.shape.is_line() {
            let mut points = self.points.clone();
            let other = points[1 - handle];
            let end = if constrain {
                fit(other, snap_angle(other, to), within)
            } else {
                to
            };
            // Too short, the end is pushed out to the least length the way it points, or the
            // way it pointed before when it sits on the other end.
            let (dx, dy) = (end.x - other.x, end.y - other.y);
            let length = (dx * dx + dy * dy).sqrt();
            points[handle] = if length >= MIN_SIZE {
                end
            } else {
                let before = (points[handle].x - other.x, points[handle].y - other.y);
                let (dx, dy) = if length > 0. {
                    (dx, dy)
                } else if before != (0., 0.) {
                    before
                } else {
                    (1., 0.)
                };
                let length = (dx * dx + dy * dy).sqrt();
                let scale = MIN_SIZE / length;
                fit(
                    other,
                    point(other.x + dx * scale, other.y + dy * scale),
                    within,
                )
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
        // A box too small to tell keeps to this corner's side of the anchor.
        let moving = point(
            if left { old.left() } else { old.right() },
            if top { old.top() } else { old.bottom() },
        );
        // Held within the picture towards the anchor when the mark keeps its proportions, else
        // on each axis alone. Against the picture's edge the least size gives way.
        let held = |to: Point<f32>, least: Size<f32>, keep_proportions: bool| {
            let pushed = point(
                at_least(anchor.x, to.x, least.width, moving.x),
                at_least(anchor.y, to.y, least.height, moving.y),
            );
            if keep_proportions {
                fit(anchor, pushed, within)
            } else {
                in_picture(within, pushed)
            }
        };
        // A side already under the least size may stay as it is, but no smaller.
        let least = size(MIN_SIZE.min(old.size.width), MIN_SIZE.min(old.size.height));
        if self.shape.is_placed() {
            // A placed mark keeps its proportions, Shift or not; text with nothing in it yet has
            // no width to scale by.
            if old.size.width < 1. || old.size.height < 1. {
                return self.clone();
            }
            let to = fit(anchor, proportional(anchor, to, old.size), within);
            let shorter = old.size.width.min(old.size.height);
            let scale = MIN_SIZE.min(shorter) / shorter;
            let to = if (to.x - anchor.x).abs() / old.size.width >= scale {
                to
            } else {
                held(to, old.size * scale, true)
            };
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
            let to = held(to, least, constrain);
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
        // A flat axis isn't stretched at all, so the least size only matters on the others.
        // Under Shift the least size keeps the trail's proportions, its longer side at the least.
        let longer = old.size.width.max(old.size.height);
        let least = if constrain && longer > 0. {
            old.size * (MIN_SIZE.min(longer) / longer)
        } else {
            least
        };
        let to = held(to, least, constrain);
        let stretch = |v: f32, from: f32, to: f32, anchor: f32| {
            let span = from - anchor;
            if span.abs() < f32::EPSILON {
                v
            } else {
                anchor + (v - anchor) * (to - anchor) / span
            }
        };
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
}

fn snap_angle(from: Point<f32>, to: Point<f32>) -> Point<f32> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0. {
        return to;
    }
    // The eight directions written out, so a straight move is exactly straight: `cos` of a
    // right angle in `f32` is a hair off zero, enough to stop a move along an edge dead.
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let (ux, uy) = match (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32 {
        0 => (1., 0.),
        1 => (d, d),
        2 => (0., 1.),
        3 => (-d, d),
        4 | -4 => (-1., 0.),
        -3 => (-d, -d),
        -2 => (0., -1.),
        _ => (d, -d),
    };
    point(from.x + length * ux, from.y + length * uy)
}

/// `to` pushed away from `anchor` until they are at least `min` apart, on the side of `side`
/// when they coincide.
fn at_least(anchor: f32, to: f32, min: f32, side: f32) -> f32 {
    let delta = to - anchor;
    if delta.abs() >= min {
        to
    } else {
        anchor + min.copysign(if delta == 0. { side - anchor } else { delta })
    }
}

fn square(from: Point<f32>, to: Point<f32>) -> Point<f32> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let side = dx.abs().max(dy.abs());
    point(
        from.x + side.copysign(if dx == 0. { 1. } else { dx }),
        from.y + side.copysign(if dy == 0. { 1. } else { dy }),
    )
}

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

/// So a drag that leaves the picture draws along its edge, not in the margin where nothing shows.
pub fn in_picture(picture: Bounds<f32>, at: Point<f32>) -> Point<f32> {
    point(
        at.x.clamp(picture.left(), picture.right()),
        at.y.clamp(picture.top(), picture.bottom()),
    )
}

/// `to` pulled back towards `from` until it is within the picture, so a square stays square and
/// a snapped line keeps its angle. `from` must be inside already.
fn fit(from: Point<f32>, to: Point<f32>, within: Bounds<f32>) -> Point<f32> {
    let room = |from: f32, delta: f32, low: f32, high: f32| {
        if delta > 0. {
            (high - from) / delta
        } else if delta < 0. {
            (from - low) / -delta
        } else {
            1.
        }
    };
    let delta = to - from;
    let t = room(from.x, delta.x, within.left(), within.right())
        .min(room(from.y, delta.y, within.top(), within.bottom()))
        .clamp(0., 1.);
    in_picture(within, point(from.x + delta.x * t, from.y + delta.y * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::annotation::test_support::*;
    use crate::editor::sticker::Sticker;
    use crate::editor::tools::{Head, LineStyle};

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
        let picture = picture(1000., 600.);
        // The square that fits from the start, not the pointer's 500 pushed off the bottom.
        let mut rect =
            Annotation::new(Shape::Rectangle, point(100., 500.), style(LineStyle::Solid));
        rect.extend(point(600., 550.), true, picture);
        assert_eq!(rect.points, vec![point(100., 500.), point(200., 600.)]);
        // A pointer past the edge is held at it first, then squared and fitted.
        rect.extend(point(1200., 560.), true, picture);
        assert_eq!(rect.points[1], point(200., 600.));
        let mut line = Annotation::new(Shape::Line, point(100., 500.), style(LineStyle::Solid));
        line.extend(point(300., 650.), true, picture);
        assert!((line.points[1].x - 200.).abs() < 1e-3);
        assert!((line.points[1].y - 600.).abs() < 1e-3);
        let mut pen = Annotation::new(Shape::Pen, point(100., 500.), style(LineStyle::Solid));
        pen.extend(point(1100., 700.), false, picture);
        assert_eq!(pen.points[1], point(1000., 600.));
    }

    #[test]
    fn shift_keeps_a_resize_within_the_picture_and_its_anchor_still() {
        let picture = picture(1000., 600.);
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
    fn moving_shifts_every_point_and_shift_keeps_it_to_45_degrees() {
        let line = mark(Shape::Line, (10., 10.), (20., 20.));
        let moved = line.moved(point(5., -5.), false, PICTURE.size);
        assert_eq!(moved.points, vec![point(15., 5.), point(25., 15.)]);
        let constrained = line.moved(point(20., 3.), true, PICTURE.size);
        assert!((constrained.points[0].y - 10.).abs() < 1e-3);
        assert!((constrained.points[0].x - 10. - (400f32 + 9.).sqrt()).abs() < 1e-3);
    }

    #[test]
    fn resizing_keeps_the_opposite_corner_and_stretches_a_trail() {
        let rect = mark(Shape::Rectangle, (10., 10.), (30., 30.));
        // Handle 0 is the top-left.
        let resized = rect.resized(0, point(0., 0.), false, PICTURE);
        assert_eq!(resized.bounds(), Bounds::new(point(0., 0.), size(30., 30.)));
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
        let rect = mark(Shape::Rectangle, (10., 10.), (30., 30.));
        let squared = rect.resized(3, point(50., 40.), true, PICTURE);
        assert_eq!(
            squared.bounds(),
            Bounds::new(point(10., 10.), size(40., 40.))
        );
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
    fn a_shift_move_near_the_edge_keeps_its_angle() {
        let picture = size(1000., 600.);
        let rect = mark(Shape::Rectangle, (900., 100.), (950., 150.));
        // Only 50 fit to the right, so the 45° move stops at 50 on both axes.
        let moved = rect.moved(point(100., 100.), true, picture);
        assert_eq!(moved.bounds().origin, point(950., 150.));
        let slid = rect.moved(point(100., 100.), false, picture);
        assert_eq!(slid.bounds().origin, point(950., 200.));
        // Pulled far up and left, the move stops where the box meets the top, still at 45°.
        let back = rect.moved(point(-1000., -1000.), true, picture);
        assert_eq!(back.bounds().origin, point(800., 0.));
        let free = rect.moved(point(-20., -3.), true, picture);
        assert!((free.bounds().origin.x - (900. - (400f32 + 9.).sqrt())).abs() < 1e-3);
        assert!((free.bounds().origin.y - 100.).abs() < 1e-3);
    }

    #[test]
    fn a_shift_move_slides_along_the_edge_it_is_flush_against() {
        let picture = size(1000., 600.);
        // Flush left, top and right: a straight move along that edge goes the whole way.
        let left = mark(Shape::Rectangle, (0., 100.), (50., 150.));
        let moved = left.moved(point(-2., 90.), true, picture);
        assert_eq!(moved.bounds().origin.x, 0.);
        assert!(moved.bounds().origin.y > 189.);
        let top = mark(Shape::Rectangle, (100., 0.), (150., 50.));
        let moved = top.moved(point(-90., -3.), true, picture);
        assert_eq!(moved.bounds().origin.y, 0.);
        assert!(moved.bounds().origin.x < 11.);
        let right = mark(Shape::Rectangle, (950., 100.), (1000., 150.));
        let moved = right.moved(point(1., -90.), true, picture);
        assert_eq!(moved.bounds().origin.x, 950.);
        assert!(moved.bounds().origin.y < 11.);
    }

    #[test]
    fn a_resize_never_shrinks_a_mark_below_the_least_size() {
        // A box's corner dragged onto the opposite one leaves a small box on its old side.
        let rect = mark(Shape::Rectangle, (10., 10.), (30., 30.));
        let squashed = rect.resized(3, point(10., 10.), false, PICTURE);
        assert_eq!(
            squashed.bounds(),
            Bounds::new(point(10., 10.), size(MIN_SIZE, MIN_SIZE))
        );
        let flat = rect.resized(0, point(5., 30.), true, PICTURE);
        assert!(flat.bounds().size.width >= MIN_SIZE && flat.bounds().size.height >= MIN_SIZE);
        // A line's end dragged onto its other end stays the least length the way it pointed.
        let line = mark(Shape::Line, (10., 10.), (30., 10.));
        assert_eq!(
            line.resized(1, point(10., 10.), false, PICTURE).points[1],
            point(10. + MIN_SIZE, 10.)
        );
        assert_eq!(
            line.resized(1, point(12., 10.), true, PICTURE).points[1],
            point(10. + MIN_SIZE, 10.)
        );
        let pen = mark(Shape::Pen, (0., 0.), (20., 10.));
        let squashed = pen.resized(3, point(0., 0.), false, PICTURE);
        assert_eq!(squashed.bounds().size, size(MIN_SIZE, MIN_SIZE));
        // Placed marks keep their proportions, the shorter side at the least size.
        let badge = badge(1, (50., 50.), 20.);
        let squashed = badge.resized(3, point(40., 40.), false, PICTURE);
        assert_eq!(
            squashed.bounds(),
            Bounds::new(point(40., 40.), size(MIN_SIZE, MIN_SIZE))
        );
        let text = text("Hello", (10., 10.), 16.);
        let squashed = text.resized(3, point(10., 10.), false, PICTURE);
        let b = squashed.bounds();
        assert!((b.size.height - MIN_SIZE).abs() < 0.01 && b.size.width > MIN_SIZE);
        assert_eq!(b.origin, point(10., 10.));
        let grown = squashed.resized(3, point(10. + b.size.width * 4., 200.), false, PICTURE);
        assert!(grown.bounds().size.height > MIN_SIZE * 3.);
    }

    #[test]
    fn the_least_size_gives_way_to_the_picture() {
        // A crop over all of a picture narrower than the least size resizes back to all of it.
        let narrow = picture(5., 400.);
        let crop = mark(Shape::Rectangle, (0., 0.), (5., 400.));
        let resized = crop.resized(0, point(0., 0.), false, narrow);
        assert_eq!(resized.bounds(), Bounds::new(point(0., 0.), size(5., 400.)));
        // Pushed past the edge, a free resize is held on that axis alone.
        let wide = picture(1000., 600.);
        let rect = mark(Shape::Rectangle, (500., 100.), (996., 300.));
        let crossed = rect.resized(0, point(997., 200.), false, wide);
        assert_eq!(
            crossed.bounds(),
            Bounds::new(point(996., 200.), size(4., 100.))
        );
        let squared = rect.resized(0, point(997., 299.), true, wide);
        let b = squared.bounds();
        assert_eq!(b.size.width, b.size.height);
        assert!(b.right() <= 1000. && b.bottom() <= 600.);
        let pen = mark(Shape::Pen, (500., 100.), (996., 300.));
        let pushed = pen.resized(0, point(999., 100.), false, wide);
        assert!(pushed.bounds().right() <= 1000.);
        assert!(pushed.bounds().size.width > 0.);
        let badge = badge(1, (991., 300.), 10.);
        let pushed = badge.resized(0, point(998., 307.), false, wide);
        let b = pushed.bounds();
        assert!(b.right() <= 1000. && b.size.width > 0.);
        assert_eq!(b.size.width, b.size.height);
    }

    #[test]
    fn a_side_already_under_the_least_size_is_not_pushed_out() {
        // A nearly straight stroke widened stays nearly straight.
        let mut pen = Annotation::new(Shape::Pen, point(100., 100.), style(LineStyle::Solid));
        pen.extend(point(200., 101.), false, PICTURE);
        pen.extend(point(300., 100.), false, PICTURE);
        let widened = pen.resized(3, point(400., 101.), false, PICTURE);
        assert_eq!(
            widened.points,
            vec![point(100., 100.), point(250., 101.), point(400., 100.)]
        );
        // Shrunk to half under Shift, both sides halve.
        let halved = pen.resized(3, point(200., 101.), true, PICTURE);
        assert_eq!(halved.bounds().size, size(100., 0.5));
        let rect = mark(Shape::Rectangle, (100., 100.), (300., 103.));
        let widened = rect.resized(3, point(400., 103.), false, PICTURE);
        assert_eq!(
            widened.bounds(),
            Bounds::new(point(100., 100.), size(300., 3.))
        );
        // A narrow text grabbed by its corner, without moving, keeps its size.
        let narrow = text("i", (10., 20.), 16.);
        let b = narrow.bounds();
        assert!(b.size.width < MIN_SIZE);
        let held = narrow.resized(3, point(b.right(), b.bottom()), false, PICTURE);
        assert!((held.bounds().size.width - b.size.width).abs() < 0.01);
        assert!((held.bounds().size.height - b.size.height).abs() < 0.01);
    }

    #[test]
    fn placed_marks_resize_in_proportion_from_the_opposite_corner() {
        let text = text("Hello", (10., 10.), 16.);
        let b = text.bounds();
        // Twice the width doubles the size; the height follows.
        let bigger = text.resized(
            3,
            point(10. + b.size.width * 2., 10. + b.size.height * 2.),
            false,
            PICTURE,
        );
        assert_eq!(bigger.text().unwrap().size, 32.);
        assert_eq!(bigger.bounds().origin, point(10., 10.));
        assert!((bigger.bounds().size.height - b.size.height * 2.).abs() < 0.01);
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
        assert_eq!(
            badge.moved(point(5., 5.), false, PICTURE.size).points,
            vec![point(55., 55.)]
        );
    }
}
