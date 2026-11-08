//! The NeoBrutalism look, copied from the "NeoBrutalism" entry of `design/tokens.json`: black
//! 3px borders, hard black shadows, flat blocks of colour and uppercase labels.

use gpui::FontWeight;
use toru_core::settings::Look;

use super::{
    Border, Borders, CUSTOM_PALETTE, Colors, CustomPalette, DayDate, Fill, Fonts, Icons, Overlay,
    Palette, Radii, Shadows, Sizes, Strings, Theme, TypeScale,
    css::{body, display, grid, hard_shadow, hex, hex_alpha, mono},
};

pub fn palettes() -> [Palette; 3] {
    [
        Palette {
            name: "Cream / yellow / mint / coral",
            swatches: [hex(0xf3efe4), hex(0xffde59), hex(0x7cf5c2), hex(0xff6b6b)],
        },
        Palette {
            name: "Paper / blue / lime / orange",
            swatches: [hex(0xfffdf5), hex(0x4d7cff), hex(0xc6ff5c), hex(0xff9f1c)],
        },
        Palette {
            name: "Black / acid / white",
            swatches: [hex(0x121212), hex(0xd4ff00), hex(0xff4fd8), hex(0xffffff)],
        },
    ]
}

pub fn theme(palette: usize) -> Theme {
    let mut theme = base();
    theme.palette = palette;
    let c = &mut theme.colors;
    let b = &mut theme.borders;
    let s = &mut theme.shadows;
    let o = &mut theme.overlay;
    match palette {
        1 => {
            c.bg = hex(0xfffdf5);
            c.surface_2 = hex(0xfffdf5);
            c.accent_bg = hex(0x4d7cff);
            c.title_bg = hex(0xff9f1c);
            c.thumb = Fill::Solid(hex(0xc6ff5c));
            c.toast_bg = hex(0xc6ff5c);
            // tokens.json puts white text on the blue (#4d7cff): 3.7:1 on the primary button, and
            // 1.3:1 and 1.9:1 where `.chip.saved` and `.bulk button.main` keep their mint and
            // yellow. Black, as the other two palettes use, is 5.6:1 or better on all three. The
            // blue selected card (#4d7cff) drops the hint text to 3.4:1; a pale tint of the same
            // blue keeps it at 10.6:1 behind the 3px outline, and is light enough for the blue
            // focus ring to hold 3.1:1 on it, as on the name field of a selected card (DESIGN.md
            // §7).
            c.accent_text = hex(0x000000);
            c.surface_sel = hex(0xe3ebff);
            c.chip_saved_text = c.accent_text;
            c.bulk_main_text = c.accent_text;
        }
        2 => {
            let white = hex(0xffffff);
            c.bg = hex(0x121212);
            c.surface = hex(0x1e1e1e);
            c.surface_2 = hex(0x121212);
            c.text = white;
            c.muted = hex(0xcfcfcf);
            c.accent = white;
            c.accent_bg = hex(0xd4ff00);
            c.accent_text = hex(0x000000);
            c.hair = white;
            c.button_bg = hex(0x1e1e1e);
            c.button_text = white;
            c.thumb = Fill::Solid(hex(0xff4fd8));
            c.tag_bg = white;
            c.tag_text = hex(0x000000);
            c.input_bg = hex(0x1e1e1e);
            c.chip_bg = hex(0x1e1e1e);
            c.chip_text = white;
            c.toggle_off = hex(0x1e1e1e);
            c.toggle_knob = white;
            c.title_bg = hex(0xd4ff00);
            c.title_text = hex(0x000000);
            c.controls = c.title_text;
            c.toast_bg = hex(0xd4ff00);
            c.canvas = grid(0x1e1e1e, 0x444444, 32.);
            c.shot_bg = hex(0x121212);
            c.shot_line = white;
            c.shot_fill = hex(0x1e1e1e);
            c.bulk_bg = white;
            c.bulk_text = hex(0x000000);
            c.bulk_sep = hex(0x999999);
            c.load_bg = hex(0x1e1e1e);
            // The look's `extra_css` fixes these at the light palettes' black and cream, which
            // vanish here: `.label{color:#000}` on #121212 is 1.1:1, `.card.dim{background:
            // #f3efe4}` would make earlier cards cream, and `.sw.on{outline-color:#000}` is a
            // black outline on a black panel. They follow the text, the background and the accent
            // instead. `--surface-sel:#d4ff00` under the white text is 1.1:1; a deep tint of the
            // acid keeps the selected card readable at 13:1 (DESIGN.md §7).
            c.label = c.text;
            c.earlier_card_bg = c.bg;
            c.swatch_on = c.accent;
            c.surface_sel = hex(0x2c3300);
            // `.seg button.on{background:#000}` is 1.3:1 on this palette's #1e1e1e track, with white
            // text either way, so a chosen segment couldn't be told apart. It uses the acid instead.
            c.on_bg = hex(0xd4ff00);
            c.on_text = hex(0x000000);
            b.accent = Border::solid(3., white);
            b.frame = Border::solid(4., white);
            for border in [
                &mut b.hair,
                &mut b.button,
                &mut b.card,
                &mut b.input,
                &mut b.toggle,
                &mut b.toast,
                &mut b.pop,
                &mut b.earlier_rule,
            ] {
                *border = Border::solid(3., white);
            }
            // `.thumb{border:3px solid #000}` is pinned for the look, so `b.thumb` stays black.
            b.chip = Border::solid(2., white);
            o.edge_halo = white;
            o.loupe = Border::solid(3., white);
            b.load = Border::dashed(3., white);
            for shadow in [
                &mut s.accent,
                &mut s.button,
                &mut s.card,
                &mut s.pop,
                &mut s.shot,
            ] {
                shadow[0].color = white;
            }
        }
        _ => {}
    }
    theme
}

