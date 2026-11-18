//! Typed translation of `design/tokens.json`, which stays the reference.

mod colors;
pub mod contrast;
pub mod css;
pub mod custom;
mod fonts;
mod gpui_kit_theme;
mod japandi;
mod neo_brutalism;
mod pixel_art;
mod shape;
mod strings;
#[cfg(test)]
mod test_support;
mod typography;

use std::path::PathBuf;

use gpui::{App, Global, Hsla};
use toru_core::settings::{BUILT_IN_PALETTES, Look, LookAppearance, PaletteChoice, Settings};

pub use colors::*;
pub use custom::CustomPalette;
pub use fonts::{FONTS, bundled_font, install_fonts};
pub use shape::*;
pub use strings::*;
pub use typography::*;

pub const CUSTOM_PALETTE: usize = BUILT_IN_PALETTES;

pub struct Theme {
    pub look: Look,
    /// Index into `Theme::palettes(look)`, or `CUSTOM_PALETTE`.
    pub palette: usize,
    /// The custom palette's photo, drawn behind the main window under a scrim.
    pub background: Option<PathBuf>,
    pub colors: Colors,
    pub borders: Borders,
    pub radii: Radii,
    pub shadows: Shadows,
    pub icons: Icons,
    pub sizes: Sizes,
    pub fonts: Fonts,
    pub type_scale: TypeScale,
    pub strings: Strings,
    pub overlay: Overlay,
    pub empty: EmptyState,
}

impl Global for Theme {}

impl Theme {
    pub fn from_settings(settings: &Settings) -> Theme {
        let look = settings.look;
        let appearance = settings.appearance(look);
        Theme::for_choice(look, appearance.palette, appearance)
    }

    pub fn for_choice(look: Look, choice: PaletteChoice, appearance: &LookAppearance) -> Theme {
        match choice {
            PaletteChoice::BuiltIn(index) => Theme::new(look, index),
            PaletteChoice::Custom => {
                let mut theme = Theme::custom(look, &custom_palette(look, appearance));
                theme.background = appearance.custom_background.clone();
                theme
            }
        }
    }

    pub fn new(look: Look, palette: usize) -> Theme {
        match look {
            Look::Japandi => japandi::theme(palette),
            Look::PixelArt => pixel_art::theme(palette),
            Look::NeoBrutalism => neo_brutalism::theme(palette),
        }
    }

    /// Without the background photo.
    pub fn custom(look: Look, palette: &CustomPalette) -> Theme {
        match look {
            Look::Japandi => japandi::custom(palette),
            Look::PixelArt => pixel_art::custom(palette),
            Look::NeoBrutalism => neo_brutalism::custom(palette),
        }
    }

    pub fn is_custom(&self) -> bool {
        self.palette == CUSTOM_PALETTE
    }

    /// For muted text straight on the window. Over a background photo, even under the scrim,
    /// `muted` can fall below DESIGN.md §7's 4.5:1, so the text colour stands in.
    pub fn window_muted(&self) -> Hsla {
        if self.background.is_some() {
            self.colors.text
        } else {
            self.colors.muted
        }
    }

    pub fn palettes(look: Look) -> [Palette; BUILT_IN_PALETTES] {
        match look {
            Look::Japandi => japandi::palettes(),
            Look::PixelArt => pixel_art::palettes(),
            Look::NeoBrutalism => neo_brutalism::palettes(),
        }
    }
}

/// Anything missing or malformed takes the look's first palette.
pub fn custom_palette(look: Look, appearance: &LookAppearance) -> CustomPalette {
    let fallback = CustomPalette::from_theme(&Theme::new(look, 0));
    CustomPalette::parse(&appearance.custom_palette, fallback)
}

pub fn set_theme(theme: Theme, cx: &mut App) {
    gpui_kit_theme::sync_kit_theme(&theme, cx);
    cx.set_global(theme);
    cx.refresh_windows();
}

pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

/// The capture overlay (DESIGN.md §6) has no mockup, so each look derives these from its palette.
/// Its readout panel borrows the toast's colours, the one surface every look sets to stand out.
pub struct Overlay {
    pub dim: Hsla,
    pub edge: Border,
    /// A one-pixel contrasting line outside `edge`, so the edge shows on any content.
    pub edge_halo: Hsla,
    pub loupe: Border,
    pub crosshair: Hsla,
}

/// The empty gallery (`main_window::empty`), which the mockups don't show.
pub struct EmptyState {
    pub title: &'static str,
    pub ornament: Ornament,
    /// Inside the look's loading-row frame (`borders.load` on `load_bg`), else straight on the
    /// window.
    pub framed: bool,
    /// Stacked and centred, else the ornament left of left-aligned text.
    pub centered: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ornament {
    /// The Region icon in a round hairline seal on the card surface.
    Seal,
    /// The Region icon in the accent colour on a tile with the chip's border and the accent
    /// button's block shadow.
    Tile,
    /// The Region icon on a block of the thumbnail colour, bordered like a thumbnail.
    Block,
}
