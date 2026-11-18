use gpui::{FontWeight, Hsla, px};
use toru_core::settings::Look;

use super::{
    Border, Borders, CUSTOM_PALETTE, Colors, CustomPalette, DayDate, EmptyState, Fill, Fonts,
    Icons, Ornament, Overlay, Palette, Radii, Shadows, Sizes, Strings, Theme, TypeScale,
    css::{body, display, gradient, hex, hex_alpha, mono, soft_shadow},
    custom::mix,
};

pub fn palettes() -> [Palette; 3] {
    [
        Palette {
            name: "Linen & walnut",
            swatches: [hex(0xefe9df), hex(0xfaf7f1), hex(0x7a5c3e), hex(0x2b2a26)],
        },
        Palette {
            name: "Stone & moss",
            swatches: [hex(0xe9ebe6), hex(0xf6f7f4), hex(0x4f6b4a), hex(0x23261f)],
        },
        Palette {
            name: "Charcoal & clay",
            swatches: [hex(0x1f1e1b), hex(0x2a2824), hex(0xc4855a), hex(0xece6da)],
        },
    ]
}

pub fn theme(palette: usize) -> Theme {
    let mut theme = base();
    theme.palette = palette;
    let c = &mut theme.colors;
    let line = match palette {
        1 => {
            c.bg = hex(0xe9ebe6);
            c.surface = hex(0xf6f7f4);
            c.surface_2 = hex(0xeff1ec);
            c.text = hex(0x23261f);
            c.muted = hex(0x5f665a);
            c.accent = hex(0x4f6b4a);
            c.accent_bg = hex(0x4f6b4a);
            c.accent_text = hex(0xf6f7f4);
            c.thumb = gradient(135., 0xd7dccf, 0xbfc7b5);
            c.title_bg = hex(0xdfe3da);
            c.title_text = hex(0x23261f);
            // tokens.json gives Stone & moss none of these, which left them in Linen's beige.
            c.surface_sel = step(c, SELECTED);
            c.input_bg = mix(c.surface, gpui::white(), INPUT);
            c.hair = step(c, HAIR);
            step(c, LINE)
        }
        2 => {
            c.bg = hex(0x1f1e1b);
            c.surface = hex(0x2a2824);
            c.surface_2 = hex(0x24231f);
            c.text = hex(0xece6da);
            c.muted = hex(0xa69d8e);
            c.accent = hex(0xc4855a);
            c.accent_bg = hex(0xc4855a);
            c.accent_text = hex(0x1f1e1b);
            c.thumb = gradient(135., 0x3a372f, 0x2e2b25);
            c.title_bg = hex(0x161512);
            c.title_text = hex(0xece6da);
            // tokens.json keeps the light `--surface-sel` (#ece4d6) here, where muted text drops to
            // 2.1:1. A lifted charcoal keeps it at 4.8:1 and text at 10.3:1 (DESIGN.md §7).
            c.surface_sel = hex(0x35322b);
            c.input_bg = hex(0x1a1916);
            c.hair = hex(0x3d3a33);
            hex(0x4a463e)
        }
        _ => return theme,
    };
    follow_linen(&mut theme, line);
    if palette == 1 {
        // Stone & moss's muted colour (#5f665a) is 4.4:1 on its tag over a black picture; two
        // levels darker holds 4.5:1 over any picture (DESIGN.md §7).
        theme.colors.tag_text = hex(0x5d6458);
    }
    if palette == 2 {
        mockup_tag(&mut theme.colors);
    }
    follow_palette(&mut theme.colors);
    theme
}

/// The Palettes mockup draws Charcoal & clay's tag as Linen & walnut's light label, 9:1 off this
/// dark thumbnail where its own surface would be 1.2:1. Its text is four levels darker than the
/// mockup's #6b665c (4.4:1 there), to hold 4.5:1 over any picture (DESIGN.md §7).
fn mockup_tag(c: &mut Colors) {
    c.tag_bg = hex_alpha(0xfaf7f1, 0.9);
    c.tag_text = hex(0x676258);
}