/// NeoBrutalism in the user's own colours: the derived colours, with the look's heavy borders
/// and hard shadows in the text colour, as the Black / acid / white palette draws them in white.
pub fn custom(palette: &CustomPalette) -> Theme {
    let mut theme = base();
    theme.palette = CUSTOM_PALETTE;
    palette.apply(&mut theme.colors);
    let c = &mut theme.colors;
    c.label = c.text;
    c.hair = c.text;
    c.tag_bg = c.text;
    c.tag_text = c.bg;
    let b = &mut theme.borders;
    for border in [
        &mut b.accent,
        &mut b.hair,
        &mut b.frame,
        &mut b.button,
        &mut b.card,
        &mut b.input,
        &mut b.chip,
        &mut b.toggle,
        &mut b.toast,
        &mut b.pop,
        &mut b.load,
        &mut b.earlier_rule,
        &mut b.thumb,
        &mut b.title_divider,
    ] {
        border.color = palette.text;
    }
    let s = &mut theme.shadows;
    for list in [
        &mut s.accent,
        &mut s.button,
        &mut s.card,
        &mut s.pop,
        &mut s.shot,
    ] {
        for shadow in list.iter_mut() {
            shadow.color = palette.text;
        }
    }
    theme.overlay.edge_halo = palette.text;
    theme.overlay.loupe.color = palette.text;
    theme
}

