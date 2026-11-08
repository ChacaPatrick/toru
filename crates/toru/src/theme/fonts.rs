//! The bundled fonts, one set per look. Most are static instances subset to the scripts the app
//! sets in them (see `assets/fonts/README.md`). GPUI sets the interface in them, and the editor
//! draws text annotations from their outlines, so the picture shows the same letters saved as
//! on screen.

use std::borrow::Cow;

use gpui::{App, FontWeight};

/// Japandi's name fields use the system monospace face, as the mockups' `ui-monospace` does. On
/// Windows that is Consolas; other platforms pick their own when they arrive.
pub const MONO: &str = "Consolas";

/// A bundled face: its family as GPUI knows it, its weight, and the file.
pub struct BundledFont {
    pub family: &'static str,
    pub weight: FontWeight,
    pub data: &'static [u8],
}

const fn bundled(family: &'static str, weight: FontWeight, data: &'static [u8]) -> BundledFont {
    BundledFont {
        family,
        weight,
        data,
    }
}

pub const FONTS: [BundledFont; 10] = [
    bundled(
        "Noto Serif JP",
        FontWeight::NORMAL,
        include_bytes!("../../../../assets/fonts/NotoSerifJP-Regular.ttf"),
    ),
    bundled(
        "Noto Serif JP",
        FontWeight::SEMIBOLD,
        include_bytes!("../../../../assets/fonts/NotoSerifJP-SemiBold.ttf"),
    ),
    bundled(
        "Inter",
        FontWeight::NORMAL,
        include_bytes!("../../../../assets/fonts/Inter-Regular.ttf"),
    ),
    bundled(
        "Inter",
        FontWeight::MEDIUM,
        include_bytes!("../../../../assets/fonts/Inter-Medium.ttf"),
    ),
    bundled(
        "Inter",
        FontWeight::SEMIBOLD,
        include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf"),
    ),
    bundled(
        "Press Start 2P",
        FontWeight::NORMAL,
        include_bytes!("../../../../assets/fonts/PressStart2P-Regular.ttf"),
    ),
    bundled(
        "VT323",
        FontWeight::NORMAL,
        include_bytes!("../../../../assets/fonts/VT323-Regular.ttf"),
    ),
    bundled(
        "Archivo Black",
        FontWeight::NORMAL,
        include_bytes!("../../../../assets/fonts/ArchivoBlack-Regular.ttf"),
    ),
    bundled(
        "Space Grotesk",
        FontWeight::MEDIUM,
        include_bytes!("../../../../assets/fonts/SpaceGrotesk-Medium.ttf"),
    ),
    bundled(
        "Space Grotesk",
        FontWeight::BOLD,
        include_bytes!("../../../../assets/fonts/SpaceGrotesk-Bold.ttf"),
    ),
];

/// Registers the bundled fonts with GPUI. Call once, before the first window opens.
pub fn install_fonts(cx: &App) {
    let fonts = FONTS.iter().map(|font| Cow::Borrowed(font.data)).collect();
    cx.text_system()
        .add_fonts(fonts)
        .expect("the bundled fonts are valid TrueType files");
}

/// The bundled file of `family` nearest to `weight`, as GPUI itself falls back to the nearest
/// weight it has. `None` for a family that isn't bundled, such as the system monospace.
pub fn bundled_font(family: &str, weight: FontWeight) -> Option<&'static BundledFont> {
    FONTS
        .iter()
        .filter(|font| font.family == family)
        .min_by(|a, b| {
            let distance = |font: &BundledFont| (font.weight.0 - weight.0).abs();
            distance(a).total_cmp(&distance(b))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_weight_of_a_family_is_found() {
        let bold = bundled_font("Inter", FontWeight::BOLD).unwrap();
        assert_eq!(bold.weight, FontWeight::SEMIBOLD);
        let thin = bundled_font("Space Grotesk", FontWeight::THIN).unwrap();
        assert_eq!(thin.weight, FontWeight::MEDIUM);
        assert_eq!(
            bundled_font("Noto Serif JP", FontWeight::NORMAL)
                .unwrap()
                .weight,
            FontWeight::NORMAL
        );
        assert!(bundled_font(MONO, FontWeight::NORMAL).is_none());
    }
}
