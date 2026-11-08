//! Where the picture sits on the canvas: its zoom and the canvas position of its top-left
//! corner, in logical pixels. Plain arithmetic, so the zoom buttons, the wheel and panning all
//! go through the same few rules.

use gpui::{Bounds, Point, Size, point, size};

/// The zoom levels − and + step through; the wheel moves freely between them.
pub const STEPS: [f32; 12] = [0.1, 0.25, 0.33, 0.5, 0.67, 0.75, 1., 1.25, 1.5, 2., 3., 4.];
pub const MIN_ZOOM: f32 = STEPS[0];
pub const MAX_ZOOM: f32 = 8.;
/// One wheel notch, which Windows reports as this many lines, zooms by this much.
const NOTCH_LINES: f32 = 3.;
const ZOOM_PER_NOTCH: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Viewport {
    pub zoom: f32,
    /// The picture's top-left corner, in canvas coordinates.
    pub origin: Point<f32>,
}

impl Viewport {
    /// The picture as large as fits inside `area`, never above 100%, centred in it.
    pub fn fit(image: Size<f32>, area: Bounds<f32>) -> Viewport {
        let zoom = (area.size.width / image.width)
            .min(area.size.height / image.height)
            .clamp(MIN_ZOOM, 1.);
        let scaled = size(image.width * zoom, image.height * zoom);
        Viewport {
            zoom,
            origin: point(
                area.origin.x + (area.size.width - scaled.width) / 2.,
                area.origin.y + (area.size.height - scaled.height) / 2.,
            ),
        }
    }

    /// Zooms to `zoom`, clamped to the range, keeping the picture point under `anchor` where it
    /// is: the pointer for the wheel, the canvas centre for the buttons.
    pub fn zoom_to(self, zoom: f32, anchor: Point<f32>) -> Viewport {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let image_point = point(
            (anchor.x - self.origin.x) / self.zoom,
            (anchor.y - self.origin.y) / self.zoom,
        );
        Viewport {
            zoom,
            origin: point(
                anchor.x - image_point.x * zoom,
                anchor.y - image_point.y * zoom,
            ),
        }
    }

    /// The next step up (`1`) or down (`-1`) from the current zoom, or the current zoom when
    /// it is already at that end.
    pub fn step(&self, direction: i32) -> f32 {
        // A hair of tolerance, so 50% reads as the 50% step after float arithmetic.
        let eps = 1e-3;
        if direction > 0 {
            STEPS
                .iter()
                .copied()
                .find(|step| *step > self.zoom + eps)
                .unwrap_or(self.zoom)
        } else {
            STEPS
                .iter()
                .rev()
                .copied()
                .find(|step| *step < self.zoom - eps)
                .unwrap_or(self.zoom)
        }
    }

    /// The zoom factor for a wheel movement of `lines` (positive away from the user).
    pub fn wheel_factor(lines: f32) -> f32 {
        ZOOM_PER_NOTCH.powf(lines / NOTCH_LINES)
    }

    pub fn panned(self, delta: Point<f32>) -> Viewport {
        Viewport {
            origin: self.origin + delta,
            ..self
        }
    }

    /// Keeps the picture on the canvas: a picture smaller than the canvas on an axis stays
    /// wholly inside it, and a larger one keeps covering it with no gap at either edge.
    pub fn clamped(self, image: Size<f32>, canvas: Size<f32>) -> Viewport {
        let clamp = |origin: f32, scaled: f32, canvas: f32| {
            if scaled <= canvas {
                origin.clamp(0., canvas - scaled)
            } else {
                origin.clamp(canvas - scaled, 0.)
            }
        };
        Viewport {
            origin: point(
                clamp(self.origin.x, image.width * self.zoom, canvas.width),
                clamp(self.origin.y, image.height * self.zoom, canvas.height),
            ),
            ..self
        }
    }

    pub fn image_bounds(&self, image: Size<f32>) -> Bounds<f32> {
        Bounds::new(
            self.origin,
            size(image.width * self.zoom, image.height * self.zoom),
        )
    }

