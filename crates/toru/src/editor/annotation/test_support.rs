use gpui::{Bounds, Point, Size, point, size};
use tiny_skia::{Color, Pixmap};

use super::{Annotation, Badge, Shape, Style, Text};
use crate::editor::sticker::Sticker;
use crate::editor::text::Font;
use crate::editor::tools::{LineStyle, Width};

pub(super) fn style(line: LineStyle) -> Style {
    Style {
        color: Color::from_rgba8(200, 0, 0, 255),
        second: Color::WHITE,
        width: Width::Medium,
        line,
        opacity: 1.,
    }
}

pub(super) fn picture(width: f32, height: f32) -> Bounds<f32> {
    Bounds::new(point(0., 0.), size(width, height))
}

pub(super) fn mark(shape: Shape, from: (f32, f32), to: (f32, f32)) -> Annotation {
    let mut a = Annotation::new(shape, point(from.0, from.1), style(LineStyle::Solid));
    a.extend(point(to.0, to.1), false, PICTURE);
    a
}

pub(super) fn is_red(picture: &Pixmap, x: u32, y: u32) -> bool {
    let p = picture.pixel(x, y).unwrap();
    p.red() > 150 && p.green() < 100
}

pub(super) fn is_white(picture: &Pixmap, x: u32, y: u32) -> bool {
    let p = picture.pixel(x, y).unwrap();
    p.red() == 255 && p.green() == 255 && p.blue() == 255
}

pub(super) fn white(width: u32, height: u32) -> Pixmap {
    let mut picture = Pixmap::new(width, height).unwrap();
    picture.fill(Color::WHITE);
    picture
}

pub(super) fn inter() -> Font {
    Font {
        family: "Inter".into(),
        weight: gpui::FontWeight::SEMIBOLD,
    }
}

pub(super) fn text(content: &str, at: (f32, f32), size: f32) -> Annotation {
    let shape = Shape::Text(Text {
        content: content.into(),
        font: inter(),
        size,
        upper: false,
    });
    Annotation::placed(shape, point(at.0, at.1), 0., style(LineStyle::Solid))
}

pub(super) fn badge(number: u32, at: (f32, f32), diameter: f32) -> Annotation {
    let shape = Shape::Number(Badge {
        number,
        diameter,
        font: inter(),
    });
    Annotation::placed(shape, point(at.0, at.1), 0., style(LineStyle::Solid))
}

pub(super) fn sticker(kind: Sticker, at: (f32, f32), side: f32) -> Annotation {
    Annotation::placed(
        Shape::Sticker(kind),
        point(at.0, at.1),
        side,
        style(LineStyle::Solid),
    )
}

/// A picture big enough for everything the tests draw.
pub(super) const PICTURE: Bounds<f32> = Bounds {
    origin: Point { x: 0., y: 0. },
    size: Size {
        width: 10_000.,
        height: 10_000.,
    },
};
