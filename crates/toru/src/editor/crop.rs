use gpui::{Bounds, Context, point};
use tiny_skia::IntRect;

use super::annotation::{Annotation, Shape, Style};
use super::tools::Tool;
use super::{Editor, tools};

impl Editor {
    pub(super) fn document_crop_draft(&self) -> Option<Annotation> {
        self.document
            .is_cropped()
            .then(|| rectangle(self.document.crop()))
    }

    pub(super) fn crop_rect(&self) -> Option<IntRect> {
        whole_pixels(self.crop_draft.as_ref()?.bounds())
    }

    pub(super) fn apply_crop(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        if let Some(rect) = self.crop_rect() {
            self.document.set_crop(Some(rect));
        }
        self.set_tool(Tool::Select, cx);
    }
}

/// Its style is never drawn.
fn rectangle(rect: IntRect) -> Annotation {
    Annotation {
        shape: Shape::Rectangle,
        points: vec![
            point(rect.x() as f32, rect.y() as f32),
            point(rect.right() as f32, rect.bottom() as f32),
        ],
        style: Style {
            color: tiny_skia::Color::BLACK,
            second: tiny_skia::Color::BLACK,
            width: tools::Width::Thin,
            line: tools::LineStyle::Solid,
            opacity: 1.,
        },
    }
}

fn whole_pixels(b: Bounds<f32>) -> Option<IntRect> {
    let (left, top) = (b.left().round() as i32, b.top().round() as i32);
    IntRect::from_ltrb(
        left,
        top,
        (b.right().round() as i32).max(left + 1),
        (b.bottom().round() as i32).max(top + 1),
    )
}

#[cfg(test)]
mod tests {
    use gpui::size;

    use super::*;

    #[test]
    fn a_crop_rectangle_rounds_to_whole_pixels_and_never_vanishes() {
        let whole = IntRect::from_xywh(0, 0, 100, 50).unwrap();
        assert_eq!(
            rectangle(whole).bounds(),
            Bounds::new(point(0., 0.), size(100., 50.))
        );
        let tiny = whole_pixels(Bounds::new(point(10.4, 5.6), size(0.2, 0.1))).unwrap();
        assert_eq!(tiny, IntRect::from_xywh(10, 6, 1, 1).unwrap());
        let rounded = whole_pixels(Bounds::new(point(9.5, 0.4), size(20.2, 10.))).unwrap();
        assert_eq!(rounded, IntRect::from_ltrb(10, 0, 30, 10).unwrap());
    }
}
