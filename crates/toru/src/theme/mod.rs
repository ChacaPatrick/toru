//! The look of the app as typed values, translated from `design/tokens.json`.
//!
//! `design/tokens.json` stays the reference. A look is a full `Theme`; a palette overrides some of
//! its values, exactly as the JSON does. Components read everything from the active theme and never
//! hard-code a colour or size.

pub mod contrast;
mod css;
mod fonts;
mod japandi;
mod kit;
mod neo_brutalism;
mod pixel_art;
#[cfg(test)]
mod test_support;

use gpui::{
    App, Background, BoxShadow, Font, FontWeight, Global, Hsla, Pixels, SharedString, Styled,
    TextRun, Window, font, px,
};
use toru_core::settings::{BUILT_IN_PALETTES, Look, PaletteChoice, Settings};

pub use fonts::install_fonts;

pub struct Theme {
    pub look: Look,
    /// Index into `Theme::palettes(look)`.
    pub palette: usize,
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
}

impl Global for Theme {}

impl Theme {
    /// The theme for the look and palette chosen in `settings`.
    pub fn from_settings(settings: &Settings) -> Theme {
        let look = settings.look;
        let palette = match settings.appearance(look).palette {
            PaletteChoice::BuiltIn(index) => index,
            // The custom slot arrives in Phase 9. Until then it shows the look's first palette.
            PaletteChoice::Custom => 0,
        };
        Theme::new(look, palette)
    }

    pub fn new(look: Look, palette: usize) -> Theme {
        match look {
            Look::Japandi => japandi::theme(palette),
            Look::PixelArt => pixel_art::theme(palette),
            Look::NeoBrutalism => neo_brutalism::theme(palette),
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
    /// Outline around the chosen colour swatch in the editor panel.
    pub swatch_on: Hsla,
    pub load_bg: Hsla,
}

impl Colors {
    /// The editor's stroke swatches, in the panel's order: the look's ink, white, its annotation
    /// colour, then the yellow and blue the mockups give every look.
    pub fn stroke_swatches(&self) -> [Swatch; 5] {
        [
            Swatch::new("Ink", self.text),
            Swatch::new("White", gpui::white()),
            Swatch::new("Red", self.anno),
            // The mockups' CSS writes these two out rather than naming tokens.
            Swatch::new("Yellow", css::hex(0xf1c40f)),
            Swatch::new("Blue", css::hex(0x2e86de)),
        ]
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

/// A stroke colour the editor's panel offers, with the name assistive technology reads.
#[derive(Clone, Copy)]
pub struct Swatch {
    pub name: &'static str,
    pub color: Hsla,
}

impl Swatch {
    fn new(name: &'static str, color: Hsla) -> Swatch {
        Swatch { name, color }
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

#[derive(Clone, Copy, PartialEq, Eq)]
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
