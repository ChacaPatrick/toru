//! The tools of DESIGN.md §5, in the pill's order and groups, each with its shortcut letter,
//! icon and hint, the shape a drawing tool leaves and the options its panel holds.

use super::annotation::{Kind, Shape};
use super::sticker::Sticker;
use crate::theme::{Case, Family, Theme};
use crate::ui::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Pan,
    Crop,
    Block,
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Draw,
    Highlighter,
    Text,
    Number,
    Stickers,
    Eraser,
}

/// The pill's groups, separated by a rule.
pub const GROUPS: [&[Tool]; 5] = [
    &[Tool::Select, Tool::Pan],
    &[Tool::Crop, Tool::Block],
    &[Tool::Arrow, Tool::Line, Tool::Rectangle, Tool::Ellipse],
    &[
        Tool::Draw,
        Tool::Highlighter,
        Tool::Text,
        Tool::Number,
        Tool::Stickers,
    ],
    &[Tool::Eraser],
];

impl Tool {
    /// Every tool, in pill order.
    pub fn all() -> impl Iterator<Item = Tool> {
        GROUPS.iter().flat_map(|group| group.iter().copied())
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Pan => "Pan",
            Tool::Crop => "Crop",
            Tool::Block => "Block / blur",
            Tool::Arrow => "Arrow",
            Tool::Line => "Line",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Draw => "Draw",
            Tool::Highlighter => "Highlighter",
            Tool::Text => "Text",
            Tool::Number => "Number badge",
            Tool::Stickers => "Stickers",
            Tool::Eraser => "Eraser",
        }
    }

    /// The shortcut letter, as drawn on the tool.
    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Pan => "H",
            Tool::Crop => "C",
            Tool::Block => "B",
            Tool::Arrow => "A",
            Tool::Line => "L",
            Tool::Rectangle => "R",
            Tool::Ellipse => "O",
            Tool::Draw => "P",
            Tool::Highlighter => "M",
            Tool::Text => "T",
            Tool::Number => "N",
            Tool::Stickers => "S",
            Tool::Eraser => "E",
        }
    }

    /// The tool a bare key press picks, from the keystroke's lowercase key.
    pub fn for_key(key: &str) -> Option<Tool> {
        Tool::all().find(|tool| tool.key().eq_ignore_ascii_case(key))
    }

    pub fn icon(self) -> Icon {
        match self {
            Tool::Select => Icon::Select,
            Tool::Pan => Icon::Pan,
            Tool::Crop => Icon::Crop,
            Tool::Block => Icon::Block,
            Tool::Arrow => Icon::Arrow,
            Tool::Line => Icon::Line,
            Tool::Rectangle => Icon::Rectangle,
            Tool::Ellipse => Icon::Ellipse,
            Tool::Draw => Icon::Draw,
            Tool::Highlighter => Icon::Highlighter,
            Tool::Text => Icon::Text,
            Tool::Number => Icon::Number,
            Tool::Stickers => Icon::Stickers,
            Tool::Eraser => Icon::Eraser,
        }
    }

    /// The shape the tool drags out, or `None` for a tool that doesn't draw one. The Arrow takes
    /// its head from its options.
    pub fn shape(self, head: Head) -> Option<Shape> {
        Some(match self {
            Tool::Arrow => Shape::Arrow(head),
            Tool::Line => Shape::Line,
            Tool::Rectangle => Shape::Rectangle,
            Tool::Ellipse => Shape::Ellipse,
            Tool::Draw => Shape::Pen,
            Tool::Highlighter => Shape::Highlighter,
            _ => return None,
        })
    }

    /// The kind of mark the tool leaves, dragged out or placed, or `None` for a tool that
    /// leaves none.
    pub fn kind(self) -> Option<Kind> {
        Some(match self {
            Tool::Text => Kind::Text,
            Tool::Number => Kind::Number,
            Tool::Stickers => Kind::Sticker,
            _ => return self.shape(Head::Single).map(|shape| shape.kind()),
        })
    }

    /// "Select (V)", as assistive technology reads the tool.
    pub fn aria_label(self) -> String {
        format!("{} ({})", self.label(), self.key())
    }

    /// The hint chip's text while the tool is active.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "Click an annotation to select it · Drag to move it · Delete removes it"
            }
            Tool::Pan => "Drag to move the picture · Scroll to zoom · Esc back to Select",
            Tool::Crop => "Drag to choose what to keep · Enter crops · Esc back to Select",
            Tool::Block => "Drag over what to hide · Esc back to Select",
            Tool::Arrow => "Drag to draw an arrow · Shift keeps it straight · Esc back to Select",
            Tool::Line => "Drag to draw a line · Shift keeps it straight · Esc back to Select",
            Tool::Rectangle => {
                "Drag to draw a rectangle · Shift makes a square · Esc back to Select"
            }
            Tool::Ellipse => "Drag to draw an ellipse · Shift makes a circle · Esc back to Select",
            Tool::Draw => "Drag to draw freehand · Esc back to Select",
            Tool::Highlighter => "Drag to highlight · Esc back to Select",
            Tool::Text => "Click to place text · Click text to edit it · Esc back to Select",
            Tool::Number => "Click to place the next number · Esc back to Select",
            Tool::Stickers => "Pick a sticker, then click to place it · Esc back to Select",
            Tool::Eraser => "Click an annotation to remove it · Esc back to Select",
        }
    }

    /// The hint while text is being typed.
    pub const TYPING_HINT: &'static str =
        "Type · Enter for a new line · Esc or click away to finish";
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    Thin,
    Medium,
    Thick,
}