fn base() -> Theme {
    let black = hex(0x000000);
    let white = hex(0xffffff);
    let cream = hex(0xf3efe4);
    Theme {
        look: Look::NeoBrutalism,
        palette: 0,
        background: None,
        colors: Colors {
            bg: cream,
            surface: white,
            surface_2: cream,
            surface_sel: white,
            text: black,
            muted: hex(0x333333),
            label: black,
            accent: black,
            accent_bg: hex(0xffde59),
            accent_text: black,
            hair: black,
            button_bg: white,
            button_text: black,
            thumb: Fill::Solid(hex(0x7cf5c2)),
            // `.card:nth-child(3n+2)` and `:nth-child(3n)` thumbs, in every palette.
            thumb_alt: vec![Fill::Solid(hex(0xffde59)), Fill::Solid(hex(0xff6b6b))],
            thumb_shade: gpui::transparent_black(),
            earlier_card_bg: cream,
            tag_bg: black,
            tag_text: white,
            input_bg: white,
            chip_bg: white,
            chip_text: black,
            toggle_off: white,
            toggle_knob: black,
            // The on-states are pinned in the look's `extra_css`, so they don't follow the accent.
            toggle_on: hex(0x7cf5c2),
            toggle_on_knob: black,
            on_bg: black,
            on_text: white,
            chip_saved_bg: hex(0x7cf5c2),
            chip_saved_text: black,
            title_bg: hex(0xff6b6b),
            title_text: black,
            dot: black,
            controls: black,
            toast_bg: hex(0x7cf5c2),
            toast_text: black,
            canvas: grid(0xcfe8ff, 0x7fb3e0, 32.),
            shot_bg: white,
            shot_line: black,
            shot_fill: cream,
            bulk_bg: black,
            bulk_text: white,
            bulk_sep: hex(0x555555),
            bulk_main_bg: hex(0xffde59),
            bulk_main_text: black,
            focus: hex(0x4d7cff),
            // 5.6:1 on the black bar and 3.7:1 on the dark palette's white one.
            bulk_focus: hex(0x4d7cff),
            focus_halo: gpui::transparent_black(),
            sticker: hex(0xffde59),
            sticker_line: black,
            anno: black,
            swatch_on: black,
            load_bg: white,
        },
        borders: Borders {
            accent: Border::solid(3., black),
            hair: Border::solid(3., black),
            frame: Border::solid(4., black),
            button: Border::solid(3., black),
            card: Border::solid(3., black),
            input: Border::solid(3., black),
            chip: Border::solid(2., black),
            toggle: Border::solid(3., black),
            toast: Border::solid(3., black),
            pop: Border::solid(3., black),
            load: Border::dashed(3., black),
            earlier_rule: Border::solid(3., black),
            thumb: Border::solid(3., black),
            // `.titlebar .win span{border-left:3px solid #000}` is pinned for the look.
            title_divider: Border::solid(3., black),
        },
        radii: Radii {
            frame: gpui::px(0.),
            sm: gpui::px(0.),
            md: gpui::px(0.),
            lg: gpui::px(0.),
            pill: gpui::px(0.),
            dot: gpui::px(0.),
        },
        shadows: Shadows {
            accent: vec![hard_shadow(4., 4., black)],
            button: vec![hard_shadow(4., 4., black)],
            card: vec![hard_shadow(6., 6., black)],
            pop: vec![hard_shadow(6., 6., black)],
            shot: vec![hard_shadow(10., 10., black)],
        },
        icons: Icons {
            stroke: gpui::px(2.5),
            square: true,
            dot: gpui::px(16.),
        },
        sizes: Sizes {
            button_sm_padding: gpui::px(9.),
            tag_padding_x: gpui::px(7.),
            window_control_square: None,
        },
        fonts: Fonts {
            display: "Archivo Black".into(),
            body: "Space Grotesk".into(),
            mono: "Space Grotesk".into(),
        },
        // Space Grotesk ships at 500 and 700 here, so the CSS's 400 and 600 land on those. Archivo
        // Black has one weight, asked for as normal so nothing is synthesised on top of it.
        type_scale: TypeScale {
            body: body(15., FontWeight::MEDIUM),
            hint: body(12., FontWeight::MEDIUM),
            label: display(12., FontWeight::NORMAL).upper(),
            section: display(18., FontWeight::NORMAL).upper(),
            day: display(14., FontWeight::NORMAL).upper(),
            day_count: body(12., FontWeight::MEDIUM),
            button: body(14., FontWeight::BOLD).upper(),
            button_sm: body(12., FontWeight::BOLD).upper(),
            primary_button: body(14., FontWeight::BOLD).upper(),
            primary_button_sm: body(12., FontWeight::BOLD).upper(),
            button_sub: body(12., FontWeight::MEDIUM).upper(),
            chip: body(11., FontWeight::BOLD).upper(),
            tag: body(11., FontWeight::MEDIUM),
            field: mono(13., FontWeight::MEDIUM),
            segment: body(13., FontWeight::MEDIUM),
            setting: body(15., FontWeight::MEDIUM),
            title_bar: display(13., FontWeight::NORMAL).upper(),
            controls: display(13., FontWeight::NORMAL),
            toast: display(13., FontWeight::NORMAL).upper(),
            jump: display(13., FontWeight::NORMAL).upper(),
            load_more: body(13., FontWeight::BOLD),
            bulk_button: body(14., FontWeight::BOLD),
            bulk_selected: body(15., FontWeight::BOLD),
            bulk_count: body(11., FontWeight::MEDIUM),
            tool_key: body(10., FontWeight::MEDIUM),
            option: body(12., FontWeight::MEDIUM),
            corner: body(13., FontWeight::MEDIUM),
            anno_text: display(16., FontWeight::NORMAL).upper(),
            anno_number: body(14., FontWeight::BOLD),
            readout: mono(15., FontWeight::BOLD),
        },
        strings: Strings {
            loading: ("LOADING ", " →"),
            copied: "COPIED.",
            back_to_session: "BACK TO THIS SESSION",
            earlier: "EARLIER — ",
            earlier_today: "EARLIER TODAY",
            session: "THIS SESSION",
            day_date: DayDate::LongUpper,
        },
        // The look's yellow block in a black 3px line; the dark palette turns the halo and loupe
        // white with its other borders.
        overlay: Overlay {
            dim: hex_alpha(0x000000, 0.6),
            edge: Border::solid(3., hex(0xffde59)),
            edge_halo: black,
            loupe: Border::solid(3., black),
            crosshair: hex(0x4d7cff),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::test_support::{Entry, assert_token, css_border, css_hex};
    use gpui::px;

    fn entry() -> Entry {
        Entry::new("NeoBrutalism", None, &palettes())
    }

    #[test]
    fn base_values_match_tokens() {
        let tokens = entry().base();
        let theme = theme(0);
        let c = &theme.colors;
        assert_token(&tokens, "--bg", &css_hex(c.bg));
        assert_token(&tokens, "--surface", &css_hex(c.surface));
        assert_token(&tokens, "--muted", "#333");
        assert_eq!(css_hex(c.muted), "#333333");
        assert_token(&tokens, "--accent-bg", &css_hex(c.accent_bg));
        assert_token(&tokens, "--title-bg", &css_hex(c.title_bg));
        assert_token(&tokens, "--toast-bg", &css_hex(c.toast_bg));
        assert_token(&tokens, "--focus", &css_hex(c.focus));
        assert_token(&tokens, "--focus-halo", "transparent");
        assert_eq!(c.focus_halo.a, 0.);
        assert_token(&tokens, "--bulk-sep", "#555");
        assert_eq!(css_hex(c.bulk_sep), "#555555");
        let Fill::Solid(thumb) = c.thumb else {
            panic!("thumb is flat");
        };
        assert_token(&tokens, "--thumb", &css_hex(thumb));
        assert_token(&tokens, "--accent-border", "3px solid #000");
        assert_eq!(css_border(theme.borders.accent), "3px solid #000000");
        assert_eq!(css_border(theme.borders.chip), "2px solid #000000");
        assert_eq!(css_border(theme.borders.load), "3px dashed #000000");
        assert_eq!(css_border(theme.borders.thumb), "3px solid #000000");
        assert_token(&tokens, "--radius-pill", "0");
        assert_eq!(theme.radii.pill, px(0.));
        assert_token(&tokens, "--card-shadow", "6px 6px 0 #000");
        assert_eq!(theme.shadows.card[0].offset, gpui::point(px(6.), px(6.)));
        assert_eq!(theme.shadows.card[0].blur_radius, px(0.));
        assert_eq!(theme.sizes.button_sm_padding, px(9.));
        assert!(tokens.contains(".btn.sm{padding:0 9px;font-size:12px}"));
        assert_token(&tokens, "toast", theme.strings.copied);
        assert_token(&tokens, "jump", theme.strings.back_to_session);
        assert_token(&tokens, "session", theme.strings.session);
    }

    /// `extra_css` pins these for the whole look, so no palette moves them.
    #[test]
    fn extra_css_on_states_hold_in_every_palette() {
        let tokens = entry().base();
        assert!(tokens.contains(".chip.saved{background:#7cf5c2}"));
        assert!(
            tokens.contains(".toggle.on{background:#7cf5c2}.toggle.on::after{background:#000}")
        );
        assert!(tokens.contains(".seg button.on{background:#000;color:#fff}"));
        assert!(tokens.contains(".bulk button.main{background:#ffde59}"));
        assert!(tokens.contains(".titlebar .dot{width:16px;height:16px;background:#000}"));
        assert!(tokens.contains(".titlebar .win span{font-weight:900;border-left:3px solid #000}"));
        assert!(tokens.contains(".card:nth-child(3n+2) .thumb{background:#ffde59}"));
        assert!(tokens.contains(".thumb{border:3px solid #000}"));
        for index in 0..3 {
            let theme = theme(index);
            let c = &theme.colors;
            assert_eq!(css_hex(c.chip_saved_bg), "#7cf5c2");
            assert_eq!(css_hex(c.toggle_on), "#7cf5c2");
            assert_eq!(css_hex(c.toggle_on_knob), "#000000");
            assert_eq!(css_hex(c.bulk_main_bg), "#ffde59");
            assert_eq!(css_hex(c.dot), "#000000");
            assert_eq!(theme.icons.dot, px(16.));
            assert_eq!(css_border(theme.borders.title_divider), "3px solid #000000");
            assert_eq!(c.thumb_alt.len(), 2);
            assert_eq!(css_hex(c.focus), "#4d7cff");
            assert_eq!(css_hex(c.bulk_focus), "#4d7cff");
            assert_eq!(css_border(theme.borders.thumb), "3px solid #000000");
        }
    }

    #[test]
    fn palettes_match_tokens() {
        let entry = entry();
        let paper = theme(1);
        let section = entry.palette(1);
        assert_token(&section, "--bg", &css_hex(paper.colors.bg));
        assert_token(&section, "--accent-bg", &css_hex(paper.colors.accent_bg));
        assert_token(&section, "--title-bg", &css_hex(paper.colors.title_bg));
        assert_token(&section, "--toast-bg", &css_hex(paper.colors.toast_bg));
        // Paper keeps the base surface, text, borders and shadows.
        assert_eq!(css_hex(paper.colors.surface), "#ffffff");
        assert_eq!(css_hex(paper.colors.text), "#000000");
        assert_eq!(css_border(paper.borders.card), "3px solid #000000");

        let dark = theme(2);
        let section = entry.palette(2);
        let c = &dark.colors;
        assert_token(&section, "--bg", &css_hex(c.bg));
        assert_token(&section, "--surface", &css_hex(c.surface));
        assert_token(&section, "--muted", &css_hex(c.muted));
        assert_token(&section, "--accent-bg", &css_hex(c.accent_bg));
        assert_token(&section, "--btn-bg", &css_hex(c.button_bg));
        assert_token(&section, "--title-bg", &css_hex(c.title_bg));
        assert_token(&section, "--bulk-sep", "#999");
        assert_eq!(css_hex(c.bulk_sep), "#999999");
        assert_token(&section, "--load-bg", &css_hex(c.load_bg));
        assert_token(&section, "--accent-border", "3px solid #fff");
        assert_eq!(css_border(dark.borders.accent), "3px solid #ffffff");
        assert_eq!(css_border(dark.borders.chip), "2px solid #ffffff");
        assert_eq!(css_border(dark.borders.load), "3px dashed #ffffff");
        assert_eq!(css_border(dark.borders.frame), "4px solid #ffffff");
        assert_token(&section, "--card-shadow", "6px 6px 0 #fff");
        for shadow in [&dark.shadows.accent, &dark.shadows.card, &dark.shadows.pop] {
            assert_eq!(css_hex(shadow[0].color), "#ffffff");
        }
        let Fill::Grid { bg, line, cell } = c.canvas else {
            panic!("canvas is a grid");
        };
        assert_eq!(
            (css_hex(bg), css_hex(line), cell),
            ("#1e1e1e".into(), "#444444".into(), px(32.))
        );
        for (index, palette) in palettes().iter().enumerate() {
            let c = theme(index).colors;
            assert_eq!(css_hex(palette.swatches[0]), css_hex(c.bg));
            assert_eq!(css_hex(palette.swatches[1]), css_hex(c.accent_bg));
        }
    }

    /// Where the theme leaves tokens.json on purpose, for DESIGN.md §7 contrast. Each departure is
    /// recorded against the token it replaces, so a change to either side shows up here.
    #[test]
    fn deliberate_departures_from_tokens() {
        let entry = entry();
        let paper = entry.palette(1);
        assert_token(&paper, "--accent-text", "#fff");
        assert_token(&paper, "--surface-sel", "#4d7cff");
        let c = theme(1).colors;
        assert_eq!(css_hex(c.accent_text), "#000000");
        assert_eq!(css_hex(c.chip_saved_text), "#000000");
        assert_eq!(css_hex(c.bulk_main_text), "#000000");
        assert_eq!(css_hex(c.surface_sel), "#e3ebff");

        let dark = entry.palette(2);
        assert_token(&dark, "--surface-sel", "#d4ff00");
        let base = entry.base();
        assert!(base.contains(".label{color:#000}"));
        assert!(base.contains(".card.dim{background:#f3efe4}"));
        assert!(base.contains(".sw.on{outline-color:#000}"));
        assert!(base.contains(".seg button.on{background:#000;color:#fff}"));
        let c = theme(2).colors;
        assert_eq!(css_hex(c.on_bg), "#d4ff00");
        assert_eq!(css_hex(c.on_text), "#000000");
        assert_eq!(css_hex(c.surface_sel), "#2c3300");
        assert_eq!(css_hex(c.label), css_hex(c.text));
        assert_eq!(css_hex(c.earlier_card_bg), css_hex(c.bg));
        assert_eq!(css_hex(c.swatch_on), css_hex(c.accent));
        // The light palettes keep the pinned values.
        for index in 0..2 {
            let c = theme(index).colors;
            assert_eq!(css_hex(c.label), "#000000");
            assert_eq!(css_hex(c.on_bg), "#000000");
            assert_eq!(css_hex(c.on_text), "#ffffff");
            assert_eq!(css_hex(c.earlier_card_bg), "#f3efe4");
            assert_eq!(css_hex(c.swatch_on), "#000000");
        }
        assert_eq!(css_hex(theme(0).colors.surface_sel), "#ffffff");
    }

    #[test]
    fn token_slices_stop_at_the_next_section() {
        let entry = entry();
        let dark_bg = "\"--bg\": \"#121212\"";
        assert!(!entry.base().contains(dark_bg));
        assert!(!entry.palette(1).contains(dark_bg));
        assert!(entry.palette(2).contains(dark_bg));
        assert!(!entry.palette(0).contains("\"--bg\": \"#fffdf5\""));
    }
}
