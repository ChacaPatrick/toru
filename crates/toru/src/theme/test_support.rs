//! Slices of `design/tokens.json` for the per-look tests. Each slice stops at the next section,
//! so a value copied from the wrong look or palette is caught.

use gpui::{Hsla, Rgba};

use super::{Border, Palette};

/// One look's entry in `design/tokens.json`, as text.
pub struct Entry {
    text: String,
    palette_names: Vec<String>,
}

impl Entry {
    /// `look` is the entry's key; `next_look` the key after it, or `None` for the last entry.
    pub fn new(look: &str, next_look: Option<&str>, palettes: &[Palette]) -> Entry {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/tokens.json");
        let text = std::fs::read_to_string(path).expect("design/tokens.json is readable");
        let start = text.find(&format!("\"{look}\"")).unwrap();
        let end = next_look.map_or(text.len(), |next| {
            text.find(&format!("\"{next}\"")).unwrap()
        });
        Entry {
            text: text[start..end].to_string(),
            palette_names: palettes
                .iter()
                .map(|palette| format!("\"{}", palette.name))
                .collect(),
        }
    }

    /// The entry's base values, up to the palette overrides.
    pub fn base(&self) -> String {
        self.text[..self.text.find("\"palettes\"").unwrap()].to_string()
    }

    /// One palette's overrides, from its name to the next palette's.
    pub fn palette(&self, index: usize) -> String {
        let start = self.text.find(&self.palette_names[index]).unwrap();
        let end = self
            .palette_names
            .get(index + 1)
            .map_or(self.text.len(), |next| {
                self.text.find(next.as_str()).unwrap()
            });
        self.text[start..end].to_string()
    }
}

pub fn css_hex(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let channel = |value: f32| (value * 255.).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

/// `"{w}px solid #rrggbb"`, as tokens.json writes a border.
pub fn css_border(border: Border) -> String {
    let style = if border.dashed { "dashed" } else { "solid" };
    format!(
        "{}px {style} {}",
        f32::from(border.width),
        css_hex(border.color)
    )
}

pub fn assert_token(tokens: &str, var: &str, value: &str) {
    let line = format!("\"{var}\": \"{value}\"");
    assert!(tokens.contains(&line), "tokens.json lacks {line}");
}