impl Width {
    pub const ALL: [Width; 3] = [Width::Thin, Width::Medium, Width::Thick];

    pub fn label(self) -> &'static str {
        match self {
            Width::Thin => "Thin",
            Width::Medium => "Medium",
            Width::Thick => "Thick",
        }
    }

    /// The stroke in picture pixels, which is also the height of its bar in the panel.
    pub fn pixels(self) -> f32 {
        match self {
            Width::Thin => 1.,
            Width::Medium => 3.,
            Width::Thick => 5.,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Head {
    /// →
    Single,
    /// ↔
    Double,
    /// ⟶
    Long,
    /// ↝
    Curly,
}

impl Head {
    pub const ALL: [Head; 4] = [Head::Single, Head::Double, Head::Long, Head::Curly];

    pub fn glyph(self) -> &'static str {
        match self {
            Head::Single => "→",
            Head::Double => "↔",
            Head::Long => "⟶",
            Head::Curly => "↝",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Head::Single => "Arrow head at the end",
            Head::Double => "Arrow heads at both ends",
            Head::Long => "Long arrow head",
            Head::Curly => "Curly arrow",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStyle {
    Solid,
    Dashed,
    Sketchy,
}

impl LineStyle {
    pub const ALL: [LineStyle; 3] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Sketchy];

    pub fn label(self) -> &'static str {
        match self {
            LineStyle::Solid => "Solid",
            LineStyle::Dashed => "Dashed",
            LineStyle::Sketchy => "Sketchy",
        }
    }
}

/// A text annotation's size, as a multiple of the look's `anno_text` size: the mockup's text is
/// the smallest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextSize {
    Small,
    Medium,
    Large,
    Huge,
}

impl TextSize {
    pub const ALL: [TextSize; 4] = [
        TextSize::Small,
        TextSize::Medium,
        TextSize::Large,
        TextSize::Huge,
    ];

    pub fn scale(self) -> f32 {
        match self {
            TextSize::Small => 1.,
            TextSize::Medium => 1.5,
            TextSize::Large => 2.,
            TextSize::Huge => 3.,
        }
    }

    /// The short label on the option.
    pub fn glyph(self) -> &'static str {
        match self {
            TextSize::Small => "S",
            TextSize::Medium => "M",
            TextSize::Large => "L",
            TextSize::Huge => "XL",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TextSize::Small => "Small",
            TextSize::Medium => "Medium",
            TextSize::Large => "Large",
            TextSize::Huge => "Extra large",
        }
    }

    /// The text size in picture pixels in `theme`'s look.
    pub fn pixels(self, theme: &Theme) -> f32 {
        f32::from(theme.type_scale.anno_text.size) * self.scale()
    }

    /// The preset nearest to a text `pixels` big, for showing a text that was resized by hand.
    pub fn nearest(pixels: f32, theme: &Theme) -> TextSize {
        TextSize::ALL
            .into_iter()
            .min_by(|a, b| {
                (a.pixels(theme) - pixels)
                    .abs()
                    .total_cmp(&(b.pixels(theme) - pixels).abs())
            })
            .unwrap_or(TextSize::Small)
    }
}

