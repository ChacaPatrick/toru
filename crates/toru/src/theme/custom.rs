//! The custom fourth palette of a look: six colours of the user's own, stored as text in
//! `LookAppearance::custom_palette`, from which everything else a palette sets is derived.
//!
//! The stored form is `bg=#efe9df surface=#faf7f1 text=#2b2a26 …`, one `key=#rrggbb` per
//! colour, so it reads in the database and survives a hand edit. A key that is missing or
//! doesn't parse keeps the fallback's colour, so bad data can never stop a look from loading.

use gpui::{Hsla, Rgba};

use super::{Colors, Fill, Theme, contrast::over};

/// The colours the user sets, in the order the editor shows them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomPalette {
    pub bg: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
}

/// What each colour is called in the stored text and in the editor.
pub const FIELDS: [(&str, &str); 6] = [
    ("bg", "Background"),
    ("surface", "Surface"),
    ("text", "Text"),
    ("muted", "Muted text"),
    ("accent", "Accent"),
    ("accent_text", "Accent text"),
];

/// How far a derived surface moves from one anchor colour towards another.
const SURFACE_2_MIX: f32 = 0.5;
const HAIR_MIX: f32 = 0.18;
const LINE_MIX: f32 = 0.3;
const SHADE: f32 = 0.08;
const HALO: f32 = 0.25;
const TAG: f32 = 0.9;

impl CustomPalette {
    /// The six colours a theme shows, as the starting point for editing it.
    pub fn from_theme(theme: &Theme) -> CustomPalette {
        let c = &theme.colors;
        CustomPalette {
            bg: c.bg,
            surface: c.surface,
            text: c.text,
            muted: c.muted,
            // The fill of the primary button, which is the accent in every look; NeoBrutalism's
            // `accent` is its black outline.
            accent: c.accent_bg,
            accent_text: c.accent_text,
        }
    }

    /// Reads the stored text, taking any colour that is missing or malformed from `fallback`.
    pub fn parse(text: &str, fallback: CustomPalette) -> CustomPalette {
        let mut palette = fallback;
        for entry in text.split_whitespace() {
            let Some((key, value)) = entry.split_once('=') else {
                continue;
            };
            let Some(color) = parse_hex(value) else {
                continue;
            };
            if let Some(slot) = palette.slot(key) {
                *slot = color;
            }
        }
        palette
    }

