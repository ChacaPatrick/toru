//! WCAG 2 contrast: the pairs every control draws, for the `DESIGN.md` §7 acceptance test over
//! every look and palette and for the custom palette's contrast warning.

use gpui::{Hsla, Rgba, black, white};

use super::{Fill, Theme, backdrop_scrim, hover_tint};
use crate::editor::chrome::tool_key_color;
use crate::ui::SUB_OPACITY;

/// `fg` laid over an opaque `bg`, as the screen blends them.
pub fn over(fg: Hsla, bg: Hsla) -> Hsla {
    let (fg, bg) = (Rgba::from(fg), Rgba::from(bg));
    let mix = |f: f32, b: f32| fg.a * f + (1. - fg.a) * b;
    Rgba {
        r: mix(fg.r, bg.r),
        g: mix(fg.g, bg.g),
        b: mix(fg.b, bg.b),
        a: 1.,
    }
    .into()
}

fn relative_luminance(color: Hsla) -> f32 {
    let rgba = Rgba::from(color);
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgba.r) + 0.7152 * linear(rgba.g) + 0.0722 * linear(rgba.b)
}

/// The contrast ratio of two opaque colours, 1 to 21.
pub fn ratio(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// §7: all text 4.5:1; 3:1 for text of 24px or more, which no type style reaches. Focus rings
/// are non-text UI, so WCAG 1.4.11's 3:1 applies.
pub const TEXT: f32 = 4.5;
pub const RING: f32 = 3.0;

/// A foreground laid over a background somewhere in the app, and the ratio it must reach.
pub struct Pair {
    pub name: String,
    pub fg: Hsla,
    pub bg: Hsla,
    pub min: f32,
}

impl Pair {
    pub fn ratio(&self) -> f32 {
        ratio(self.fg, self.bg)
    }
}

/// The colours a `Fill` can show under something: each solid stop.
fn stops(fill: &Fill) -> Vec<Hsla> {
    match *fill {
        Fill::Solid(color) => vec![color],
        Fill::Gradient { from, to, .. } => vec![from, to],
        Fill::Checker { a, b, .. } => vec![a, b],
        Fill::Grid { bg, line, .. } => vec![bg, line],
    }
}

/// Every text-on-background pair the controls draw, and the focus ring against everything it
/// sits on: the window, the three surfaces, a selected card (its name field and chips) and the
/// bulk bar. Translucent foregrounds and backgrounds are blended over what lies beneath them
/// first. With a background photo, the window is also taken over a black and over a white
/// picture: under the scrim its colour moves with the picture, channel by channel, so those two
/// bound what any photo can do to it.
pub fn pairs(theme: &Theme) -> Vec<Pair> {
    let c = &theme.colors;
    let mut pairs = Vec::new();
    let mut text = |name: &str, fg: Hsla, bg: Hsla| {
        pairs.push(Pair {
            name: name.to_string(),
            fg,
            bg,
            min: TEXT,
        })
    };
    let mut windows = vec![("bg", c.bg)];
    if theme.background.is_some() {
        let scrim = backdrop_scrim(c.bg);
        windows.push(("bg over a dark photo", over(scrim, black())));
        windows.push(("bg over a light photo", over(scrim, white())));
    }
    let window_muted = theme.window_muted();
    for (name, bg) in &windows {
        text(&format!("text on {name}"), c.text, *bg);
        text(&format!("muted on {name}"), window_muted, *bg);
        text(
            &format!("load more on {name}"),
            window_muted,
            over(c.load_bg, *bg),
        );
    }
    let surfaces = [
        ("surface", c.surface),
        ("surface_2", c.surface_2),
        ("surface_sel", c.surface_sel),
        ("earlier_card_bg", c.earlier_card_bg),
        ("input_bg", c.input_bg),
    ];
    for (name, bg) in surfaces {
        text(&format!("text on {name}"), c.text, bg);
        text(&format!("muted on {name}"), c.muted, bg);
    }
    text("label on bg", c.label, c.bg);
    text("label on surface_2", c.label, c.surface_2);
    text("button", c.button_text, c.button_bg);
    // Only plain buttons carry a sub (`Display 1 ▾`, `5 s ▾`); the primary ones never do.
    let sub = Hsla {
        a: SUB_OPACITY,
        ..c.button_text
    };
    text("button sub", over(sub, c.button_bg), c.button_bg);
    // Hovered, a split sub shows the full text colour over a tint of it.
    text(
        "hovered button sub",
        c.button_text,
        over(hover_tint(c.button_text), c.button_bg),
    );
    text("primary button", c.accent_text, c.accent_bg);
    text("chosen segment", c.on_text, c.on_bg);
    // The editor's tool pill: a tool's shortcut letter, and the chosen tool's.
    text("tool key", tool_key_color(theme, false), c.surface);
    text("chosen tool key", tool_key_color(theme, true), c.on_bg);
    for (name, bg) in &surfaces[..4] {
        text(
            &format!("chip on {name}"),
            c.chip_text,
            over(c.chip_bg, *bg),
        );
    }
    text("saved chip", c.chip_saved_text, c.chip_saved_bg);
    // The overlay's readout panel borrows the toast's colours, so this covers it too.
    text("toast", c.toast_text, c.toast_bg);
    text("bulk bar", c.bulk_text, c.bulk_bg);
    text(
        "bulk count",
        over(
            Hsla {
                a: 0.7,
                ..c.bulk_text
            },
            c.bulk_bg,
        ),
        c.bulk_bg,
    );
    text("bulk main button", c.bulk_main_text, c.bulk_main_bg);
    text("title bar", c.title_text, c.title_bg);
    text("window controls", c.controls, c.title_bg);
    text(
        "hovered window control",
        c.title_text,
        over(hover_tint(c.title_text), c.title_bg),
    );
    for (index, thumb) in std::iter::once(&c.thumb).chain(&c.thumb_alt).enumerate() {
        for (stop, color) in stops(thumb).into_iter().enumerate() {
            text(
                &format!("tag on thumb {index} stop {stop}"),
                c.tag_text,
                over(c.tag_bg, color),
            );
        }
    }
    let rings = windows
        .iter()
        .chain(&surfaces[..3])
        .map(|(name, bg)| (*name, c.focus, *bg))
        .chain([("bulk_bg", c.bulk_focus, c.bulk_bg)]);
    for (name, ring, bg) in rings {
        for (what, bg) in [("", bg), (" halo", over(c.focus_halo, bg))] {
            pairs.push(Pair {
                name: format!("focus ring on {name}{what}"),
                fg: ring,
                bg,
                min: RING,
            });
        }
    }
    pairs
}

/// A pair that falls short of its ratio.
#[derive(Debug)]
pub struct Failure {
    pub name: String,
    pub ratio: f32,
    pub min: f32,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is {:.1}:1, needs {}:1",
            self.name, self.ratio, self.min
        )
    }
}

