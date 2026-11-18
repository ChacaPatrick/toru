//! The look of the app as typed values, translated from `design/tokens.json`.
//!
//! `design/tokens.json` stays the reference. A look is a full `Theme`; a palette overrides some of
//! its values, exactly as the JSON does. Components read everything from the active theme and never
//! hard-code a colour or size.

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

/// `Theme::palette` when the custom palette is shown: one past the built-in ones.
pub const CUSTOM_PALETTE: usize = BUILT_IN_PALETTES;

pub struct Theme {
    pub look: Look,
    /// Index into `Theme::palettes(look)`, or `CUSTOM_PALETTE`.
    pub palette: usize,
    /// The custom palette's background photo, drawn behind the main window under a scrim.
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
    /// The theme for the look and palette chosen in `settings`.
    pub fn from_settings(settings: &Settings) -> Theme {
        let look = settings.look;
        let appearance = settings.appearance(look);
        Theme::for_choice(look, appearance.palette, appearance)
    }

    /// The theme for `choice` in `look`; the custom palette reads its colours and photo from
    /// `appearance`.
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

    /// `look` in the user's own colours, without a background photo.
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

    /// Muted text drawn straight on the window rather than on a surface: `muted`, or the text
    /// colour while a background photo is behind it, since under the scrim a dark or light
    /// picture can take `muted` below DESIGN.md §7's 4.5:1.
    pub fn window_muted(&self) -> Hsla {
        if self.background.is_some() {
            self.colors.text
        } else {
            self.colors.muted
        }
    }

    /// The built-in palettes of a look, for pickers and for stepping through them.
    pub fn palettes(look: Look) -> [Palette; BUILT_IN_PALETTES] {
        match look {
            Look::Japandi => japandi::palettes(),
            Look::PixelArt => pixel_art::palettes(),
            Look::NeoBrutalism => neo_brutalism::palettes(),
        }
    }
}

/// The custom colours `appearance` stores for `look`. Anything missing or malformed takes the
/// look's first palette, so a theme can always be built from them.
pub fn custom_palette(look: Look, appearance: &LookAppearance) -> CustomPalette {
    let fallback = CustomPalette::from_theme(&Theme::new(look, 0));
    CustomPalette::parse(&appearance.custom_palette, fallback)
}

/// Makes `theme` the active one and redraws every window with it.
pub fn set_theme(theme: Theme, cx: &mut App) {
    gpui_kit_theme::sync_kit_theme(&theme, cx);
    cx.set_global(theme);
    cx.refresh_windows();
}

/// `cx.theme()` anywhere a `&App` is at hand.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

/// The capture overlay (DESIGN.md §6), which has no mockup: each look derives these from its own
/// palette. The readout and magnifier panel borrows the toast's colours, border and type, since
/// the toast is the one surface every look already sets to stand out from anything.
pub struct Overlay {
    /// Laid over the frozen screen everywhere but inside the selection.
    pub dim: Hsla,
    /// Around the selection, just outside the selected pixels.
    pub edge: Border,
    /// A one-pixel line outside `edge`, in a contrasting colour, so the edge shows on any content.
    pub edge_halo: Hsla,
    /// Around the magnifier inside the panel.
    pub loupe: Border,
    /// The lines marking the pixel under the pointer in the magnifier.
    pub crosshair: Hsla,
}

/// How the look draws the gallery while it has nothing to list (`main_window::empty`), which
/// the mockups don't show: each look says it in its own voice and builds it from parts it
/// already has, so it reads as that look and not as an error.
pub struct EmptyState {
    /// The headline, set in the look's section type.
    pub title: &'static str,
    pub ornament: Ornament,
    /// Inside the look's placeholder frame, as its loading row is (`borders.load` on
    /// `load_bg`), or straight on the window.
    pub framed: bool,
    /// Stacked and centred, or the ornament at the left of left-aligned text.
    pub centered: bool,
}

/// The small figure beside or above an empty state's words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ornament {
    /// The Region icon in a round hairline seal on the card surface.
    Seal,
    /// The Region icon in the accent colour on a surface tile with the chip's hard border and
    /// the accent button's block shadow.
    Tile,
    /// The Region icon on a block of the thumbnail colour, bordered like a thumbnail.
    Block,
}
