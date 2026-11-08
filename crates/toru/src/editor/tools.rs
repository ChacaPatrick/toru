//! The tools of DESIGN.md §5, in the pill's order and groups, each with its shortcut letter,
//! icon and hint, and the options the Arrow panel holds. The shapes themselves come in 8.2.

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
            Tool::Text => "Click to place text · Esc back to Select",
            Tool::Number => "Click to place the next number · Esc back to Select",
            Tool::Stickers => "Pick a sticker, then click to place it · Esc back to Select",
            Tool::Eraser => "Click an annotation to remove it · Esc back to Select",
        }
    }
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

/// The Arrow panel's choices, kept while the editor is open. 8.2 reads them when it draws.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ArrowOptions {
    /// Index into `Colors::stroke_swatches`.
    pub stroke: usize,
    pub width: Width,
    pub head: Head,
    pub style: LineStyle,
    /// 0 to 1.
    pub opacity: f32,
}

impl Default for ArrowOptions {
    /// The mockups' chosen options: ink, medium, a single head, solid, fully opaque.
    fn default() -> Self {
        ArrowOptions {
            stroke: 0,
            width: Width::Medium,
            head: Head::Single,
            style: LineStyle::Solid,
            opacity: 1.,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
