use gpui::{App, Font, FontWeight, Hsla, Pixels, SharedString, Styled, TextRun, Window, font, px};

use super::Theme;

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

    /// CSS `line-height: normal`, as the mockups' chips take their height from.
    pub fn natural_line_height(&self, theme: &Theme, cx: &App) -> Pixels {
        let text_system = cx.text_system();
        let font_id = text_system.resolve_font(&self.font(theme));
        // The Windows text system reports the descent as a negative distance.
        text_system.ascent(font_id, self.size) + text_system.descent(font_id, self.size).abs()
    }

    /// Rounded up, as GPUI's text element rounds its width.
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

/// Doesn't apply `style.case`; apply it to the text itself.
pub trait ThemedText: Styled + Sized {
    fn type_style(self, theme: &Theme, style: TypeStyle) -> Self {
        self.font_family(theme.fonts.family(style.family))
            .text_size(style.size)
            .font_weight(style.weight)
    }
}

impl<T: Styled> ThemedText for T {}

pub struct TypeScale {
    pub body: TypeStyle,
    pub hint: TypeStyle,
    pub label: TypeStyle,
    pub section: TypeStyle,
    pub day: TypeStyle,
    pub day_count: TypeStyle,
    pub button: TypeStyle,
    pub button_sm: TypeStyle,
    pub primary_button: TypeStyle,
    /// Such as a card's Save; the looks size it apart from `button_sm`.
    pub primary_button_sm: TypeStyle,
    /// The secondary part of a button label, such as its shortcut.
    pub button_sub: TypeStyle,
    pub chip: TypeStyle,
    pub tag: TypeStyle,
    pub field: TypeStyle,
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
    pub bulk_button: TypeStyle,
    pub bulk_selected: TypeStyle,
    pub bulk_count: TypeStyle,
    /// Editor: `option` is the context panel's buttons, `corner` the zoom and undo buttons.
    pub tool_key: TypeStyle,
    pub option: TypeStyle,
    pub corner: TypeStyle,
    pub anno_text: TypeStyle,
    pub anno_number: TypeStyle,
    /// In the mono face, so the digits hold still as they change.
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
