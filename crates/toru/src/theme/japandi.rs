//! The Japandi look, copied from the "Japandi" entry of `design/tokens.json`.

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
    // The line around a control, a little stronger than the hairline.
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
            // tokens.json gives Stone & moss no value for these, which left them in Linen's
            // beige: they take Linen's steps from this palette's own colours.
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
        // Linen & walnut is the base as it stands.
        _ => return theme,
    };
    follow_linen(&mut theme, line);
    if palette == 2 {
        mockup_tag(&mut theme.colors);
    }
    follow_palette(&mut theme.colors);
    theme
}

/// The Palettes mockup draws Charcoal & clay's tag as Linen & walnut's light label, which stands
/// 9:1 off this palette's dark thumbnail where its own surface would be 1.2:1. The label is kept
/// as drawn. Its text is four levels darker than the mockup's `--tag-text` (#6b665c), which is
/// 4.4:1 on the label over this thumbnail; #676258 holds 4.5:1 over any picture (DESIGN.md §7).
///
/// The mockup's toast is not kept: see `follow_linen`.
fn mockup_tag(c: &mut Colors) {
    c.tag_bg = hex_alpha(0xfaf7f1, 0.9);
    c.tag_text = hex(0x676258);
}

/// How far Linen & walnut's greys sit from its window towards its text: a selected card
/// (`#ece4d6`), the toggle's track and the editor canvas (`#e9e1d3`, `#e8e1d4`), the hairline
/// (`#d8cfbf`), the frame (`#cfc5b3`), the line around a control (`#cdc3b1`) and the toggle's
/// edge (`#b9ad99`). Each lands within a few levels of Linen's warmer token.
const SELECTED: f32 = 0.03;
const TRACK: f32 = 0.046;
const HAIR: f32 = 0.144;
const FRAME: f32 = 0.198;
const LINE: f32 = 0.209;
const TOGGLE_EDGE: f32 = 0.322;
/// A field is the surface half-way to white (`#fffdf9` over `#faf7f1`), the title bar's dot is
/// the window half-way to the accent (`#b5a48a`), and the bulk bar's separator is its ink a
/// fifth of the way to its text (`#5a564e`).
const INPUT: f32 = 0.51;
const DOT: f32 = 0.51;
const BULK_SEP: f32 = 0.21;

/// `amount` of the way from the window to the text: the same step in any palette, in that
/// palette's own tint.
fn step(c: &Colors, amount: f32) -> Hsla {
    mix(c.bg, c.text, amount)
}

/// tokens.json gives the other palettes no value for these, which left Linen & walnut's beige
/// and walnut on a stone or charcoal window. Each takes the colour of its own palette that holds
/// the same place in Linen's tokens: there a button is the surface under the text, a chip's and
/// a tag's text are the muted colour, the tag is the surface at 90%, the toggle's knob and the
/// primary button's edge are the accent, the toast and the bulk bar are the text colour under
/// the surface colour, and the hairline draws the cards, menus and day rules. Linen's other
/// greys are the same steps from the window to the text.
///
/// The Palettes mockup draws Charcoal & clay's toast in Linen's ink (#2b2a26) all the same. With
/// no border and a shadow that doesn't show on a dark window, that toast is 1.2:1 against the
/// window and 1.0:1 against a card, so its edge can't be seen; Linen's own stands 11.9:1 off its
/// window. Charcoal & clay's toast and bulk bar keep this rule, which gives 13.4:1. Its tag
/// follows the mockup instead: see `mockup_tag`.
///
/// The soft shadows and the thumbnail's shade are faint translucent darks, which suit any
/// palette; the annotation colour, the sticker and the capture overlay are drawn on the
/// picture, not on the window. Those stay the look's.
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

/// Japandi's CSS has no rule of its own for these, so they follow the palette's accent, muted and
/// surface.
///
/// tokens.json fixes `--focus`, `--focus-halo` and the window control colour at the base palette's
/// values, which fail DESIGN.md §7 on the other palettes (the focus ring is 1.2:1 on Charcoal & clay).
/// So the focus ring follows the text, the halo the accent, and the controls the muted colour.
/// The bulk bar is the ink of the text, where that ring is 1:1, so on it the ring is the bar's
/// own light text.
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

/// Japandi in the user's own colours: the derived colours, then this look's palette rules, with
/// every hairline border in the derived hairline and every control edge in the stronger line.
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
            tag_text: hex(0x6b665c),
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
        // Ink over the screen, a paper line with an ink halo around the selection, and the
        // walnut accent for the crosshair, as the palettes' own inks and papers all read close.
        overlay: Overlay {
            dim: hex_alpha(0x2b2a26, 0.55),
            edge: Border::solid(1., hex(0xfaf7f1)),
            edge_halo: hex_alpha(0x2b2a26, 0.6),
            loupe: Border::solid(1., hex_alpha(0xfaf7f1, 0.3)),
            crosshair: hex(0xc4855a),
        },
        // Unframed and centred with air around it: a serif line under a small round seal.
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

    /// The entry's base values, up to the palette overrides.
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
            // The CSS on-states are the accent, so a palette's accent must reach them.
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

    /// Every colour `follow_linen` sets, by name, with the line it is given.
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

    /// What the tokens leave unset in the other palettes is taken from each palette's own
    /// colours the way Linen & walnut's tokens relate. Asked of Linen's colours, the rules give
    /// its tokens back: exactly where a token repeats another of Linen's colours, and within a
    /// few levels where it is one of Linen's warm greys, which the rules take as a step from the
    /// window to the text.
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

    /// The other palettes show none of Linen & walnut's beige or walnut where the tokens give
    /// them no value of their own.
    #[test]
    fn the_other_palettes_keep_none_of_linens_colours() {
        let linen: Vec<String> = followed(&theme(0))
            .into_iter()
            .map(|(_, color)| css_hex(color))
            .collect();
        for index in 1..3 {
            let theme = theme(index);
            // Charcoal & clay's tag is Linen's on purpose: `charcoal_keeps_the_mockups_tag`.
            let mockup_tag = |name: &str| index == 2 && name == "tag_bg";
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
                assert_eq!(css_hex(c.tag_text), css_hex(c.muted));
            }
            for hairline in [b.hair, b.card, b.pop, b.earlier_rule, b.title_divider] {
                assert_eq!(css_hex(hairline.color), css_hex(c.hair));
            }
            for edge in [b.input, b.chip, b.load] {
                assert_eq!(css_hex(edge.color), css_hex(b.button.color));
            }
        }
    }

    /// Where the theme leaves tokens.json on purpose, for DESIGN.md §7 contrast. Each departure is
    /// recorded against the token it replaces, so a change to either side shows up here.
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

    /// tokens.json gives Charcoal & clay no tag, toast or bulk bar of its own, so the Palettes
    /// mockup draws its tag and toast with Linen & walnut's tokens.
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
        theme(0).colors
    }

    /// The mockup's light tag is kept on Charcoal & clay, where it stands off the dark thumbnail
    /// as a control's edge must. Its text is the mockup's grey taken just far enough to read over
    /// any picture, which the mockup's own does not on this palette's thumbnail.
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

    /// The mockup's toast is not kept: Linen's ink has no edge on Charcoal & clay's window or its
    /// cards, where the toast, the return pill and the bulk bar float. The palette's own text
    /// colour stands off both as far as Linen's ink does off Linen's.
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

    /// Each slice stops at the next section, so a value copied from the wrong palette is caught.
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
