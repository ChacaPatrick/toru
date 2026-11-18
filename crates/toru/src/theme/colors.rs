use gpui::Hsla;

use super::{Fill, contrast, css};

/// A hovered control also shows its text at full strength, so the hover reads stronger, never
/// weaker; `contrast`'s test checks the hovered pairs in every palette.
pub const HOVER_TINT: f32 = 0.12;

pub fn hover_tint(text: Hsla) -> Hsla {
    Hsla {
        a: HOVER_TINT,
        ..text
    }
}

pub const SELECTION_TINT: f32 = 0.3;

pub fn selection_tint(accent: Hsla) -> Hsla {
    Hsla {
        a: SELECTION_TINT,
        ..accent
    }
}

/// Enough to say the window is waiting, while what is behind stays readable.
pub const SCRIM: f32 = 0.4;

pub fn scrim(text: Hsla) -> Hsla {
    Hsla { a: SCRIM, ..text }
}

/// Under this scrim a background photo still pulls the window towards black or white, so text
/// straight on the window is checked over both (`contrast::pairs`).
pub const BACKDROP_SCRIM: f32 = 0.8;

pub fn backdrop_scrim(bg: Hsla) -> Hsla {
    Hsla {
        a: BACKDROP_SCRIM,
        ..bg
    }
}

pub struct Palette {
    pub name: &'static str,
    pub swatches: [Hsla; 4],
}

pub struct Colors {
    pub bg: Hsla,
    pub surface: Hsla,
    pub surface_2: Hsla,
    pub surface_sel: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    /// Settings group and editor panel labels; `muted` in the soft looks, ink in the hard ones.
    pub label: Hsla,
    pub accent: Hsla,
    pub accent_bg: Hsla,
    pub accent_text: Hsla,
    pub hair: Hsla,
    pub button_bg: Hsla,
    pub button_text: Hsla,
    pub thumb: Fill,
    /// Fills the second, third… cards cycle through after `thumb`; empty when every card matches.
    pub thumb_alt: Vec<Fill>,
    /// Bottom of a gradient laid over each thumbnail, from transparent at the top.
    pub thumb_shade: Hsla,
    /// `surface` unless the look dims ended day groups.
    pub earlier_card_bg: Hsla,
    pub tag_bg: Hsla,
    pub tag_text: Hsla,
    pub input_bg: Hsla,
    pub chip_bg: Hsla,
    pub chip_text: Hsla,
    pub toggle_off: Hsla,
    pub toggle_knob: Hsla,
    pub toggle_on: Hsla,
    pub toggle_on_knob: Hsla,
    /// A chosen segment button or editor tool.
    pub on_bg: Hsla,
    pub on_text: Hsla,
    pub chip_saved_bg: Hsla,
    pub chip_saved_text: Hsla,
    pub title_bg: Hsla,
    pub title_text: Hsla,
    pub dot: Hsla,
    pub controls: Hsla,
    pub toast_bg: Hsla,
    pub toast_text: Hsla,
    pub canvas: Fill,
    pub shot_bg: Hsla,
    pub shot_line: Hsla,
    pub shot_fill: Hsla,
    pub bulk_bg: Hsla,
    pub bulk_text: Hsla,
    pub bulk_sep: Hsla,
    pub bulk_main_bg: Hsla,
    pub bulk_main_text: Hsla,
    pub focus: Hsla,
    /// The focus ring on the bulk bar, whose dark or coloured `bulk_bg` can swallow `focus`.
    pub bulk_focus: Hsla,
    pub focus_halo: Hsla,
    pub sticker: Hsla,
    pub sticker_line: Hsla,
    pub anno: Hsla,
    /// The border of a field whose text isn't valid. The mockups show none; each look takes a
    /// red that stands out on its fields.
    pub warning: Hsla,
    /// Outline around the chosen colour swatch in the editor panel.
    pub swatch_on: Hsla,
    pub load_bg: Hsla,
}

/// White and yellow discs fall below it, the mockups' darker swatches stay above.
const WHITE_DIGITS_MIN_CONTRAST: f32 = 3.;

impl Colors {
    /// In the panel's order. The mockups' CSS writes the yellow and blue out for every look
    /// rather than naming tokens.
    pub fn stroke_swatches(&self) -> [Swatch; 5] {
        [
            Swatch::new(self.text),
            Swatch::new(gpui::white()),
            Swatch::new(self.anno),
            Swatch::new(css::hex(0xf1c40f)),
            Swatch::new(css::hex(0x2e86de)),
        ]
    }

    /// The mockups write `#fff` digits for every look, which a light disc swallows, so there the
    /// look's ink or page takes over, whichever contrasts more.
    pub fn badge_text(&self, disc: Hsla) -> Hsla {
        let white = gpui::white();
        if contrast::ratio(white, disc) >= WHITE_DIGITS_MIN_CONTRAST {
            white
        } else if contrast::ratio(self.text, disc) >= contrast::ratio(self.bg, disc) {
            self.text
        } else {
            self.bg
        }
    }

    /// The on-states as the base CSS draws them. A look whose `extra_css` pins them to other
    /// colours sets those after this.
    pub fn follow_accent(&mut self) {
        self.toggle_on = self.accent_bg;
        self.toggle_on_knob = self.accent_text;
        self.on_bg = self.accent_bg;
        self.on_text = self.accent_text;
        self.chip_saved_bg = self.accent_bg;
        self.chip_saved_text = self.accent_text;
        self.bulk_main_bg = self.accent_bg;
        self.bulk_main_text = self.accent_text;
        self.swatch_on = self.accent;
    }
}

