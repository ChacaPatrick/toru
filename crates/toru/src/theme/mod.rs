//! The look of the app as typed values, translated from `design/tokens.json`.
//!
//! `design/tokens.json` stays the reference. A look is a full `Theme`; a palette overrides some of
//! its values, exactly as the JSON does. Components read everything from the active theme and never
//! hard-code a colour or size.

pub mod contrast;
pub mod css;
pub mod custom;
mod fonts;
mod japandi;
mod kit;
mod neo_brutalism;
mod pixel_art;
#[cfg(test)]
mod test_support;

use std::path::PathBuf;

use gpui::{
    App, Background, BoxShadow, Font, FontWeight, Global, Hsla, Pixels, SharedString, Styled,
    TextRun, Window, font, px,
};
use toru_core::settings::{BUILT_IN_PALETTES, Look, LookAppearance, PaletteChoice, Settings};

pub use custom::CustomPalette;
pub use fonts::{FONTS, bundled_font, install_fonts};

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

/// How strongly a hovered control's background is tinted with its text colour. A hovered control
/// also shows its text at full strength (a muted glyph or a translucent sub goes to the plain text
/// colour), so the hover reads as stronger, never weaker; `contrast`'s test checks the hovered
/// pairs in every palette.
pub const HOVER_TINT: f32 = 0.12;

/// The translucent fill a control hovered with text colour `text` lays over its background.
pub fn hover_tint(text: Hsla) -> Hsla {
    Hsla {
        a: HOVER_TINT,
        ..text
    }
}

/// How strongly a dialog's backdrop lays the text colour over the window behind it: enough to
/// say the window is waiting, while what is behind stays readable.
pub const SCRIM: f32 = 0.4;

/// The backdrop behind a dialog, in a palette whose text colour is `text`.
pub fn scrim(text: Hsla) -> Hsla {
    Hsla { a: SCRIM, ..text }
}

/// The custom colours `appearance` stores for `look`. Anything missing or malformed takes the
/// look's first palette, so a theme can always be built from them.
pub fn custom_palette(look: Look, appearance: &LookAppearance) -> CustomPalette {
    let fallback = CustomPalette::from_theme(&Theme::new(look, 0));
    CustomPalette::parse(&appearance.custom_palette, fallback)
}

/// How much of the background colour is laid over a background photo. Under it the picture
/// still moves the window's colour towards black or white, so text drawn straight on the window
/// is checked over both (`contrast::pairs`) and muted text there takes the text colour
/// (`Theme::window_muted`).
pub const BACKDROP_SCRIM: f32 = 0.8;

/// The tint over a background photo, in a palette whose background is `bg`.
pub fn backdrop_scrim(bg: Hsla) -> Hsla {
    Hsla {
        a: BACKDROP_SCRIM,
        ..bg
    }
}

/// Makes `theme` the active one and redraws every window with it.
pub fn set_theme(theme: Theme, cx: &mut App) {
    kit::sync_kit_theme(&theme, cx);
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

/// A built-in palette as the picker shows it: its name and four swatches.
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
    /// Cards in a day group that has ended; `surface` unless the look dims them.
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
    /// The title bar dot and the window control glyphs.
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
    /// The bulk bar's main action; the accent unless the look pins it.
    pub bulk_main_bg: Hsla,
    pub bulk_main_text: Hsla,
    pub focus: Hsla,
    /// The focus ring on the bulk bar, whose dark or coloured `bulk_bg` can swallow `focus`.
    pub bulk_focus: Hsla,
    pub focus_halo: Hsla,
    pub sticker: Hsla,
    pub sticker_line: Hsla,
    pub anno: Hsla,
    /// The border of a field whose text isn't valid, such as a hex colour that doesn't parse.
    /// The mockups show none; each look takes a red that stands out on its fields.
    pub warning: Hsla,
    /// Outline around the chosen colour swatch in the editor panel.
    pub swatch_on: Hsla,
    pub load_bg: Hsla,
}