    pub fn to_stored(self) -> String {
        FIELDS
            .iter()
            .map(|(key, _)| format!("{key}={}", hex_text(self.get(key))))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The colour stored under `key`, which must be one of `FIELDS`.
    pub fn get(&self, key: &str) -> Hsla {
        match key {
            "bg" => self.bg,
            "surface" => self.surface,
            "text" => self.text,
            "muted" => self.muted,
            "accent" => self.accent,
            _ => self.accent_text,
        }
    }

    pub fn set(&mut self, key: &str, color: Hsla) {
        if let Some(slot) = self.slot(key) {
            *slot = color;
        }
    }

    fn slot(&mut self, key: &str) -> Option<&mut Hsla> {
        Some(match key {
            "bg" => &mut self.bg,
            "surface" => &mut self.surface,
            "text" => &mut self.text,
            "muted" => &mut self.muted,
            "accent" => &mut self.accent,
            "accent_text" => &mut self.accent_text,
            _ => return None,
        })
    }

    /// The four swatches the picker shows, in the built-in palettes' order: background, surface,
    /// accent, text.
    pub fn swatches(&self) -> [Hsla; 4] {
        [self.bg, self.surface, self.accent, self.text]
    }

    /// Writes the six colours into `c` and derives every other colour a palette sets from
    /// them, the way the built-in palettes' overrides do: the second surface, which the title
    /// bar shares, sits between the background and the surface, a selected card sinks to the
    /// background (text must read on both, so it reads there), hairlines are a faint tint of the text over the
    /// background, the toast and the on-states take the accent, and the bulk bar is the text
    /// colour inverted. Each look applies its own `follow_palette` after this.
    pub fn apply(&self, c: &mut Colors) {
        c.bg = self.bg;
        c.surface = self.surface;
        c.text = self.text;
        c.muted = self.muted;
        c.accent = self.accent;
        c.accent_bg = self.accent;
        c.accent_text = self.accent_text;
        c.surface_2 = mix(self.bg, self.surface, SURFACE_2_MIX);
        c.surface_sel = self.bg;
        c.input_bg = self.surface;
        c.button_bg = self.surface;
        c.button_text = self.text;
        c.hair = self.hair();
        c.label = self.muted;
        c.title_bg = c.surface_2;
        c.title_text = self.text;
        c.controls = self.muted;
        c.dot = self.accent;
        c.toast_bg = self.accent;
        c.toast_text = self.accent_text;
        c.chip_bg = c.surface_2;
        c.chip_text = self.muted;
        c.tag_bg = Hsla {
            a: TAG,
            ..self.surface
        };
        c.tag_text = self.muted;
        c.thumb = Fill::Solid(c.surface_2);
        c.thumb_alt = Vec::new();
        c.thumb_shade = Hsla {
            a: SHADE,
            ..self.text
        };
        c.earlier_card_bg = self.surface;
        c.toggle_off = c.surface_2;
        c.toggle_knob = self.muted;
        c.canvas = Fill::Solid(c.surface_2);
        c.shot_bg = self.surface;
        c.shot_line = c.hair;
        c.shot_fill = c.surface_2;
        c.bulk_bg = self.text;
        c.bulk_text = self.bg;
        c.bulk_sep = self.muted;
        c.focus = self.text;
        c.bulk_focus = self.bg;
        c.focus_halo = Hsla {
            a: HALO,
            ..self.accent
        };
        c.load_bg = gpui::transparent_black();
        c.follow_accent();
    }

    /// The hairline colour: a faint tint of the text over the background.
    pub fn hair(&self) -> Hsla {
        mix(self.bg, self.text, HAIR_MIX)
    }

    /// The edge of a control, a little stronger than a hairline.
    pub fn line(&self) -> Hsla {
        mix(self.bg, self.text, LINE_MIX)
    }
}

/// `from` moved `amount` of the way to `to`, in sRGB as the screen blends.
pub(super) fn mix(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    over(Hsla { a: amount, ..to }, from)
}

/// `#rrggbb` or `rrggbb`, in either case, as an opaque colour.
pub fn parse_hex(text: &str) -> Option<Hsla> {
    let digits = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some(gpui::rgb(value).into())
}

/// A colour as `#rrggbb`, its alpha dropped.
pub fn hex_text(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{CUSTOM_PALETTE, contrast, css::hex};
    use toru_core::settings::{BUILT_IN_PALETTES, Look};

    fn linen() -> CustomPalette {
        CustomPalette::from_theme(&Theme::new(Look::Japandi, 0))
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(hex_text(parse_hex("#2B2A26").unwrap()), "#2b2a26");
        assert_eq!(hex_text(parse_hex(" faf7f1 ").unwrap()), "#faf7f1");
        assert_eq!(hex_text(hex(0x000000)), "#000000");
        assert_eq!(hex_text(hex(0xffffff)), "#ffffff");
        for bad in ["", "#", "#fff", "#12345g", "#1234567", "red", "# 2b2a26"] {
            assert!(parse_hex(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn stored_text_round_trips() {
        let palette = CustomPalette {
            bg: hex(0x112233),
            surface: hex(0x445566),
            text: hex(0x778899),
            muted: hex(0xaabbcc),
            accent: hex(0xddeeff),
            accent_text: hex(0x010203),
        };
        let stored = palette.to_stored();
        assert_eq!(
            stored,
            "bg=#112233 surface=#445566 text=#778899 muted=#aabbcc accent=#ddeeff \
             accent_text=#010203"
        );
        assert_eq!(CustomPalette::parse(&stored, linen()), palette);
    }

    #[test]
    fn bad_stored_data_keeps_the_fallback() {
        let fallback = linen();
        assert_eq!(CustomPalette::parse("", fallback), fallback);
        assert_eq!(CustomPalette::parse("garbage ==== bg=", fallback), fallback);
        // A good key among bad ones is taken; unknown keys and bad colours are passed over.
        let mixed = CustomPalette::parse("bg=#000000 text=notacolour nope=#ffffff", fallback);
        assert_eq!(hex_text(mixed.bg), "#000000");
        assert_eq!(mixed.text, fallback.text);
        assert_eq!(mixed.surface, fallback.surface);
    }

    #[test]
    fn swatches_match_the_picker_order() {
        let theme = Theme::new(Look::Japandi, 0);
        let palette = CustomPalette::from_theme(&theme);
        let built_in = Theme::palettes(Look::Japandi)[0].swatches;
        assert_eq!(hex_text(palette.swatches()[0]), hex_text(built_in[0]));
        assert_eq!(hex_text(palette.swatches()[2]), hex_text(built_in[2]));
    }

    #[test]
    fn custom_theme_takes_the_six_colours() {
        let palette = CustomPalette {
            bg: hex(0x101010),
            surface: hex(0x202020),
            text: hex(0xf0f0f0),
            muted: hex(0xb0b0b0),
            accent: hex(0xffcc00),
            accent_text: hex(0x101010),
        };
        for look in [Look::Japandi, Look::PixelArt, Look::NeoBrutalism] {
            let theme = Theme::custom(look, &palette);
            assert_eq!(theme.look, look);
            assert_eq!(theme.palette, CUSTOM_PALETTE);
            assert_eq!(CustomPalette::from_theme(&theme), palette);
            let c = &theme.colors;
            assert_eq!(hex_text(c.on_bg), "#ffcc00");
            assert_eq!(hex_text(c.toast_bg), "#ffcc00");
            assert_eq!(hex_text(c.bulk_bg), "#f0f0f0");
        }
    }

    /// Every built-in palette, entered as a custom one, still meets DESIGN.md §7, so the
    /// derived colours are sound whenever the six entered ones are; and with a background photo
    /// behind the window too, whatever the picture.
    #[test]
    fn built_in_colours_entered_as_custom_meet_design_md_7() {
        let mut failures = Vec::new();
        for look in [Look::Japandi, Look::PixelArt, Look::NeoBrutalism] {
            for index in 0..BUILT_IN_PALETTES {
                let palette = CustomPalette::from_theme(&Theme::new(look, index));
                let mut theme = Theme::custom(look, &palette);
                for failure in contrast::failures(&theme) {
                    failures.push(format!("{look:?} / {index}: {failure}"));
                }
                theme.background = Some("photo.jpg".into());
                for failure in contrast::failures(&theme) {
                    failures.push(format!("{look:?} / {index} + photo: {failure}"));
                }
            }
        }
        assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
    }

    #[test]
    fn a_bad_palette_is_named_in_the_warnings() {
        let mut palette = linen();
        palette.muted = palette.surface;
        let theme = Theme::custom(Look::Japandi, &palette);
        let failures = contrast::failures(&theme);
        assert!(
            failures.iter().any(|f| f.name == "muted on surface"),
            "{failures:?}"
        );
        assert!(failures.iter().all(|f| f.ratio < f.min));
    }
}