/// The context panel's groups. Each tool shows the ones it needs, in this order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Stroke,
    Width,
    Head,
    Style,
    Font,
    Size,
    Sticker,
    Opacity,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Stroke => "Stroke",
            Group::Width => "Width",
            Group::Head => "Arrow head",
            Group::Style => "Style",
            Group::Font => "Font",
            Group::Size => "Size",
            Group::Sticker => "Sticker",
            Group::Opacity => "Opacity",
        }
    }
}

/// The panel's groups for a kind of mark: the Arrow's full set from the mockup, the other lines
/// and boxes without the head, the pens without a line style, which freehand doesn't need, text
/// with its font and size, a badge with its colour, and the stickers' tray.
pub fn groups(kind: Kind) -> &'static [Group] {
    match kind {
        Kind::Arrow => &[
            Group::Stroke,
            Group::Width,
            Group::Head,
            Group::Style,
            Group::Opacity,
        ],
        Kind::Line | Kind::Rectangle | Kind::Ellipse => {
            &[Group::Stroke, Group::Width, Group::Style, Group::Opacity]
        }
        Kind::Pen | Kind::Highlighter => &[Group::Stroke, Group::Width, Group::Opacity],
        Kind::Text => &[Group::Stroke, Group::Font, Group::Size, Group::Opacity],
        Kind::Number => &[Group::Stroke],
        Kind::Sticker => &[Group::Sticker],
    }
}

/// A tool's panel choices, kept per tool while the editor is open, and what a mark on the
/// picture is drawn with. Each tool reads the fields its groups show.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct StrokeOptions {
    /// Index into `Colors::stroke_swatches`.
    pub stroke: usize,
    pub width: Width,
    pub head: Head,
    pub style: LineStyle,
    /// Text: which of the look's two bundled families.
    pub font: Family,
    pub size: TextSize,
    pub sticker: Sticker,
    /// 0 to 1.
    pub opacity: f32,
}

impl StrokeOptions {
    /// The yellow swatch, which the highlighter starts on, and the look's annotation colour,
    /// which the mockups set text and badges in.
    const YELLOW: usize = 3;
    const ANNO: usize = 2;

    /// The mockups' chosen options: ink, medium, a single head, solid, fully opaque. The
    /// highlighter starts yellow and half-strength, so the text under it stays readable; text
    /// and badges start in the annotation colour, text in the look's `anno_text` face at the
    /// mockup's size.
    pub fn for_kind(kind: Kind, theme: &Theme) -> StrokeOptions {
        let mut options = StrokeOptions {
            stroke: 0,
            width: Width::Medium,
            head: Head::Single,
            style: LineStyle::Solid,
            font: theme.type_scale.anno_text.family,
            size: TextSize::Small,
            sticker: Sticker::Hand(0),
            opacity: 1.,
        };
        match kind {
            Kind::Highlighter => {
                options.stroke = StrokeOptions::YELLOW;
                options.opacity = 0.5;
            }
            Kind::Text | Kind::Number => options.stroke = StrokeOptions::ANNO,
            _ => {}
        }
        options
    }