/// Linen & walnut's greys as steps from its window towards its text, each within a few levels of
/// its token: selected card #ece4d6, toggle track and canvas #e9e1d3 and #e8e1d4, hairline
/// #d8cfbf, frame #cfc5b3, control line #cdc3b1, toggle edge #b9ad99.
const SELECTED: f32 = 0.03;
const TRACK: f32 = 0.046;
const HAIR: f32 = 0.144;
const FRAME: f32 = 0.198;
const LINE: f32 = 0.209;
const TOGGLE_EDGE: f32 = 0.322;
/// Field: surface half-way to white (#fffdf9). Dot: window half-way to the accent (#b5a48a). Bulk
/// separator: its ink a fifth of the way to its text (#5a564e).
const INPUT: f32 = 0.51;
const DOT: f32 = 0.51;
const BULK_SEP: f32 = 0.21;

fn step(c: &Colors, amount: f32) -> Hsla {
    mix(c.bg, c.text, amount)
}

/// tokens.json gives the other palettes no value for these, which left Linen & walnut's beige and
/// walnut on a stone or charcoal window, so each takes its own colour that holds the same place in
/// Linen's tokens.
///
/// The Palettes mockup draws Charcoal & clay's toast in Linen's ink all the same, which is 1.2:1
/// against its window, so its edge can't be seen; this rule gives 13.4:1.
///
/// The shadows and thumbnail shade suit any palette, and the annotation colour, sticker and
/// overlay are drawn on the picture, so those stay the look's.
fn follow_linen(theme: &mut Theme, line: Hsla) {
    let c = &mut theme.colors;
    c.button_bg = c.surface;
    c.button_text = c.text;
    c.chip_text = c.muted;
    c.tag_bg = Hsla {
        a: 0.9,
        ..c.surface
    };
    c.tag_text = c.muted;
    c.toggle_off = step(c, TRACK);
    c.toggle_knob = c.accent;
    c.dot = mix(c.bg, c.accent, DOT);
    c.toast_bg = c.text;
    c.toast_text = c.surface;
    c.canvas = Fill::Solid(step(c, TRACK));
    c.shot_bg = c.surface;
    c.shot_line = c.hair;
    c.shot_fill = c.surface_2;
    c.bulk_bg = c.text;
    c.bulk_text = c.surface;
    c.bulk_sep = mix(c.bulk_bg, c.bulk_text, BULK_SEP);
    let b = &mut theme.borders;
    b.accent.color = c.accent;
    for border in [
        &mut b.hair,
        &mut b.card,
        &mut b.pop,
        &mut b.earlier_rule,
        &mut b.title_divider,
    ] {
        border.color = c.hair;
    }
    for border in [&mut b.button, &mut b.input, &mut b.chip, &mut b.load] {
        border.color = line;
    }
    b.frame.color = step(c, FRAME);
    b.toggle.color = step(c, TOGGLE_EDGE);
}

/// tokens.json fixes `--focus`, `--focus-halo` and the window control colour at the base palette's
/// values, which fail DESIGN.md §7 on the others (the ring is 1.2:1 on Charcoal & clay). The bulk
/// bar is the text colour, where the ring would be 1:1, so there it takes the bar's text.
fn follow_palette(c: &mut Colors) {
    c.focus = c.text;
    c.bulk_focus = c.bulk_text;
    c.focus_halo = Hsla {
        a: 0.25,
        ..c.accent
    };
    c.controls = c.muted;
    c.follow_accent();
    c.label = c.muted;
    c.earlier_card_bg = c.surface;
}

pub fn custom(palette: &CustomPalette) -> Theme {
    let mut theme = base();
    theme.palette = CUSTOM_PALETTE;
    palette.apply(&mut theme.colors);
    follow_palette(&mut theme.colors);
    let b = &mut theme.borders;
    for border in [&mut b.hair, &mut b.card, &mut b.pop, &mut b.earlier_rule] {
        border.color = palette.hair();
    }
    for border in [
        &mut b.frame,
        &mut b.button,
        &mut b.input,
        &mut b.chip,
        &mut b.toggle,
        &mut b.load,
    ] {
        border.color = palette.line();
    }
    b.accent.color = palette.accent;
    b.title_divider = b.hair;
    theme
}

