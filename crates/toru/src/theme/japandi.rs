//! The Japandi look, copied from the "Japandi" entry of `design/tokens.json`.

use gpui::{FontWeight, Hsla, px};
use toru_core::settings::Look;

use super::{
    Border, Borders, CUSTOM_PALETTE, Colors, CustomPalette, DayDate, EmptyState, Fill, Fonts,
    Icons, Ornament, Overlay, Palette, Radii, Shadows, Sizes, Strings, Theme, TypeScale,
    css::{body, display, gradient, hex, hex_alpha, mono, soft_shadow},
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
    let b = &mut theme.borders;
    match palette {
        1 => {
            c.bg = hex(0xe9ebe6);
            c.surface = hex(0xf6f7f4);
            c.surface_2 = hex(0xeff1ec);
            c.text = hex(0x23261f);
            c.muted = hex(0x5f665a);
            c.accent = hex(0x4f6b4a);
            c.accent_bg = hex(0x4f6b4a);
            c.accent_text = hex(0xf6f7f4);
            follow_palette(c);
            c.thumb = gradient(135., 0xd7dccf, 0xbfc7b5);
            c.title_bg = hex(0xdfe3da);
            c.title_text = hex(0x23261f);
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
            follow_palette(c);
            c.thumb = gradient(135., 0x3a372f, 0x2e2b25);
            c.title_bg = hex(0x161512);
            c.title_text = hex(0xece6da);
            // tokens.json keeps the light `--surface-sel` (#ece4d6) here, where muted text drops to
            // 2.1:1. A lifted charcoal keeps it at 4.8:1 and text at 10.3:1 (DESIGN.md §7).
            c.surface_sel = hex(0x35322b);
            c.input_bg = hex(0x1a1916);
            c.button_bg = hex(0x2a2824);
            c.button_text = hex(0xece6da);
            c.hair = hex(0x3d3a33);
            c.chip_text = hex(0xa69d8e);
            b.card = Border::solid(1., hex(0x3d3a33));
            b.hair = Border::solid(1., hex(0x3d3a33));
            b.input = Border::solid(1., hex(0x4a463e));
            b.button = Border::solid(1., hex(0x4a463e));
            b.chip = Border::solid(1., hex(0x4a463e));
            // tokens.json leaves the light tag (#faf7f1 at .9) with its light-palette `--tag-text`
            // (#6b665c): 4.4:1 over this palette's thumb. The base `--bulk-sep` grey reaches 5.7:1.
            c.tag_text = hex(0x5a564e);
        }
        _ => {}
    }
    theme.borders.title_divider = theme.borders.hair;
    theme
}

/// Japandi's CSS has no rule of its own for these, so they follow the palette's accent, muted and
/// surface. `--accent-border` and `--toggle-knob` are not overridden in the tokens, so they keep
/// the base values.
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
        // Not overridden by any palette, so the base value carries through.
        assert_token(
            &tokens(),
            "--accent-border",
            &format!("1px solid {}", css_hex(dark.borders.accent.color)),
        );
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
        assert_eq!(css_hex(theme(1).colors.surface_sel), "#ece4d6");
        assert_eq!(css_hex(theme(2).colors.surface_sel), "#35322b");
        assert_token(&tokens, "--tag-text", "#6b665c");
        assert!(!palette_section(2).contains("--tag-text"));
        assert_eq!(css_hex(theme(1).colors.tag_text), "#6b665c");
        assert_eq!(css_hex(theme(2).colors.tag_text), "#5a564e");
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