impl Colors {
    /// The editor's stroke swatches, in the panel's order: the look's ink, white, its annotation
    /// colour, then the yellow and blue the mockups give every look.
    pub fn stroke_swatches(&self) -> [Swatch; 5] {
        [
            Swatch::new(self.text),
            Swatch::new(gpui::white()),
            Swatch::new(self.anno),
            // The mockups' CSS writes these two out rather than naming tokens.
            Swatch::new(css::hex(0xf1c40f)),
            Swatch::new(css::hex(0x2e86de)),
        ]
    }

    /// The digits on a number badge: the mockups write `#fff` out for every look rather than
    /// naming a token.
    pub fn badge_text(&self) -> Hsla {
        gpui::white()
    }

    /// The on-states as the base CSS draws them: the accent fill with the accent text. A look
    /// whose `extra_css` pins them to other colours sets those after this.
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

/// A stroke colour the editor's panel offers, with the name assistive technology reads: what it
/// looks like in this palette, which a custom palette decides.
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

/// The everyday name of a colour. A dark colour keeps a name of its own (maroon, olive, navy)
/// unless it is nearly black or has little colour in it.
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

/// A background that may be flat, a two-stop gradient, or one of the patterned fills the other
/// looks use for thumbnails and the editor canvas.
#[derive(Clone)]
pub enum Fill {
    Solid(Hsla),
    Gradient {
        angle: f32,
        from: Hsla,
        to: Hsla,
    },
    /// Two colours alternating in squares of `cell`.
    Checker {
        a: Hsla,
        b: Hsla,
        cell: Pixels,
    },
    /// `line` drawn every `cell` over `bg`.
    Grid {
        bg: Hsla,
        line: Hsla,
        cell: Pixels,
    },
}

impl Fill {
    /// What GPUI can paint directly. Patterns return their base colour; `ui::fill` paints them
    /// in full.
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
    /// Between the window controls: the hair rule, except where a look pins it.
    pub title_divider: Border,
}

pub struct Radii {
    pub frame: Pixels,
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub pill: Pixels,
    /// The title bar dot: a circle in soft looks, square in the hard ones.
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

/// The control sizes a look changes. Everything else is the same in every look's CSS and is set
/// where the control is drawn.
pub struct Sizes {
    /// The horizontal padding of `.btn.sm`.
    pub button_sm_padding: Pixels,
    /// The horizontal padding of a thumbnail's caption tag.
    pub tag_padding_x: Pixels,
    /// A look that draws each window control as a filled square of this size instead of a glyph.
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

/// Family names as GPUI's text system knows them.
pub struct Fonts {
    pub display: SharedString,
    pub body: SharedString,
    pub mono: SharedString,
}

impl Fonts {
    pub fn family(&self, family: Family) -> SharedString {
        match family {
            Family::Display => self.display.clone(),
            Family::Body => self.body.clone(),
            Family::Mono => self.mono.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Display,
    Body,
    Mono,
}

/// How one text role is set. Each look picks all four per role.
#[derive(Clone, Copy)]
pub struct TypeStyle {
    pub family: Family,
    pub size: Pixels,
    pub weight: FontWeight,
    pub case: Case,
}

impl TypeStyle {
    pub fn new(family: Family, size: f32, weight: FontWeight, case: Case) -> TypeStyle {
        TypeStyle {
            family,
            size: px(size),
            weight,
            case,
        }
    }

    fn font(&self, theme: &Theme) -> Font {
        Font {
            weight: self.weight,
            ..font(theme.fonts.family(self.family))
        }
    }

    /// The height of one line at CSS `line-height: normal`: the font's ascent plus descent, for a
    /// control whose height comes from its text, as the mockups' chips do.
    pub fn natural_line_height(&self, theme: &Theme, cx: &App) -> Pixels {
        let text_system = cx.text_system();
        let font_id = text_system.resolve_font(&self.font(theme));
        // The Windows text system reports the descent as a negative distance.
        text_system.ascent(font_id, self.size) + text_system.descent(font_id, self.size).abs()
    }

    /// The width of `text` set in this style, with its case applied, for a layout that has to be
    /// known before the text is laid out. Rounded up, as GPUI's text element rounds its width.
    pub fn text_width(&self, text: &str, theme: &Theme, window: &Window) -> Pixels {
        let text = self.case.apply(text);
        let run = TextRun {
            len: text.len(),
            font: self.font(theme),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .layout_line(&text, self.size, &[run], None)
            .width
            .ceil()
    }
}

/// `.type_style(theme, style)` sets the family, size and weight in one go. Apply `style.case`
/// to the text itself.
pub trait ThemedText: Styled + Sized {
    fn type_style(self, theme: &Theme, style: TypeStyle) -> Self {
        self.font_family(theme.fonts.family(style.family))
            .text_size(style.size)
            .font_weight(style.weight)
    }
}

impl<T: Styled> ThemedText for T {}

/// One `TypeStyle` per text role, since each look sets its own.
pub struct TypeScale {
    pub body: TypeStyle,
    pub hint: TypeStyle,
    pub label: TypeStyle,
    pub section: TypeStyle,
    pub day: TypeStyle,
    /// The count beside a day group's date.
    pub day_count: TypeStyle,
    pub button: TypeStyle,
    pub button_sm: TypeStyle,
    pub primary_button: TypeStyle,
    /// A small primary button, such as a card's Save: the looks size it apart from `button_sm`.
    pub primary_button_sm: TypeStyle,
    /// The secondary part of a button label, such as its shortcut.
    pub button_sub: TypeStyle,
    pub chip: TypeStyle,
    pub tag: TypeStyle,
    pub field: TypeStyle,
    /// A segmented control's buttons.
    pub segment: TypeStyle,
    /// Setting rows: toggle labels and shortcut keys.
    pub setting: TypeStyle,
    pub title_bar: TypeStyle,
    /// The window control glyphs.
    pub controls: TypeStyle,
    pub toast: TypeStyle,
    /// The return pill.
    pub jump: TypeStyle,
    pub load_more: TypeStyle,
    /// Select mode's bulk bar: its buttons, the bold "3 selected", and the small counts.
    pub bulk_button: TypeStyle,
    pub bulk_selected: TypeStyle,
    pub bulk_count: TypeStyle,
    /// Editor: the shortcut letter on a tool, the context panel's option buttons, the zoom and
    /// undo corner buttons, and the text and numbered annotations drawn on the canvas.
    pub tool_key: TypeStyle,
    pub option: TypeStyle,
    pub corner: TypeStyle,
    pub anno_text: TypeStyle,
    pub anno_number: TypeStyle,
    /// The overlay's size readout, in the mono face so the digits hold still as they change.
    pub readout: TypeStyle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    AsWritten,
    Upper,
}

impl Case {
    pub fn apply(self, text: &str) -> String {
        match self {
            Case::AsWritten => text.to_string(),
            Case::Upper => text.to_uppercase(),
        }
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

/// The strings whose tone changes with the look. Dates go where the caller puts them.
pub struct Strings {
    /// Around the day being fetched: "Fetching " + date + " …".
    pub loading: (&'static str, &'static str),
    pub copied: &'static str,
    pub back_to_session: &'static str,
    /// Before the day in a history group header.
    pub earlier: &'static str,
    /// The header of today's captures from sessions that have ended.
    pub earlier_today: &'static str,
    pub session: &'static str,
    pub day_date: DayDate,
}

/// How a day group writes its date.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DayDate {
    /// "Tue, Nov 4"
    Long,
    /// "NOV 4"
    ShortUpper,
    /// "TUE NOV 4"
    LongUpper,
}

#[cfg(test)]
mod tests {
    use super::*;

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