/// `name` is what assistive technology reads, so it describes the colour, which a custom palette
/// can change.
#[derive(Clone, Copy)]
pub struct Swatch {
    pub name: &'static str,
    pub color: Hsla,
}

impl Swatch {
    fn new(color: Hsla) -> Swatch {
        Swatch {
            name: color_name(color),
            color,
        }
    }
}

/// A dark colour keeps a name of its own (maroon, olive, navy) unless it is nearly black or has
/// little colour in it.
fn color_name(color: Hsla) -> &'static str {
    let (hue, saturation, lightness) = (color.h * 360., color.s, color.l);
    let dark = lightness < 0.3;
    if lightness < 0.1 || (lightness < 0.2 && saturation < 0.25) {
        "Black"
    } else if lightness > 0.98 {
        "White"
    } else if lightness > 0.88 {
        "Off-white"
    } else if saturation < 0.12 {
        "Gray"
    } else {
        match hue {
            h if !(15. ..330.).contains(&h) && dark => "Maroon",
            h if !(15. ..345.).contains(&h) && lightness > 0.75 => "Pink",
            h if !(15. ..345.).contains(&h) => "Red",
            h if h < 45. && lightness < 0.5 => "Brown",
            h if h < 45. && (saturation < 0.5 || lightness > 0.8) => "Beige",
            h if h < 45. => "Orange",
            h if h < 70. && lightness < 0.4 => "Olive",
            h if h < 70. => "Yellow",
            h if h < 170. && dark => "Dark green",
            h if h < 170. => "Green",
            h if h < 200. && lightness < 0.45 => "Teal",
            h if h < 200. => "Cyan",
            h if h < 255. && dark => "Navy",
            h if h < 255. => "Blue",
            h if h < 290. => "Purple",
            h if h < 330. && lightness < 0.4 => "Purple",
            _ => "Pink",
        }
    }
}

#[cfg(test)]
mod tests {
    use toru_core::settings::{BUILT_IN_PALETTES, Look};

    use super::super::Theme;
    use super::*;
    use crate::theme::{contrast, css};

    #[test]
    fn a_light_badge_takes_the_looks_ink_for_its_digits() {
        for look in [Look::Japandi, Look::PixelArt, Look::NeoBrutalism] {
            for palette in 0..BUILT_IN_PALETTES {
                let colors = Theme::new(look, palette).colors;
                let swatches = colors.stroke_swatches();
                // White and yellow, the swatches the white digits vanished on.
                for disc in [swatches[1].color, swatches[3].color] {
                    let digits = colors.badge_text(disc);
                    assert_ne!(digits, gpui::white());
                    assert!(contrast::ratio(digits, disc) >= contrast::TEXT);
                }
                let blue = swatches[4].color;
                assert_eq!(colors.badge_text(blue), gpui::white());
            }
        }
    }

    fn names(look: Look, palette: usize) -> Vec<&'static str> {
        Theme::new(look, palette)
            .colors
            .stroke_swatches()
            .iter()
            .map(|swatch| swatch.name)
            .collect()
    }

    #[test]
    fn swatches_are_named_for_what_they_show_in_the_look() {
        assert_eq!(
            names(Look::Japandi, 0),
            ["Black", "White", "Brown", "Yellow", "Blue"]
        );
        assert_eq!(names(Look::PixelArt, 0)[2], "Pink");
        assert_eq!(names(Look::NeoBrutalism, 0)[2], "Black");
    }

    #[test]
    fn colours_get_their_everyday_names() {
        let name = |rgb| color_name(css::hex(rgb));
        assert_eq!(name(0x7a5c3e), "Brown");
        assert_eq!(name(0xff6b97), "Pink");
        assert_eq!(name(0xe74c3c), "Red");
        assert_eq!(name(0xe67e22), "Orange");
        assert_eq!(name(0x27ae60), "Green");
        assert_eq!(name(0x808080), "Gray");
        assert_eq!(name(0xf1f1e6), "Off-white");
    }

    #[test]
    fn dark_and_pale_colours_are_not_named_for_their_bright_hue() {
        let name = |rgb| color_name(css::hex(rgb));
        assert_eq!(name(0x6b5b00), "Olive");
        assert_eq!(name(0x800080), "Purple");
        assert_eq!(name(0x4a0000), "Maroon");
        assert_eq!(name(0x0a1a4a), "Navy");
        assert_eq!(name(0xd2b48c), "Beige");
        assert_eq!(name(0x00ffff), "Cyan");
        assert_eq!(name(0x008080), "Teal");
        assert_eq!(name(0x0b3d0b), "Dark green");
        assert_eq!(name(0x2b2a26), "Black");
        assert_eq!(name(0x0d0d18), "Black");
        assert_eq!(name(0xf1c40f), "Yellow");
        assert_eq!(name(0x2e86de), "Blue");
        assert_eq!(name(0xb388ff), "Purple");
        assert_eq!(name(0x9370db), "Purple");
        assert_eq!(name(0x4b0082), "Purple");
        assert_eq!(name(0xff1493), "Pink");
        assert_eq!(name(0xc71585), "Pink");
        assert_eq!(name(0xffe6b3), "Beige");
    }
}