    /// "53".
    pub fn percent(&self) -> u32 {
        (self.zoom * 100.).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: Size<f32> = Size {
        width: 1440.,
        height: 900.,
    };

    fn area(x: f32, y: f32, w: f32, h: f32) -> Bounds<f32> {
        Bounds::new(point(x, y), size(w, h))
    }

    #[test]
    fn fit_fills_the_tighter_axis_and_centres() {
        let v = Viewport::fit(IMAGE, area(0., 0., 720., 900.));
        assert_eq!(v.zoom, 0.5);
        assert_eq!(v.origin, point(0., 225.));
        assert_eq!(v.percent(), 50);
    }

    #[test]
    fn fit_never_enlarges_a_small_picture() {
        let v = Viewport::fit(size(400., 300.), area(10., 20., 1000., 800.));
        assert_eq!(v.zoom, 1.);
        assert_eq!(v.origin, point(310., 270.));
    }

    #[test]
    fn zooming_keeps_the_anchored_picture_point_still() {
        let v = Viewport {
            zoom: 0.5,
            origin: point(100., 50.),
        };
        // The picture point under (400, 250) is (600, 400).
        let zoomed = v.zoom_to(1., point(400., 250.));
        assert_eq!(zoomed.zoom, 1.);
        assert_eq!(zoomed.origin, point(400. - 600., 250. - 400.));
        let same = zoomed.image_bounds(IMAGE);
        assert_eq!(same.origin.x + 600. * zoomed.zoom, 400.);
    }

    #[test]
    fn zoom_stays_in_range() {
        let v = Viewport {
            zoom: 1.,
            origin: point(0., 0.),
        };
        assert_eq!(v.zoom_to(0.01, point(0., 0.)).zoom, MIN_ZOOM);
        assert_eq!(v.zoom_to(50., point(0., 0.)).zoom, MAX_ZOOM);
    }

    #[test]
    fn steps_move_to_the_neighbouring_level_and_stop_at_the_ends() {
        let at = |zoom| Viewport {
            zoom,
            origin: point(0., 0.),
        };
        assert_eq!(at(0.5).step(1), 0.67);
        assert_eq!(at(0.5).step(-1), 0.33);
        // Between levels, a step lands on the next level either way.
        assert_eq!(at(0.53).step(1), 0.67);
        assert_eq!(at(0.53).step(-1), 0.5);
        assert_eq!(at(4.).step(1), 4.);
        assert_eq!(at(0.1).step(-1), 0.1);
        // Above the top step, stepping down returns to it.
        assert_eq!(at(6.).step(-1), 4.);
    }

    #[test]
    fn a_wheel_notch_zooms_by_a_fifth() {
        assert!((Viewport::wheel_factor(3.) - 1.2).abs() < 1e-5);
        assert!((Viewport::wheel_factor(-3.) - 1. / 1.2).abs() < 1e-5);
        assert_eq!(Viewport::wheel_factor(0.), 1.);
    }

    #[test]
    fn clamping_keeps_a_small_picture_inside_and_a_large_one_covering() {
        let canvas = size(1000., 600.);
        let small = Viewport {
            zoom: 0.5,
            origin: point(-50., 500.),
        };
        // 720×450 at 0.5: x can't go below 0, y can't exceed 600 − 450.
        assert_eq!(small.clamped(IMAGE, canvas).origin, point(0., 150.));
        let large = Viewport {
            zoom: 1.,
            origin: point(10., -400.),
        };
        // 1440×900: x can't exceed 0, y can't go below 600 − 900.
        assert_eq!(large.clamped(IMAGE, canvas).origin, point(0., -300.));
        let fine = Viewport {
            zoom: 1.,
            origin: point(-200., -100.),
        };
        assert_eq!(fine.clamped(IMAGE, canvas), fine);
    }

    #[test]
    fn panning_moves_the_origin() {
        let v = Viewport {
            zoom: 1.,
            origin: point(10., 10.),
        };
        assert_eq!(v.panned(point(5., -3.)).origin, point(15., 7.));
    }
}
