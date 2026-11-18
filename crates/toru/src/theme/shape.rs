use gpui::{Background, BoxShadow, Hsla, Pixels, px};

#[derive(Clone)]
pub enum Fill {
    Solid(Hsla),
    Gradient { angle: f32, from: Hsla, to: Hsla },
    Checker { a: Hsla, b: Hsla, cell: Pixels },
    Grid { bg: Hsla, line: Hsla, cell: Pixels },
}

impl Fill {
    /// Patterns return their base colour; `ui::fill` paints them in full.
    pub fn background(&self) -> Background {
        match *self {
            Fill::Solid(color) => color.into(),
            Fill::Gradient { angle, from, to } => gpui::linear_gradient(
                angle,
                gpui::linear_color_stop(from, 0.),
                gpui::linear_color_stop(to, 1.),
            ),
            Fill::Checker { a, .. } => a.into(),
            Fill::Grid { bg, .. } => bg.into(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Border {
    pub width: Pixels,
    pub color: Hsla,
    pub dashed: bool,
}

impl Border {
    pub fn solid(width: f32, color: Hsla) -> Border {
        Border {
            width: px(width),
            color,
            dashed: false,
        }
    }

    pub fn dashed(width: f32, color: Hsla) -> Border {
        Border {
            width: px(width),
            color,
            dashed: true,
        }
    }

    pub fn none() -> Border {
        Border::solid(0., gpui::transparent_black())
    }
}

pub struct Borders {
    pub accent: Border,
    pub hair: Border,
    pub frame: Border,
    pub button: Border,
    pub card: Border,
    pub input: Border,
    pub chip: Border,
    pub toggle: Border,
    pub toast: Border,
    pub pop: Border,
    pub load: Border,
    pub earlier_rule: Border,
    pub thumb: Border,
    /// Between the window controls.
    pub title_divider: Border,
}

pub struct Radii {
    pub frame: Pixels,
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub pill: Pixels,
    /// The title bar dot.
    pub dot: Pixels,
}

/// Each is the full list for one `box-shadow`, so hard offsets and inset highlights fit.
pub struct Shadows {
    pub accent: Vec<BoxShadow>,
    pub button: Vec<BoxShadow>,
    pub card: Vec<BoxShadow>,
    pub pop: Vec<BoxShadow>,
    pub shot: Vec<BoxShadow>,
}

/// Only the sizes a look changes; the rest are the same in every look's CSS and set where the
/// control is drawn.
pub struct Sizes {
    /// `.btn.sm`'s horizontal padding.
    pub button_sm_padding: Pixels,
    pub tag_padding_x: Pixels,
    /// Some: each window control is a filled square of this size instead of a glyph.
    pub window_control_square: Option<Pixels>,
}

/// Line icons and the title bar dot.
pub struct Icons {
    pub stroke: Pixels,
    /// Square caps and mitred joins instead of round ones.
    pub square: bool,
    /// The dot's diameter.
    pub dot: Pixels,
}
