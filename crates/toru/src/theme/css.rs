//! Helpers that read like the CSS in `design/tokens.json`, so each look's file is a transcription
//! of its entry.

use gpui::{BoxShadow, FontWeight, Hsla, px, rgb};

use super::{Case, Family, Fill, TypeStyle};

pub fn hex(rgb_hex: u32) -> Hsla {
    rgb(rgb_hex).into()
}

/// `rgba(r,g,b,alpha)` written as the hex of its colour.
pub fn hex_alpha(rgb_hex: u32, alpha: f32) -> Hsla {
    Hsla {
        a: alpha,
        ..hex(rgb_hex)
    }
}

/// `linear-gradient({angle}deg, from, to)`.
pub fn gradient(angle: f32, from: u32, to: u32) -> Fill {
    Fill::Gradient {
        angle,
        from: hex(from),
        to: hex(to),
    }
}

/// `repeating-conic-gradient(a 0 25%, b 0 50%) 0 0/{tile}px {tile}px`: a 2×2 checker per tile.
pub fn checker(a: u32, b: u32, tile: f32) -> Fill {
    Fill::Checker {
        a: hex(a),
        b: hex(b),
        cell: px(tile / 2.),
    }
}

/// Two repeating 1px lines every `cell` over `bg`, as the NeoBrutalism canvas draws them.
pub fn grid(bg: u32, line: u32, cell: f32) -> Fill {
    Fill::Grid {
        bg: hex(bg),
        line: hex(line),
        cell: px(cell),
    }
}

/// CSS `0 {y}px {blur}px color`. GPUI's blur radius is the Gaussian sigma; CSS's blur is twice it.
pub fn soft_shadow(y: f32, blur: f32, color: Hsla) -> BoxShadow {
    BoxShadow::new(px(0.), px(y), color).blur_radius(px(blur / 2.))
}

/// CSS `{x}px {y}px 0 color`: a sharp offset copy of the box.
pub fn hard_shadow(x: f32, y: f32, color: Hsla) -> BoxShadow {
    BoxShadow::new(px(x), px(y), color)
}

/// CSS `inset {x}px {y}px 0 color`: a sharp band along the inside edges the offset points at.
pub fn inset_shadow(x: f32, y: f32, color: Hsla) -> BoxShadow {
    BoxShadow::new(px(x), px(y), color).inset()
}

pub fn body(size: f32, weight: FontWeight) -> TypeStyle {
    TypeStyle::new(Family::Body, size, weight, Case::AsWritten)
}

pub fn display(size: f32, weight: FontWeight) -> TypeStyle {
    TypeStyle::new(Family::Display, size, weight, Case::AsWritten)
}

pub fn mono(size: f32, weight: FontWeight) -> TypeStyle {
    TypeStyle::new(Family::Mono, size, weight, Case::AsWritten)
}

impl TypeStyle {
    /// The same style with `text-transform: uppercase`.
    pub fn upper(self) -> TypeStyle {
        TypeStyle {
            case: Case::Upper,
            ..self
        }
    }
}