/// Every pair in `theme` that misses DESIGN.md §7, in the order `pairs` lists them. The §7
/// acceptance test runs this over the built-in palettes; the custom palette editor shows it.
pub fn failures(theme: &Theme) -> Vec<Failure> {
    pairs(theme)
        .into_iter()
        .filter(|pair| pair.ratio() < pair.min)
        .map(|pair| Failure {
            ratio: pair.ratio(),
            name: pair.name,
            min: pair.min,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::css::hex;
    use toru_core::settings::{BUILT_IN_PALETTES, Look};

    /// Every built-in palette. Only the custom palette can have a photo behind it; `custom`'s
    /// tests cover that.
    fn themes() -> impl Iterator<Item = Theme> {
        [Look::Japandi, Look::PixelArt, Look::NeoBrutalism]
            .into_iter()
            .flat_map(|look| (0..BUILT_IN_PALETTES).map(move |palette| Theme::new(look, palette)))
    }

    fn describe(theme: &Theme) -> String {
        format!(
            "{:?} / {}",
            theme.look,
            Theme::palettes(theme.look)[theme.palette].name
        )
    }

    /// With a photo behind the window, the pairs take in the window over a black and a white
    /// picture, and name them.
    #[test]
    fn a_photo_adds_the_window_over_dark_and_light_pictures() {
        let mut theme = Theme::new(Look::Japandi, 0);
        let plain = pairs(&theme).len();
        theme.background = Some("photo.jpg".into());
        let names: Vec<String> = pairs(&theme).into_iter().map(|pair| pair.name).collect();
        assert!(names.len() > plain);
        for name in ["text", "muted", "load more", "focus ring"] {
            for photo in ["dark", "light"] {
                let wanted = format!("{name} on bg over a {photo} photo");
                assert!(names.contains(&wanted), "{wanted}: {names:?}");
            }
        }
    }

    #[test]
    fn known_ratios() {
        assert!((ratio(hex(0x000000), hex(0xffffff)) - 21.).abs() < 0.01);
        assert!((ratio(hex(0x777777), hex(0xffffff)) - 4.48).abs() < 0.01);
        assert_eq!(ratio(hex(0x123456), hex(0x123456)), 1.);
        // Half-transparent white over black is mid grey.
        let grey = Rgba::from(over(
            Hsla {
                a: 0.5,
                ..hex(0xffffff)
            },
            hex(0x000000),
        ));
        assert!((grey.r - 0.5).abs() < 0.001 && grey.a == 1.);
    }

    #[test]
    fn every_palette_meets_design_md_7() {
        let failures: Vec<String> = themes()
            .flat_map(|theme| {
                let name = describe(&theme);
                failures(&theme)
                    .into_iter()
                    .map(move |failure| format!("{name}: {failure}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(
            failures.is_empty(),
            "
{}
",
            failures.join(
                "
"
            )
        );
    }

    /// `cargo test -p toru print_contrast_table -- --ignored --nocapture` lists every pair.
    #[test]
    #[ignore]
    fn print_contrast_table() {
        for theme in themes() {
            println!("== {}", describe(&theme));
            for pair in pairs(&theme) {
                println!("{:6.2}  {}", pair.ratio(), pair.name);
            }
        }
    }
}
