//! The PixelArt look, copied from the "PixelArt" entry of `design/tokens.json`. Everything is
//! square, bordered in near-black and lifted by a hard 4px shadow.

use gpui::FontWeight;
use toru_core::settings::Look;

use super::{
    Border, Borders, Colors, DayDate, Fonts, Icons, Palette, Radii, Shadows, Sizes, Strings, Theme,
    TypeScale,
    css::{body, checker, display, hard_shadow, hex, hex_alpha, inset_shadow, mono},
};

pub fn palettes() -> [Palette; 3] {
    [
        Palette {
            name: "Night arcade",
            swatches: [hex(0x1b1b2f), hex(0x2a2a45), hex(0xffd23f), hex(0x4fe3a3)],
        },
        Palette {
            name: "Game Boy green",
            swatches: [hex(0x0f380f), hex(0x306230), hex(0x9bbc0f), hex(0xe0f8d0)],
        },
        Palette {
            name: "CRT amber",
            swatches: [hex(0x2b1d0e), hex(0x3d2a14), hex(0xff9f1c), hex(0xffe6b3)],
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
            c.bg = hex(0x0f380f);
            c.surface = hex(0x306230);
            c.surface_2 = hex(0x1e4a1e);
            c.text = hex(0xe0f8d0);
            c.accent = hex(0x9bbc0f);
            c.accent_bg = hex(0x9bbc0f);
            c.accent_text = hex(0x0f380f);
            c.thumb = checker(0x8bac0f, 0x306230, 16.);
            c.title_bg = hex(0x0a2a0a);
            c.title_text = hex(0x9bbc0f);
            c.button_bg = hex(0x306230);
            c.button_text = hex(0xe0f8d0);
            c.input_bg = hex(0x0f380f);
            c.chip_bg = hex(0x306230);
            c.toast_bg = hex(0x9bbc0f);
            c.toast_text = hex(0x0f380f);
            c.tag_bg = hex(0x0f380f);
            c.tag_text = hex(0x9bbc0f);
            c.toggle_off = hex(0x306230);
            c.toggle_knob = hex(0xe0f8d0);
            // tokens.json uses the accent (#9bbc0f) for `--muted` too, which is 3.3:1 on the
            // surface, where card hints sit. One step lighter on the Game Boy ramp reaches 4.7:1,
            // and the lifted `--surface-sel` (#4a7a3a) is traded for `--surface-2` so hints on a
            // selected card hold 6.6:1 (DESIGN.md §7).
            c.muted = hex(0xc0dc3f);
            c.surface_sel = hex(0x1e4a1e);
            follow_palette(c);
            c.hair = hex(0x0a2a0a);
            b.accent = Border::solid(3., hex(0x0f380f));
            for border in [
                &mut b.hair,
                &mut b.button,
                &mut b.card,
                &mut b.input,
                &mut b.toggle,
            ] {
                *border = Border::solid(3., hex(0x0a2a0a));
            }
            b.chip = Border::solid(2., hex(0x0a2a0a));
        }
        2 => {
            c.bg = hex(0x2b1d0e);
            c.surface = hex(0x3d2a14);
            c.surface_2 = hex(0x33230f);
            c.surface_sel = hex(0x5a3d1c);
            c.text = hex(0xffe6b3);
            c.muted = hex(0xd9a85c);
            c.accent = hex(0xff9f1c);
            c.accent_bg = hex(0xff9f1c);
            c.accent_text = hex(0x2b1d0e);
            c.thumb = checker(0x4d3519, 0x3d2a14, 16.);
            c.title_bg = hex(0x140c04);
            c.title_text = hex(0xff9f1c);
            c.button_bg = hex(0x5a3d1c);
            c.button_text = hex(0xffe6b3);
            c.input_bg = hex(0x1f1408);
            c.chip_bg = hex(0x5a3d1c);
            c.toast_bg = hex(0xffd166);
            c.toast_text = hex(0x2b1d0e);
            c.tag_bg = hex(0x140c04);
            c.tag_text = hex(0xff9f1c);
            c.toggle_off = hex(0x5a3d1c);
            c.toggle_knob = hex(0xffe6b3);
            follow_palette(c);
            c.hair = hex(0x140c04);
            for border in [
                &mut b.accent,
                &mut b.hair,
                &mut b.button,
                &mut b.card,
                &mut b.input,
                &mut b.toggle,
            ] {
                *border = Border::solid(3., hex(0x140c04));
            }
            b.chip = Border::solid(2., hex(0x140c04));
        }
        _ => {}
    }
    theme
}

