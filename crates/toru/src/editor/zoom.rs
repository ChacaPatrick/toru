use gpui::{
    Bounds, Context, Pixels, Point, ScrollDelta, ScrollEvent, Size, Window, point, px, size,
};
use tiny_skia::IntRect;

use super::tools::Tool;
use super::viewport::Viewport;
use super::{Editor, chrome, panel};

/// A wheel reported in pixels counts this many per line.
const PIXELS_PER_LINE: f32 = 20.;

impl Editor {
    /// All of the picture while the Crop tool is out, so the rectangle can be drawn anywhere.
    pub(super) fn shown(&self) -> IntRect {
        if self.tool == Tool::Crop {
            self.document.whole()
        } else {
            self.document.crop()
        }
    }

    pub(super) fn shown_bounds(&self) -> Bounds<f32> {
        bounds_of(self.shown())
    }

    /// The logical size at 100%: one picture pixel per device pixel, so it is sharp on screen
    /// and the same size as the capture was.
    pub(super) fn picture(&self) -> Size<f32> {
        self.shown_bounds().size.map(|v| v / self.scale)
    }

    /// Clear of the panel on both sides, so the picture stays centred.
    fn fit_area(&self) -> Bounds<f32> {
        let edge = chrome::CHROME_INSET;
        let side = panel::LEFT + panel::WIDTH + edge;
        let top = edge + chrome::PILL_HEIGHT + edge;
        let bottom = edge + chrome::CORNER_HEIGHT + edge;
        let canvas = self.canvas_size();
        Bounds::new(
            point(side, top),
            size(
                (canvas.width - side * 2.).max(1.),
                (canvas.height - top - bottom).max(1.),
            ),
        )
    }

    pub(super) fn canvas_size(&self) -> Size<f32> {
        self.canvas.size.map(f32::from)
    }

    pub(super) fn laid_out(&self) -> Viewport {
        if self.fitted {
            Viewport::fit(self.picture(), self.fit_area())
        } else {
            self.viewport.clamped(self.picture(), self.canvas_size())
        }
    }

    pub(super) fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.fitted = true;
        self.viewport = self.laid_out();
        cx.notify();
    }

    pub(super) fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let canvas = self.canvas_size();
        let centre = point(canvas.width / 2., canvas.height / 2.);
        self.zoom_at(zoom, centre, cx);
    }

    pub(super) fn zoom_step(&mut self, direction: i32, cx: &mut Context<Self>) {
        self.zoom_to(self.viewport.step(direction), cx);
    }

    fn zoom_at(&mut self, zoom: f32, anchor: Point<f32>, cx: &mut Context<Self>) {
        self.fitted = false;
        self.viewport = self
            .viewport
            .zoom_to(zoom, anchor)
            .clamped(self.picture(), self.canvas_size());
        cx.notify();
    }

    pub(super) fn wheel(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        let lines = match event.delta {
            ScrollDelta::Lines(lines) => lines.y,
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_LINE,
        };
        if lines == 0. {
            return;
        }
        let anchor = (event.position - self.canvas.origin).map(f32::from);
        let zoom = self.viewport.zoom * Viewport::wheel_factor(lines);
        self.zoom_at(zoom, anchor, cx);
    }

    /// A window position as a pixel of the whole picture.
    pub(super) fn image_point(&self, position: Point<Pixels>) -> Point<f32> {
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        let v = &self.viewport;
        let shown = self.shown_bounds().origin;
        point(
            shown.x + (on_canvas.x - v.origin.x) / v.zoom * self.scale,
            shown.y + (on_canvas.y - v.origin.y) / v.zoom * self.scale,
        )
    }

    /// A pixel of the whole picture as a logical point on the canvas.
    pub(super) fn canvas_point(&self, at: Point<f32>) -> Point<f32> {
        let v = &self.viewport;
        let shown = self.shown_bounds().origin;
        point(
            v.origin.x + (at.x - shown.x) / self.scale * v.zoom,
            v.origin.y + (at.y - shown.y) / self.scale * v.zoom,
        )
    }

    /// The chrome built for this frame read the old viewport, so the next frame is asked for; the
    /// picture itself paints at the new one.
    pub(super) fn canvas_laid_out(&mut self, bounds: Bounds<Pixels>, window: &mut Window) {
        let scale = window.scale_factor();
        if bounds.size == self.canvas.size && scale == self.scale {
            self.canvas = bounds;
            return;
        }
        let first = !self.measured();
        self.canvas = bounds;
        self.scale = scale;
        let viewport = self.laid_out();
        if first || viewport != self.viewport {
            self.viewport = viewport;
            window.request_animation_frame();
        }
    }

    /// Until a frame has measured the canvas, the viewport is provisional, so the chrome that
    /// reads it waits rather than show a zoom the picture isn't at.
    pub(super) fn measured(&self) -> bool {
        self.canvas.size.width > px(0.)
    }
}

fn bounds_of(rect: IntRect) -> Bounds<f32> {
    Bounds::new(
        point(rect.x() as f32, rect.y() as f32),
        size(rect.width() as f32, rect.height() as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::annotation::in_picture;

    #[test]
    fn drags_stay_within_the_picture() {
        let picture = Bounds::new(point(0., 0.), size(100., 50.));
        assert_eq!(in_picture(picture, point(-5., 20.)), point(0., 20.));
        assert_eq!(in_picture(picture, point(120., 70.)), point(100., 50.));
        assert_eq!(in_picture(picture, point(40., 20.)), point(40., 20.));
        // A cropped picture starts where the crop does.
        let cropped = Bounds::new(point(20., 10.), size(50., 30.));
        assert_eq!(in_picture(cropped, point(5., 5.)), point(20., 10.));
        assert_eq!(
            bounds_of(IntRect::from_xywh(20, 10, 50, 30).unwrap()),
            cropped
        );
    }
}