fn base() -> Theme {
    let shadow_ink = 0x3c321e; // rgba(60,50,30,…)
    Theme {
        look: Look::Japandi,
        palette: 0,
        background: None,
        colors: Colors {
            bg: hex(0xefe9df),
            surface: hex(0xfaf7f1),
            surface_2: hex(0xf4efe6),
            surface_sel: hex(0xece4d6),
            text: hex(0x2b2a26),
            muted: hex(0x6b665c),
            label: hex(0x6b665c),
            accent: hex(0x7a5c3e),
            accent_bg: hex(0x7a5c3e),
            accent_text: hex(0xfaf7f1),
            hair: hex(0xd8cfbf),
            button_bg: hex(0xfaf7f1),
            button_text: hex(0x2b2a26),
            thumb: gradient(135., 0xe6dccb, 0xd3c6b0),
            thumb_alt: vec![],
            thumb_shade: hex_alpha(0x2b2a26, 0.08),
            earlier_card_bg: hex(0xfaf7f1),
            tag_bg: hex_alpha(0xfaf7f1, 0.9),
            // tokens.json uses `--tag-text` #6b665c, 4.3:1 on the tag over a black picture, which
            // shows through it. Four levels darker holds 4.5:1 over any picture (DESIGN.md §7).
            tag_text: hex(0x676258),
            input_bg: hex(0xfffdf9),
            chip_bg: gpui::transparent_black(),
            chip_text: hex(0x6b665c),
            toggle_off: hex(0xe9e1d3),
            toggle_knob: hex(0x7a5c3e),
            toggle_on: hex(0x7a5c3e),
            toggle_on_knob: hex(0xfaf7f1),
            on_bg: hex(0x7a5c3e),
            on_text: hex(0xfaf7f1),
            chip_saved_bg: hex(0x7a5c3e),
            chip_saved_text: hex(0xfaf7f1),
            title_bg: hex(0xe9e2d5),
            title_text: hex(0x2b2a26),
            dot: hex(0xb5a48a),
            // tokens.json uses `--muted` (#6b665c), 4.43:1 on `--title-bg`. One step darker reaches
            // 4.57:1 (DESIGN.md §7).
            controls: hex(0x69645a),
            toast_bg: hex(0x2b2a26),
            toast_text: hex(0xfaf7f1),
            canvas: Fill::Solid(hex(0xe8e1d4)),
            shot_bg: hex(0xfaf7f1),
            shot_line: hex(0xd8cfbf),
            shot_fill: hex(0xf4efe6),
            bulk_bg: hex(0x2b2a26),
            bulk_text: hex(0xfaf7f1),
            bulk_sep: hex(0x5a564e),
            bulk_main_bg: hex(0x7a5c3e),
            bulk_main_text: hex(0xfaf7f1),
            focus: hex(0x2b2a26),
            bulk_focus: hex(0xfaf7f1),
            focus_halo: hex_alpha(0x7a5c3e, 0.25),
            sticker: hex(0xc4855a),
            sticker_line: hex(0x2b2a26),
            anno: hex(0x7a5c3e),
            warning: hex(0xc8553d),
            swatch_on: hex(0x7a5c3e),
            load_bg: gpui::transparent_black(),
        },
        borders: Borders {
            accent: Border::solid(1., hex(0x7a5c3e)),
            hair: Border::solid(1., hex(0xd8cfbf)),
            frame: Border::solid(1., hex(0xcfc5b3)),
            button: Border::solid(1., hex(0xcdc3b1)),
            card: Border::solid(1., hex(0xd8cfbf)),
            input: Border::solid(1., hex(0xcdc3b1)),
            chip: Border::solid(1., hex(0xcdc3b1)),
            toggle: Border::solid(1., hex(0xb9ad99)),
            toast: Border::none(),
            pop: Border::solid(1., hex(0xd8cfbf)),
            load: Border::dashed(1., hex(0xcdc3b1)),
            earlier_rule: Border::solid(1., hex(0xd8cfbf)),
            thumb: Border::none(),
            title_divider: Border::solid(1., hex(0xd8cfbf)),
        },
        radii: Radii {
            frame: px(10.),
            sm: px(4.),
            md: px(6.),
            lg: px(10.),
            pill: px(999.),
            dot: px(999.),
        },
        shadows: Shadows {
            accent: vec![],
            button: vec![],
            card: vec![soft_shadow(1., 2., hex_alpha(shadow_ink, 0.06))],
            pop: vec![soft_shadow(8., 24., hex_alpha(shadow_ink, 0.14))],
            shot: vec![soft_shadow(10., 30., hex_alpha(shadow_ink, 0.18))],
        },
        icons: Icons {
            stroke: px(1.6),
            square: false,
            dot: px(14.),
        },
        sizes: Sizes {
            button_sm_padding: px(12.),
            tag_padding_x: px(7.),
            window_control_square: None,
        },
        fonts: Fonts {
            display: "Noto Serif JP".into(),
            body: "Inter".into(),
            mono: super::fonts::MONO.into(),
        },
        type_scale: TypeScale {
            body: body(14., FontWeight::NORMAL),
            hint: body(12., FontWeight::NORMAL),
            label: display(12., FontWeight::NORMAL),
            section: display(17., FontWeight::SEMIBOLD),
            day: display(14., FontWeight::SEMIBOLD),
            day_count: body(12., FontWeight::NORMAL),
            button: body(14., FontWeight::SEMIBOLD),
            button_sm: body(13., FontWeight::SEMIBOLD),
            primary_button: body(14., FontWeight::MEDIUM),
            primary_button_sm: body(13., FontWeight::MEDIUM),
            button_sub: body(12., FontWeight::NORMAL),
            chip: body(11., FontWeight::SEMIBOLD),
            tag: body(11., FontWeight::NORMAL),
            field: mono(13., FontWeight::NORMAL),
            segment: body(13., FontWeight::NORMAL),
            setting: body(14., FontWeight::NORMAL),
            title_bar: body(13., FontWeight::NORMAL),
            controls: body(13., FontWeight::NORMAL),
            toast: body(14., FontWeight::SEMIBOLD),
            jump: body(13., FontWeight::SEMIBOLD),
            load_more: body(13., FontWeight::NORMAL),
            bulk_button: body(14., FontWeight::SEMIBOLD),
            bulk_selected: body(14., FontWeight::BOLD),
            bulk_count: body(11., FontWeight::NORMAL),
            tool_key: body(10., FontWeight::NORMAL),
            option: body(12., FontWeight::NORMAL),
            corner: body(13., FontWeight::NORMAL),
            anno_text: body(16., FontWeight::BOLD),
            anno_number: body(14., FontWeight::BOLD),
            readout: mono(14., FontWeight::SEMIBOLD),
        },
        strings: Strings {
            loading: ("Fetching ", " …"),
            copied: "Copied to clipboard",
            back_to_session: "Back to this session",
            earlier: "Earlier — ",
            earlier_today: "Earlier today",
            session: "This session",
            day_date: DayDate::Long,
        },
        // The same for every palette: their inks and papers all read close.
        overlay: Overlay {
            dim: hex_alpha(0x2b2a26, 0.55),
            edge: Border::solid(1., hex(0xfaf7f1)),
            edge_halo: hex_alpha(0x2b2a26, 0.6),
            loupe: Border::solid(1., hex_alpha(0xfaf7f1, 0.3)),
            crosshair: hex(0xc4855a),
        },
        empty: EmptyState {
            title: "Nothing captured yet",
            ornament: Ornament::Seal,
            framed: false,
            centered: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::contrast::{RING, TEXT, over, ratio};
    use crate::theme::test_support::{Entry, assert_token, css_hex};

    fn entry() -> Entry {
        Entry::new("Japandi", Some("Y2K"), &palettes())
    }

    fn tokens() -> String {
        entry().base()
    }

    fn palette_section(index: usize) -> String {
        entry().palette(index)
    }

    #[test]
    fn base_values_match_tokens() {
        let tokens = tokens();
        let theme = theme(0);
        assert_token(&tokens, "--bg", &css_hex(theme.colors.bg));
        assert_token(&tokens, "--accent", &css_hex(theme.colors.accent));
        assert_token(&tokens, "--muted", &css_hex(theme.colors.muted));
        assert_token(&tokens, "--input-bg", &css_hex(theme.colors.input_bg));
        assert_token(&tokens, "--toggle-knob", &css_hex(theme.colors.toggle_knob));
        assert_token(
            &tokens,
            "--btn-border",
            &format!(
                "{}px solid {}",
                f32::from(theme.borders.button.width),
                css_hex(theme.borders.button.color)
            ),
        );
        assert_token(
            &tokens,
            "--load-border",
            &format!("1px dashed {}", css_hex(theme.borders.load.color)),
        );
        assert_token(
            &tokens,
            "--radius",
            &format!("{}px", f32::from(theme.radii.md)),
        );
        let Fill::Gradient { angle, from, to } = theme.colors.thumb else {
            panic!("thumb is a gradient");
        };
        assert_token(
            &tokens,
            "--thumb",
            &format!(
                "linear-gradient({angle}deg,{},{})",
                css_hex(from),
                css_hex(to)
            ),
        );
        assert_token(&tokens, "--toast-border", "0");
        assert_eq!(theme.borders.toast.width, px(0.));
        assert_token(&tokens, "session", theme.strings.session);
        assert_token(&tokens, "toast", theme.strings.copied);
    }

    #[test]
    fn palettes_match_tokens() {
        for (index, palette) in palettes().iter().enumerate() {
            let section = palette_section(index);
            let theme = theme(index);
            assert_token(&section, "--bg", &css_hex(theme.colors.bg));
            assert_token(&section, "--surface", &css_hex(theme.colors.surface));
            assert_token(&section, "--accent-bg", &css_hex(theme.colors.accent_bg));
            assert_token(
                &section,
                "--accent-text",
                &css_hex(theme.colors.accent_text),
            );
            assert_token(&section, "--title-bg", &css_hex(theme.colors.title_bg));
            assert_eq!(css_hex(palette.swatches[0]), css_hex(theme.colors.bg));
            assert_eq!(css_hex(palette.swatches[2]), css_hex(theme.colors.accent));
            let c = &theme.colors;
            assert_eq!(css_hex(c.toggle_on), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.toggle_on_knob), css_hex(c.accent_text));
            assert_eq!(css_hex(c.on_bg), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.chip_saved_text), css_hex(c.accent_text));
            assert_eq!(css_hex(c.bulk_main_bg), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.swatch_on), css_hex(c.accent));
            // Japandi's CSS has no rule for these, so they are what `.label` and `.card` give.
            assert_eq!(css_hex(c.label), css_hex(c.muted));
            assert_eq!(css_hex(c.earlier_card_bg), css_hex(c.surface));
        }
        let dark = theme(2);
        let section = palette_section(2);
        assert_token(&section, "--hair-c", &css_hex(dark.colors.hair));
        assert_token(
            &section,
            "--input-border",
            &format!("1px solid {}", css_hex(dark.borders.input.color)),
        );
        let solid = |border: Border| format!("1px solid {}", css_hex(border.color));
        let (c, b) = (&dark.colors, &dark.borders);
        assert_token(&section, "--input-bg", &css_hex(c.input_bg));
        assert_token(&section, "--btn-bg", &css_hex(c.button_bg));
        assert_token(&section, "--btn-text", &css_hex(c.button_text));
        assert_token(&section, "--chip-text", &css_hex(c.chip_text));
        assert_token(&section, "--hair", &solid(b.hair));
        assert_token(&section, "--card-border", &solid(b.card));
        assert_token(&section, "--btn-border", &solid(b.button));
        assert_token(&section, "--chip-border", &solid(b.chip));
        assert_token(
            &tokens(),
            "--accent-border",
            &solid(theme(0).borders.accent),
        );
    }

    fn followed(theme: &Theme) -> Vec<(&'static str, Hsla)> {
        let (c, b) = (&theme.colors, &theme.borders);
        let Fill::Solid(canvas) = c.canvas else {
            panic!("canvas is flat");
        };
        vec![
            ("surface_sel", c.surface_sel),
            ("input_bg", c.input_bg),
            ("hair", c.hair),
            ("button_bg", c.button_bg),
            ("button_text", c.button_text),
            ("chip_text", c.chip_text),
            ("tag_bg", c.tag_bg),
            ("tag_text", c.tag_text),
            ("toggle_off", c.toggle_off),
            ("toggle_knob", c.toggle_knob),
            ("dot", c.dot),
            ("toast_bg", c.toast_bg),
            ("toast_text", c.toast_text),
            ("canvas", canvas),
            ("shot_bg", c.shot_bg),
            ("shot_line", c.shot_line),
            ("shot_fill", c.shot_fill),
            ("bulk_bg", c.bulk_bg),
            ("bulk_text", c.bulk_text),
            ("bulk_sep", c.bulk_sep),
            ("accent border", b.accent.color),
            ("hair border", b.hair.color),
            ("card border", b.card.color),
            ("pop border", b.pop.color),
            ("earlier rule", b.earlier_rule.color),
            ("title divider", b.title_divider.color),
            ("button border", b.button.color),
            ("input border", b.input.color),
            ("chip border", b.chip.color),
            ("load border", b.load.color),
            ("frame", b.frame.color),
            ("toggle border", b.toggle.color),
        ]
    }

    /// Exact where a token repeats another of Linen's colours; within a few levels for its warm
    /// greys, which the rules take as steps from the window to the text.
    #[test]
    fn the_rules_for_the_other_palettes_give_linen_its_own_tokens() {
        let linen = theme(0);
        let mut derived = base();
        let c = &mut derived.colors;
        c.surface_sel = step(c, SELECTED);
        c.input_bg = mix(c.surface, gpui::white(), INPUT);
        c.hair = step(c, HAIR);
        let line = step(c, LINE);
        follow_linen(&mut derived, line);
        let greys = [
            // Darkened a little from the token, to read over any picture.
            "tag_text",
            "surface_sel",
            "input_bg",
            "hair",
            "toggle_off",
            "dot",
            "canvas",
            "shot_line",
            "bulk_sep",
            "hair border",
            "card border",
            "pop border",
            "earlier rule",
            "title divider",
            "button border",
            "input border",
            "chip border",
            "load border",
            "frame",
            "toggle border",
        ];
        for ((name, token), (_, rule)) in followed(&linen).into_iter().zip(followed(&derived)) {
            assert_eq!(token.a, rule.a, "{name}");
            if greys.contains(&name) {
                let (token, rule) = (gpui::Rgba::from(token), gpui::Rgba::from(rule));
                for (a, b) in [(token.r, rule.r), (token.g, rule.g), (token.b, rule.b)] {
                    assert!((a - b).abs() * 255. <= 10.5, "{name}: {token:?} {rule:?}");
                }
            } else {
                assert_eq!(css_hex(token), css_hex(rule), "{name}");
            }
        }
    }

    #[test]
    fn the_other_palettes_keep_none_of_linens_colours() {
        let linen: Vec<String> = followed(&theme(0))
            .into_iter()
            .map(|(_, color)| css_hex(color))
            .collect();
        for index in 1..3 {
            let theme = theme(index);
            // Charcoal & clay's tag is Linen's on purpose: `charcoal_keeps_the_mockups_tag`.
            let mockup_tag = |name: &str| index == 2 && matches!(name, "tag_bg" | "tag_text");
            for (name, color) in followed(&theme) {
                assert!(
                    mockup_tag(name) || !linen.contains(&css_hex(color)),
                    "{index}: {name}"
                );
            }
            let (c, b) = (&theme.colors, &theme.borders);
            assert_eq!(css_hex(c.toggle_knob), css_hex(c.accent));
            assert_eq!(css_hex(b.accent.color), css_hex(c.accent));
            assert_eq!(css_hex(c.toast_bg), css_hex(c.text));
            assert_eq!(css_hex(c.toast_text), css_hex(c.surface));
            assert_eq!(css_hex(c.bulk_bg), css_hex(c.text));
            assert_eq!(css_hex(c.bulk_text), css_hex(c.surface));
            if index == 1 {
                assert_eq!(css_hex(c.tag_bg), css_hex(c.surface));
                assert_eq!(css_hex(c.tag_text), "#5d6458");
            }
            for hairline in [b.hair, b.card, b.pop, b.earlier_rule, b.title_divider] {
                assert_eq!(css_hex(hairline.color), css_hex(c.hair));
            }
            for edge in [b.input, b.chip, b.load] {
                assert_eq!(css_hex(edge.color), css_hex(b.button.color));
            }
        }
    }

    #[test]
    fn deliberate_departures_from_tokens() {
        let tokens = tokens();
        assert!(tokens.contains(".titlebar .win span{color:#6b665c}"));
        assert_eq!(css_hex(theme(0).colors.controls), "#69645a");
        assert_token(&tokens, "--focus", "#2b2a26");
        assert_token(&tokens, "--focus-halo", "rgba(122,92,62,.25)");
        let linen = theme(0).colors;
        assert_eq!(css_hex(linen.focus), "#2b2a26");
        assert_eq!(css_hex(linen.focus_halo), "#7a5c3e");
        assert_eq!(linen.focus_halo.a, 0.25);
        for index in 1..3 {
            let c = theme(index).colors;
            assert_eq!(css_hex(c.controls), css_hex(c.muted));
            assert_eq!(css_hex(c.focus), css_hex(c.text));
            assert_eq!(css_hex(c.focus_halo), css_hex(c.accent));
            assert_eq!(c.focus_halo.a, 0.25);
        }
        // `--focus` is `--bulk-bg` itself on Linen, so the ring on the bulk bar is its text.
        assert_token(&tokens, "--bulk-bg", "#2b2a26");
        for index in 0..3 {
            let c = theme(index).colors;
            assert_eq!(css_hex(c.bulk_focus), css_hex(c.bulk_text));
        }
        assert_token(&tokens, "--surface-sel", "#ece4d6");
        assert!(!palette_section(2).contains("--surface-sel"));
        assert_eq!(css_hex(theme(2).colors.surface_sel), "#35322b");
    }

    /// The Palettes mockup draws these with Linen & walnut's tokens.
    fn linen_tokens_charcoal_leaves_unset() -> Colors {
        let section = palette_section(2);
        for token in ["--tag-", "--toast-", "--bulk-", "--pop-"] {
            assert!(!section.contains(token), "{token}");
        }
        let tokens = tokens();
        assert_token(&tokens, "--tag-bg", "rgba(250,247,241,.9)");
        assert_token(&tokens, "--tag-text", "#6b665c");
        assert_token(&tokens, "--toast-bg", "#2b2a26");
        assert_token(&tokens, "--toast-border", "0");
        assert_token(&tokens, "--bulk-bg", "#2b2a26");
        // Linen's own tag text is darkened from the token; the mockup draws the token.
        let mut linen = theme(0).colors;
        linen.tag_text = hex(0x6b665c);
        linen
    }

    #[test]
    fn charcoal_keeps_the_mockups_tag() {
        let (linen, clay) = (linen_tokens_charcoal_leaves_unset(), theme(2).colors);
        assert_eq!(clay.tag_bg, linen.tag_bg);
        assert_eq!(css_hex(linen.tag_text), "#6b665c");
        assert_eq!(css_hex(clay.tag_text), "#676258");
        let Fill::Gradient { from, to, .. } = clay.thumb else {
            panic!("thumb is a gradient");
        };
        for thumb in [from, to] {
            let tag = over(clay.tag_bg, thumb);
            assert!(ratio(tag, thumb) >= RING);
            assert!(ratio(over(clay.surface.opacity(0.9), thumb), thumb) < RING);
            assert!(ratio(linen.tag_text, tag) < TEXT);
        }
        for picture in [from, to, gpui::black(), gpui::white()] {
            assert!(ratio(clay.tag_text, over(clay.tag_bg, picture)) >= TEXT);
        }
    }

    #[test]
    fn charcoal_does_not_keep_the_mockups_toast() {
        let (linen, clay) = (linen_tokens_charcoal_leaves_unset(), theme(2).colors);
        assert_eq!(theme(2).borders.toast.width, px(0.));
        for under in [clay.bg, clay.surface, clay.surface_sel] {
            assert!(ratio(linen.toast_bg, under) < 1.2);
            assert!(ratio(clay.toast_bg, under) >= 10.);
            assert!(ratio(clay.bulk_bg, under) >= 10.);
        }
        assert!(ratio(linen.toast_bg, linen.bg) >= 10.);
    }

    #[test]
    fn token_slices_stop_at_the_next_section() {
        let clay_bg = "\"--bg\": \"#1f1e1b\"";
        assert!(!tokens().contains(clay_bg));
        assert!(!palette_section(1).contains(clay_bg));
        assert!(palette_section(2).contains(clay_bg));
        assert!(!palette_section(2).contains("\"Y2K\""));
        assert!(!palette_section(0).contains("\"--bg\": \"#e9ebe6\""));
    }
}