/// PixelArt's CSS has no rule of its own for these, so they follow the palette's accent, muted
/// and surface, as the base CSS does. The focus ring and the window controls are fixed in the
/// tokens and stay so.
fn follow_palette(c: &mut Colors) {
    c.follow_accent();
    c.dot = c.accent;
    c.label = c.muted;
    c.earlier_card_bg = c.surface;
}

fn base() -> Theme {
    let ink = hex(0x0d0d18);
    Theme {
        look: Look::PixelArt,
        palette: 0,
        colors: Colors {
            bg: hex(0x1b1b2f),
            surface: hex(0x2a2a45),
            surface_2: hex(0x22223a),
            surface_sel: hex(0x3a3a5e),
            text: hex(0xf1f1e6),
            muted: hex(0xa9a9c9),
            label: hex(0xa9a9c9),
            accent: hex(0xffd23f),
            accent_bg: hex(0xffd23f),
            accent_text: hex(0x1b1b2f),
            hair: ink,
            button_bg: hex(0x4a4a7a),
            button_text: hex(0xf1f1e6),
            thumb: checker(0x3d3d66, 0x34345a, 16.),
            thumb_alt: vec![],
            thumb_shade: gpui::transparent_black(),
            earlier_card_bg: hex(0x2a2a45),
            tag_bg: ink,
            tag_text: hex(0xffd23f),
            input_bg: hex(0x15152a),
            chip_bg: hex(0x4a4a7a),
            chip_text: hex(0xf1f1e6),
            toggle_off: hex(0x4a4a7a),
            toggle_knob: hex(0xf1f1e6),
            toggle_on: hex(0xffd23f),
            toggle_on_knob: hex(0x1b1b2f),
            on_bg: hex(0xffd23f),
            on_text: hex(0x1b1b2f),
            chip_saved_bg: hex(0xffd23f),
            chip_saved_text: hex(0x1b1b2f),
            title_bg: ink,
            title_text: hex(0xffd23f),
            dot: hex(0xffd23f),
            controls: hex(0xffd23f),
            toast_bg: hex(0x4fe3a3),
            toast_text: hex(0x1b1b2f),
            canvas: checker(0x24243d, 0x1f1f36, 24.),
            shot_bg: hex(0x2a2a45),
            shot_line: hex(0x5a5a8a),
            shot_fill: hex(0x333355),
            bulk_bg: ink,
            bulk_text: hex(0xf1f1e6),
            bulk_sep: hex(0x4a4a7a),
            bulk_main_bg: hex(0xffd23f),
            bulk_main_text: hex(0x1b1b2f),
            focus: hex(0x4fe3a3),
            bulk_focus: hex(0x4fe3a3),
            focus_halo: gpui::transparent_black(),
            sticker: hex(0xffd23f),
            sticker_line: ink,
            anno: hex(0xff6b97),
            swatch_on: hex(0xffd23f),
            load_bg: hex(0x15152a),
        },
        borders: Borders {
            accent: Border::solid(3., hex(0x1b1b2f)),
            hair: Border::solid(3., ink),
            frame: Border::solid(4., ink),
            button: Border::solid(3., ink),
            card: Border::solid(3., ink),
            input: Border::solid(3., ink),
            chip: Border::solid(2., ink),
            toggle: Border::solid(3., ink),
            toast: Border::solid(3., ink),
            pop: Border::solid(3., ink),
            load: Border::dashed(3., hex(0x4a4a7a)),
            earlier_rule: Border::dashed(3., hex(0x4a4a7a)),
            thumb: Border::none(),
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
            accent: vec![hard_shadow(4., 4., ink)],
            // The bevel from `extra_css`: a dark band bottom-right and a light one top-left.
            button: vec![
                hard_shadow(4., 4., ink),
                inset_shadow(-3., -3., hex_alpha(0x000000, 0.35)),
                inset_shadow(3., 3., hex_alpha(0xffffff, 0.12)),
            ],
            card: vec![
                hard_shadow(4., 4., ink),
                inset_shadow(-3., -3., hex(0x1c1c30)),
            ],
            pop: vec![hard_shadow(6., 6., ink)],
            shot: vec![hard_shadow(8., 8., ink)],
        },
        icons: Icons {
            stroke: gpui::px(2.5),
            square: true,
            dot: gpui::px(14.),
        },
        sizes: Sizes {
            button_sm_padding: gpui::px(12.),
        },
        fonts: Fonts {
            display: "Press Start 2P".into(),
            body: "VT323".into(),
            mono: "VT323".into(),
        },
        // Both faces come in one weight, so nothing asks for a synthesised bold. VT323 is a small
        // face, so the body sizes are larger than the other looks'.
        type_scale: TypeScale {
            body: body(18., FontWeight::NORMAL),
            hint: body(15., FontWeight::NORMAL),
            label: display(10., FontWeight::NORMAL).upper(),
            section: display(12., FontWeight::NORMAL),
            day: display(10., FontWeight::NORMAL),
            day_count: body(17., FontWeight::NORMAL),
            button: body(17., FontWeight::NORMAL),
            button_sm: body(13., FontWeight::NORMAL),
            primary_button: display(11., FontWeight::NORMAL),
            primary_button_sm: display(11., FontWeight::NORMAL),
            button_sub: body(12., FontWeight::NORMAL),
            chip: body(14., FontWeight::NORMAL),
            tag: body(13., FontWeight::NORMAL),
            field: mono(17., FontWeight::NORMAL),
            segment: body(17., FontWeight::NORMAL),
            setting: body(16., FontWeight::NORMAL),
            title_bar: display(10., FontWeight::NORMAL),
            controls: display(10., FontWeight::NORMAL),
            toast: display(10., FontWeight::NORMAL),
            jump: display(10., FontWeight::NORMAL),
            load_more: display(10., FontWeight::NORMAL),
            bulk_button: body(17., FontWeight::NORMAL),
            bulk_selected: body(18., FontWeight::NORMAL),
            bulk_count: body(11., FontWeight::NORMAL),
            tool_key: body(8., FontWeight::NORMAL),
            option: body(17., FontWeight::NORMAL),
            corner: body(17., FontWeight::NORMAL),
            anno_text: display(11., FontWeight::NORMAL),
            anno_number: body(14., FontWeight::NORMAL),
        },
        strings: Strings {
            loading: ("LOADING ", " ▮▮▮▯▯▯"),
            copied: "★ COPIED! ★",
            back_to_session: "BACK TO SESSION",
            earlier: "EARLIER — ",
            session: "THIS SESSION",
            day_date: DayDate::ShortUpper,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{
        Fill,
        test_support::{Entry, assert_token, css_border, css_hex},
    };
    use gpui::px;

    fn entry() -> Entry {
        Entry::new("PixelArt", Some("Gothic"), &palettes())
    }

    #[test]
    fn base_values_match_tokens() {
        let tokens = entry().base();
        let theme = theme(0);
        let c = &theme.colors;
        assert_token(&tokens, "--bg", &css_hex(c.bg));
        assert_token(&tokens, "--surface-sel", &css_hex(c.surface_sel));
        assert_token(&tokens, "--muted", &css_hex(c.muted));
        assert_token(&tokens, "--btn-bg", &css_hex(c.button_bg));
        assert_token(&tokens, "--input-bg", &css_hex(c.input_bg));
        assert_token(&tokens, "--chip-bg", &css_hex(c.chip_bg));
        assert_token(&tokens, "--toast-bg", &css_hex(c.toast_bg));
        assert_token(&tokens, "--focus", &css_hex(c.focus));
        assert_token(&tokens, "--focus-halo", "transparent");
        assert_eq!(c.focus_halo.a, 0.);
        assert_token(&tokens, "--anno", &css_hex(c.anno));
        assert_token(
            &tokens,
            "--accent-border",
            &css_border(theme.borders.accent),
        );
        assert_token(&tokens, "--btn-border", &css_border(theme.borders.button));
        assert_token(&tokens, "--chip-border", &css_border(theme.borders.chip));
        assert_token(&tokens, "--load-border", &css_border(theme.borders.load));
        assert_token(
            &tokens,
            "--earlier-rule",
            &css_border(theme.borders.earlier_rule),
        );
        assert_token(&tokens, "--radius", "0");
        assert_eq!(theme.radii.md, px(0.));
        assert_eq!(theme.radii.pill, px(0.));
        let Fill::Checker { a, b, cell } = &c.thumb else {
            panic!("thumb is a checker");
        };
        assert_token(
            &tokens,
            "--thumb",
            &format!(
                "repeating-conic-gradient({} 0 25%,{} 0 50%) 0 0/{tile}px {tile}px",
                css_hex(*a),
                css_hex(*b),
                tile = f32::from(*cell) * 2.
            ),
        );
        assert_token(&tokens, "--accent-shadow", "4px 4px 0 #0d0d18");
        assert_eq!(theme.shadows.accent.len(), 1);
        assert_eq!(theme.shadows.accent[0].offset, gpui::point(px(4.), px(4.)));
        assert_eq!(theme.shadows.accent[0].blur_radius, px(0.));
        assert!(!theme.shadows.accent[0].inset);
        assert!(theme.shadows.button[1].inset && theme.shadows.button[2].inset);
        assert_token(&tokens, "toast", theme.strings.copied);
        assert_token(&tokens, "jump", theme.strings.back_to_session);
        assert_token(&tokens, "session", theme.strings.session);
    }

    #[test]
    fn palettes_match_tokens() {
        let entry = entry();
        for (index, palette) in palettes().iter().enumerate().skip(1) {
            let section = entry.palette(index);
            let theme = theme(index);
            let c = &theme.colors;
            assert_token(&section, "--bg", &css_hex(c.bg));
            assert_token(&section, "--surface", &css_hex(c.surface));
            assert_token(&section, "--surface-2", &css_hex(c.surface_2));
            assert_token(&section, "--text", &css_hex(c.text));
            assert_token(&section, "--accent-bg", &css_hex(c.accent_bg));
            assert_token(&section, "--accent-text", &css_hex(c.accent_text));
            assert_token(&section, "--title-bg", &css_hex(c.title_bg));
            assert_token(&section, "--btn-bg", &css_hex(c.button_bg));
            assert_token(&section, "--input-bg", &css_hex(c.input_bg));
            assert_token(&section, "--toast-bg", &css_hex(c.toast_bg));
            assert_token(&section, "--toggle-knob", &css_hex(c.toggle_knob));
            assert_token(&section, "--hair-c", &css_hex(c.hair));
            assert_token(
                &section,
                "--accent-border",
                &css_border(theme.borders.accent),
            );
            assert_token(&section, "--card-border", &css_border(theme.borders.card));
            assert_token(&section, "--chip-border", &css_border(theme.borders.chip));
            assert_eq!(css_hex(palette.swatches[0]), css_hex(c.bg));
            assert_eq!(css_hex(palette.swatches[2]), css_hex(c.accent));
            assert_eq!(css_hex(palette.swatches[3]), css_hex(c.text));
        }
        for index in 0..3 {
            let c = theme(index).colors;
            // The CSS on-states are the accent, so a palette's accent must reach them.
            assert_eq!(css_hex(c.toggle_on), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.on_text), css_hex(c.accent_text));
            assert_eq!(css_hex(c.chip_saved_bg), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.bulk_main_bg), css_hex(c.accent_bg));
            assert_eq!(css_hex(c.dot), css_hex(c.accent));
            assert_eq!(css_hex(c.label), css_hex(c.muted));
            assert_eq!(css_hex(c.earlier_card_bg), css_hex(c.surface));
            // Fixed in the tokens and the look's `extra_css`, whatever the palette.
            assert_eq!(css_hex(c.focus), "#4fe3a3");
            assert_eq!(css_hex(c.bulk_focus), "#4fe3a3");
            assert_eq!(css_hex(c.controls), "#ffd23f");
        }
        // The palettes override neither, so the base values carry through.
        let base = entry.base();
        assert_token(&base, "--pop-border", &css_border(theme(2).borders.pop));
        assert_token(&base, "--toast-border", &css_border(theme(1).borders.toast));
    }

    /// Where the theme leaves tokens.json on purpose, for DESIGN.md §7 contrast. Each departure is
    /// recorded against the token it replaces, so a change to either side shows up here.
    #[test]
    fn deliberate_departures_from_tokens() {
        let game_boy = entry().palette(1);
        assert_token(&game_boy, "--muted", "#9bbc0f");
        assert_token(&game_boy, "--surface-sel", "#4a7a3a");
        let c = theme(1).colors;
        assert_eq!(css_hex(c.muted), "#c0dc3f");
        assert_eq!(css_hex(c.surface_sel), css_hex(c.surface_2));
        // The other palettes keep their tokens.
        let amber = entry().palette(2);
        assert_token(&amber, "--muted", &css_hex(theme(2).colors.muted));
        assert_token(
            &amber,
            "--surface-sel",
            &css_hex(theme(2).colors.surface_sel),
        );
        assert_token(&entry().base(), "--muted", &css_hex(theme(0).colors.muted));
    }

    #[test]
    fn token_slices_stop_at_the_next_section() {
        let entry = entry();
        let amber_bg = "\"--bg\": \"#2b1d0e\"";
        assert!(!entry.base().contains(amber_bg));
        assert!(!entry.palette(1).contains(amber_bg));
        assert!(entry.palette(2).contains(amber_bg));
        assert!(!entry.palette(2).contains("\"Gothic\""));
    }
}
