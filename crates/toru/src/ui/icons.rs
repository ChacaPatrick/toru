//! The mockups' line icons, with the stroke from the theme's `Icons`. Each is the body of a
//! 24-unit SVG from `design/build-mockups.py`; GPUI rasterises the markup as a mask and tints it.

use gpui::{Hsla, Pixels, Svg, prelude::*, px, svg};

use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Region,
    Window,
    Screen,
    Clock,
    Plus,
    Gear,
    Check,
    Copy,
    Back,
    Undo,
    Redo,
    Paths,
    Save,
    Merge,
    Trash,
    ArrowUp,
    Spinner,
    ChevronDown,
    ChevronUp,
    // The editor's tools, in pill order.
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

impl Icon {
    /// The size the mockups draw it at.
    pub fn size(self) -> Pixels {
        match self {
            Icon::Check
            | Icon::Copy
            | Icon::Back
            | Icon::Paths
            | Icon::Save
            | Icon::Merge
            | Icon::Trash
            | Icon::ArrowUp
            | Icon::Spinner
            | Icon::ChevronDown
            | Icon::ChevronUp => px(16.),
            Icon::Region
            | Icon::Window
            | Icon::Screen
            | Icon::Clock
            | Icon::Plus
            | Icon::Gear
            | Icon::Undo
            | Icon::Redo => px(18.),
            _ => px(20.),
        }
    }

    fn body(self) -> &'static str {
        match self {
            Icon::Region => r#"<path d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4"/>"#,
            Icon::Window => {
                r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>"#
            }
            Icon::Screen => {
                r#"<rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>"#
            }
            Icon::Clock => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
            Icon::Plus => r#"<path d="M12 5v14M5 12h14"/>"#,
            Icon::Gear => {
                r#"<circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.5-2.3 1a7 7 0 0 0-1.7-1L14.5 3h-5l-.4 2.6a7 7 0 0 0-1.7 1l-2.3-1-2 3.5 2 1.5a7 7 0 0 0 0 2l-2 1.5 2 3.5 2.3-1a7 7 0 0 0 1.7 1l.4 2.6h5l.4-2.6a7 7 0 0 0 1.7-1l2.3 1 2-3.5-2-1.5c.1-.3.1-.7.1-1z"/>"#
            }
            Icon::Check => r#"<path d="M5 12l5 5L20 7"/>"#,
            Icon::Copy => {
                r#"<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5h10"/>"#
            }
            Icon::Back => r#"<path d="M19 12H5M12 19l-7-7 7-7"/>"#,
            Icon::Undo => r#"<path d="M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3"/>"#,
            Icon::Redo => r#"<path d="M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3"/>"#,
            Icon::Paths => r#"<path d="M4 6h16M4 12h16M4 18h10"/>"#,
            Icon::Save => {
                r#"<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2zM17 21v-8H7v8M7 3v5h8"/>"#
            }
            Icon::Merge => {
                r#"<path d="M3 7l9 6 9-6M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V7M3 7l9-4 9 4"/>"#
            }
            Icon::Trash => r#"<path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14M10 11v6M14 11v6"/>"#,
            Icon::ArrowUp => r#"<path d="M12 19V5M5 12l7-7 7 7"/>"#,
            Icon::Spinner => r#"<path d="M21 12a9 9 0 1 1-3-6.7"/>"#,
            Icon::ChevronDown => r#"<path d="M6 9l6 6 6-6"/>"#,
            Icon::ChevronUp => r#"<path d="M6 15l6-6 6 6"/>"#,
            Icon::Select => r#"<path d="M5 3l14 9-6 1.5L10 20z"/>"#,
            Icon::Pan => {
                r#"<path d="M8 13V5a1.5 1.5 0 0 1 3 0v6M11 10V4a1.5 1.5 0 0 1 3 0v7M14 11V6a1.5 1.5 0 0 1 3 0v8a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-2.7L3 13a1.6 1.6 0 0 1 2.7-1.7L8 14"/>"#
            }
            Icon::Crop => r#"<path d="M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14"/>"#,
            Icon::Block => {
                r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9l6-6M3 15L15 3M9 21L21 9M15 21l6-6"/>"#
            }
            Icon::Arrow => r#"<path d="M5 19L19 5M19 5h-8M19 5v8"/>"#,
            Icon::Line => r#"<path d="M5 19L19 5"/>"#,
            Icon::Rectangle => r#"<rect x="4" y="5" width="16" height="14" rx="2"/>"#,
            Icon::Ellipse => r#"<ellipse cx="12" cy="12" rx="8" ry="7"/>"#,
            Icon::Draw => {
                r#"<path d="M12 19l7-7 3 3-7 7-3-3zM18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5zM2 2l7.6 7.6"/>"#
            }
            Icon::Highlighter => {
                r#"<path d="M9 11l-6 6v3h9l3-3M22 12l-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>"#
            }
            Icon::Text => r#"<path d="M4 7V4h16v3M9 20h6M12 4v16"/>"#,
            Icon::Number => r#"<circle cx="12" cy="12" r="9"/><path d="M10 9l2-1.5V16"/>"#,
            Icon::Stickers => {
                r#"<path d="M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V4.5a1.5 1.5 0 0 1 3 0V11M14 11V6a1.5 1.5 0 0 1 3 0v7.5a6 6 0 0 1-6 6h-.6a6 6 0 0 1-5.1-2.9L3.4 13a1.6 1.6 0 0 1 2.7-1.7L8 13.5"/>"#
            }
            Icon::Eraser => {
                r#"<path d="M20 20H8L3 15a2 2 0 0 1 0-3l9-9a2 2 0 0 1 3 0l6 6a2 2 0 0 1 0 3l-7 7M6.5 11.5l6 6"/>"#
            }
        }
    }

    pub fn render(self, theme: &Theme, color: Hsla) -> Svg {
        let icons = &theme.icons;
        let (cap, join) = if icons.square {
            ("square", "miter")
        } else {
            ("round", "round")
        };
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}">{}</svg>"##,
            f32::from(icons.stroke),
            self.body()
        );
        svg()
            .data(markup.as_bytes())
            .size(self.size())
            .flex_none()
            .text_color(color)
    }
}