    /// Whether the look sets text annotations in capitals.
    pub fn upper(theme: &Theme) -> bool {
        theme.type_scale.anno_text.case == Case::Upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toru_core::settings::Look;

    #[test]
    fn the_pill_lists_the_fourteen_tools_in_the_design_order() {
        let keys: String = Tool::all().map(Tool::key).collect();
        assert_eq!(keys, "VHCBALROPMTNSE");
    }

    #[test]
    fn every_shortcut_letter_is_distinct_and_finds_its_tool() {
        for tool in Tool::all() {
            assert_eq!(Tool::for_key(&tool.key().to_lowercase()), Some(tool));
            assert_eq!(Tool::for_key(tool.key()), Some(tool));
        }
        assert_eq!(Tool::for_key("x"), None);
        assert_eq!(Tool::for_key("escape"), None);
    }

    #[test]
    fn labels_read_as_the_mockups_write_them() {
        assert_eq!(Tool::Block.aria_label(), "Block / blur (B)");
        assert_eq!(Tool::Number.aria_label(), "Number badge (N)");
        assert_eq!(
            Tool::Arrow.hint(),
            "Drag to draw an arrow · Shift keeps it straight · Esc back to Select"
        );
    }

    #[test]
    fn only_the_six_drawing_tools_leave_a_shape() {
        let drawing: Vec<Tool> = Tool::all()
            .filter(|tool| tool.shape(Head::Single).is_some())
            .collect();
        assert_eq!(
            drawing,
            [
                Tool::Arrow,
                Tool::Line,
                Tool::Rectangle,
                Tool::Ellipse,
                Tool::Draw,
                Tool::Highlighter
            ]
        );
        assert_eq!(
            Tool::Arrow.shape(Head::Curly),
            Some(Shape::Arrow(Head::Curly))
        );
    }

    #[test]
    fn each_kind_shows_the_groups_it_needs() {
        assert_eq!(groups(Kind::Arrow).len(), 5);
        assert!(!groups(Kind::Line).contains(&Group::Head));
        assert_eq!(
            groups(Kind::Pen),
            &[Group::Stroke, Group::Width, Group::Opacity]
        );
        assert_eq!(
            groups(Kind::Text),
            &[Group::Stroke, Group::Font, Group::Size, Group::Opacity]
        );
        assert_eq!(groups(Kind::Sticker), &[Group::Sticker]);
        let theme = Theme::new(Look::Japandi, 0);
        let highlighter = StrokeOptions::for_kind(Kind::Highlighter, &theme);
        assert_eq!(highlighter.stroke, 3);
        assert_eq!(highlighter.opacity, 0.5);
        assert_eq!(StrokeOptions::for_kind(Kind::Line, &theme).opacity, 1.);
        let text = StrokeOptions::for_kind(Kind::Text, &theme);
        assert_eq!(text.stroke, 2);
        assert_eq!(text.font, Family::Body);
        assert!(!StrokeOptions::upper(&theme));
        assert!(StrokeOptions::upper(&Theme::new(Look::NeoBrutalism, 0)));
    }

    #[test]
    fn the_placing_tools_have_a_kind_but_no_dragged_shape() {
        assert_eq!(Tool::Text.kind(), Some(Kind::Text));
        assert_eq!(Tool::Number.kind(), Some(Kind::Number));
        assert_eq!(Tool::Stickers.kind(), Some(Kind::Sticker));
        assert_eq!(Tool::Text.shape(Head::Single), None);
        assert_eq!(Tool::Arrow.kind(), Some(Kind::Arrow));
        assert_eq!(Tool::Select.kind(), None);
        assert_eq!(Tool::Eraser.kind(), None);
    }

    #[test]
    fn text_sizes_scale_the_looks_size_and_round_back_to_the_nearest() {
        let theme = Theme::new(Look::Japandi, 0);
        assert_eq!(TextSize::Small.pixels(&theme), 16.);
        assert_eq!(TextSize::Huge.pixels(&theme), 48.);
        assert_eq!(TextSize::nearest(16., &theme), TextSize::Small);
        assert_eq!(TextSize::nearest(26., &theme), TextSize::Medium);
        assert_eq!(TextSize::nearest(200., &theme), TextSize::Huge);
    }
}
