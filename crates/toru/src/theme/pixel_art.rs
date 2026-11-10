//! The PixelArt look, copied from the "PixelArt" entry of `design/tokens.json`. Everything is
//! square, bordered in near-black and lifted by a hard 4px shadow.

use gpui::{FontWeight, Hsla};
use toru_core::settings::Look;

use super::{
    Border, Borders, CUSTOM_PALETTE, Colors, CustomPalette, DayDate, EmptyState, Fill, Fonts,
    Icons, Ornament, Overlay, Palette, Radii, Shadows, Sizes, Strings, Theme, TypeScale,
    css::{body, checker, display, hard_shadow, hex, hex_alpha, inset_shadow, mono},
    custom::mix,
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
            c.hair = hex(0x0a2a0a);
            theme.borders.accent.color = hex(0x0f380f);
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
            c.hair = hex(0x140c04);
            theme.borders.accent.color = hex(0x140c04);
        }
        // Night arcade is the base as it stands.
        _ => return theme,
    }
    follow_palette(&mut theme.colors);
    follow_night_arcade(&mut theme);
    outline(&mut theme);
    theme
}

/// PixelArt's CSS has no rule of its own for these, so they follow the palette's accent, muted
/// and surface, as the base CSS does.
fn follow_palette(c: &mut Colors) {
    c.follow_accent();
    c.dot = c.accent;
    c.label = c.muted;
    c.earlier_card_bg = c.surface;
}

/// How far the editor canvas's two squares sit from the window towards the surface in Night
/// arcade (`#24243d` and `#1f1f36`, between `#1b1b2f` and `#2a2a45`), and the card's inner bevel
/// (`#1c1c30`) from the surface back towards the window.
const CANVAS: (f32, f32) = (0.62, 0.2975);
const BEVEL: f32 = 0.95;
/// The ink's strength where it dims the screen behind the capture overlay.
const DIM: f32 = 0.6;

/// tokens.json gives the other palettes no value for these, which left them in Night arcade's
/// navy and purple on a green or amber window. Each takes the colour of its own palette that
/// holds the same place in Night arcade's tokens: there the chip's text is the button's, the
/// window controls are the title's yellow, the bulk bar is ink under the text colour with the
/// button fill between its groups, the placeholder frame and the rule above a day are dashed in
/// the button fill, and the placeholder is filled like an input.
///
/// The focus ring stays the look's mint. It is drawn where a selected card's accent outline is,
/// so it can't be a palette's accent, and it is checked against every palette's surfaces. The
/// annotation pink, the sticker and the overlay's crosshair are drawn on the picture, not on
/// the window, and stay the look's too.
fn follow_night_arcade(theme: &mut Theme) {
    let c = &mut theme.colors;
    let ink = c.hair;
    c.chip_text = c.button_text;
    c.controls = c.title_text;
    c.bulk_bg = ink;
    c.bulk_text = c.text;
    c.bulk_sep = c.button_bg;
    c.load_bg = c.input_bg;
    if let Fill::Checker { a, b, .. } = &mut c.canvas {
        *a = mix(c.bg, c.surface, CANVAS.0);
        *b = mix(c.bg, c.surface, CANVAS.1);
    }
    // Nothing draws the mockups' stand-in screenshot, whose colours these are; they keep to
    // the palette all the same.
    c.shot_bg = c.surface;
    c.shot_line = c.button_bg;
    c.shot_fill = c.surface_2;
    theme.borders.load.color = c.button_bg;
    theme.borders.earlier_rule.color = c.button_bg;
    theme.overlay.dim = Hsla { a: DIM, ..ink };
    theme.overlay.edge_halo = ink;
    theme.overlay.loupe.color = ink;
}

/// The look outlines everything in one ink, `colors.hair`, and drops its hard shadows in it.
/// The card's inner bevel is its surface sunk towards the window, and the selection edge is the
/// palette's accent on its ink, like the title bar.
fn outline(theme: &mut Theme) {
    let c = &theme.colors;
    let ink = c.hair;
    let b = &mut theme.borders;
    for border in [
        &mut b.hair,
        &mut b.frame,
        &mut b.button,
        &mut b.card,
        &mut b.input,
        &mut b.chip,
        &mut b.toggle,
        &mut b.toast,
        &mut b.pop,
    ] {
        border.color = ink;
    }
    b.title_divider = b.hair;
    let s = &mut theme.shadows;
    for list in [
        &mut s.accent,
        &mut s.button,
        &mut s.card,
        &mut s.pop,
        &mut s.shot,
    ] {
        for shadow in list.iter_mut().filter(|shadow| !shadow.inset) {
            shadow.color = ink;
        }
    }
    // The button's bevel is black and white laid over its fill, so it suits any palette; the
    // card's is a colour of its own.
    for shadow in s.card.iter_mut().filter(|shadow| shadow.inset) {
        shadow.color = mix(c.surface, c.bg, BEVEL);
    }
    theme.overlay.edge.color = c.accent_bg;
}

/// PixelArt in the user's own colours. The look outlines everything in one ink; here that ink
/// is the text colour, so the outlines show on the user's surfaces whatever their tone, and the
/// hard drop shadows follow it. The placeholder frame and the day rule are dashed in it too,
/// since the user's surfaces may sit too close together to show a dash in one of them. The
/// button's bevel keeps the look's own highlights.
pub fn custom(palette: &CustomPalette) -> Theme {
    let mut theme = base();
    theme.palette = CUSTOM_PALETTE;
    palette.apply(&mut theme.colors);
    follow_palette(&mut theme.colors);
    theme.colors.hair = palette.text;
    outline(&mut theme);
    let b = &mut theme.borders;
    for border in [&mut b.accent, &mut b.load, &mut b.earlier_rule] {
        border.color = palette.text;
    }
    theme
}

fn base() -> Theme {
    let ink = hex(0x0d0d18);
    Theme {
        look: Look::PixelArt,
        palette: 0,
        background: None,
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
            title_divider: Border::solid(3., ink),
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
            tag_padding_x: gpui::px(6.),
            // `.titlebar .win span::before{width:10px;height:10px;background:currentColor}`
            window_control_square: Some(gpui::px(10.)),
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
            // tokens.json sets every `.btn.primary` in the display face at 11px, beside which
            // the body face's 17px and 13px labels look a size smaller: Press Start 2P's letters
            // fill most of its em, VT323's about half. At 10px and 8px the two faces' x-heights
            // match, so a row of buttons reads as one size; 8px is also the display face's own
            // pixel grid, so the card's Save is crisp.
            primary_button: display(10., FontWeight::NORMAL),
            primary_button_sm: display(8., FontWeight::NORMAL),
            // tokens.json leaves the base CSS's 12px here, 11px on the bulk bar's counts and
            // sets `.tool .k` to 8px. In VT323, whose letters are about half its em, those are
            // too small to read, so the two body sizes go to the chip's 14px and the tool's
            // letter to the display face at 8px, its own pixel grid.
            button_sub: body(14., FontWeight::NORMAL),
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
            bulk_count: body(14., FontWeight::NORMAL),
            tool_key: display(8., FontWeight::NORMAL),
            option: body(17., FontWeight::NORMAL),
            corner: body(17., FontWeight::NORMAL),
            anno_text: display(11., FontWeight::NORMAL),
            anno_number: body(14., FontWeight::NORMAL),
            readout: mono(20., FontWeight::NORMAL),
        },
        strings: Strings {
            loading: ("LOADING ", " ▮▮▮▯▯▯"),
            copied: "★ COPIED! ★",
            back_to_session: "BACK TO SESSION",
            earlier: "EARLIER — ",
            earlier_today: "EARLIER TODAY",
            session: "THIS SESSION",
            day_date: DayDate::ShortUpper,
        },
        // Thick ink around everything, as the look's controls have; `theme` swaps in each
        // palette's accent for the edge. The crosshair is the annotation pink, which no palette's
        // screen is likely to be.
        overlay: Overlay {
            dim: hex_alpha(0x0d0d18, 0.6),
            edge: Border::solid(3., hex(0xffd23f)),
            edge_halo: ink,
            loupe: Border::solid(3., ink),
            crosshair: hex(0xff6b97),
        },
        // The loading row's dashed frame around the Region icon on a tile, like an empty save
        // slot showing what goes in it.
        empty: EmptyState {
            title: "NO SHOTS YET",
            ornament: Ornament::Tile,
            framed: true,
            centered: true,
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
            // Fixed in the tokens, whatever the palette.
            assert_eq!(css_hex(c.focus), "#4fe3a3");
            assert_eq!(css_hex(c.bulk_focus), "#4fe3a3");
            // `.titlebar .win span{border-left:var(--hair)}`, and the squares in place of glyphs.
            let theme = theme(index);
            assert_eq!(
                css_border(theme.borders.title_divider),
                css_border(theme.borders.hair)
            );
            assert_eq!(theme.sizes.window_control_square, Some(px(10.)));
        }
    }

    /// Every colour `follow_night_arcade` and `outline` set, by name.
    fn followed(theme: &Theme) -> Vec<(&'static str, String)> {
        let (c, b, s, o) = (
            &theme.colors,
            &theme.borders,
            &theme.shadows,
            &theme.overlay,
        );
        let Fill::Checker { a, b: other, .. } = &c.canvas else {
            panic!("canvas is a checker");
        };
        let mut colours = vec![
            ("chip_text", c.chip_text),
            ("controls", c.controls),
            ("bulk_bg", c.bulk_bg),
            ("bulk_text", c.bulk_text),
            ("bulk_sep", c.bulk_sep),
            ("load_bg", c.load_bg),
            ("canvas", *a),
            ("canvas", *other),
            ("shot_bg", c.shot_bg),
            ("dim", o.dim),
            ("edge", o.edge.color),
            ("edge_halo", o.edge_halo),
            ("loupe", o.loupe.color),
        ];
        let borders = [
            b.hair,
            b.frame,
            b.button,
            b.card,
            b.input,
            b.chip,
            b.toggle,
            b.toast,
            b.pop,
            b.load,
            b.earlier_rule,
            b.title_divider,
        ];
        colours.extend(borders.map(|border| ("border", border.color)));
        let shadows = [&s.accent, &s.button, &s.card, &s.pop, &s.shot];
        colours.extend(
            shadows
                .into_iter()
                .flatten()
                .map(|shadow| ("shadow", shadow.color)),
        );
        colours
            .into_iter()
            .map(|(name, color)| (name, format!("{} at {}", css_hex(color), color.a)))
            .collect()
    }

    /// What the tokens leave unset in the other palettes is taken from each palette's own
    /// colours the way Night arcade's tokens relate: asked of Night arcade's colours, the
    /// rules give its tokens back.
    #[test]
    fn the_rules_for_the_other_palettes_give_night_arcade_its_own_tokens() {
        let mut derived = base();
        follow_palette(&mut derived.colors);
        follow_night_arcade(&mut derived);
        outline(&mut derived);
        assert_eq!(followed(&derived), followed(&theme(0)));
        let tokens = entry().base();
        assert_token(&tokens, "--load-border", &css_border(derived.borders.load));
        assert_token(&tokens, "--pop-border", &css_border(derived.borders.pop));
        assert_token(&tokens, "--bulk-sep", &css_hex(derived.colors.bulk_sep));
        assert_token(&tokens, "--load-bg", &css_hex(derived.colors.load_bg));
    }

    /// The other palettes show none of Night arcade's navy, purple or yellow where the tokens
    /// give them no value of their own: each is that palette's ink, text, button fill, input
    /// fill or title colour.
    #[test]
    fn the_other_palettes_keep_none_of_night_arcades_colours() {
        let night: Vec<String> = followed(&theme(0))
            .into_iter()
            .map(|(_, color)| color)
            .collect();
        for index in 1..3 {
            let theme = theme(index);
            for (name, color) in followed(&theme) {
                // The button's bevel is black and white, in every palette.
                let bevel = color.starts_with("#000000") || color.starts_with("#ffffff");
                assert!(bevel || !night.contains(&color), "{index}: {name} {color}");
            }
            let (c, b) = (&theme.colors, &theme.borders);
            assert_eq!(css_hex(c.chip_text), css_hex(c.text));
            assert_eq!(css_hex(c.controls), css_hex(c.title_text));
            assert_eq!(css_hex(c.bulk_bg), css_hex(c.hair));
            assert_eq!(css_hex(c.bulk_text), css_hex(c.text));
            assert_eq!(css_hex(c.bulk_sep), css_hex(c.button_bg));
            assert_eq!(css_hex(c.load_bg), css_hex(c.input_bg));
            for dashed in [b.load, b.earlier_rule] {
                assert!(dashed.dashed);
                assert_eq!(css_hex(dashed.color), css_hex(c.button_bg));
            }
            for inked in [b.frame, b.toast, b.pop, b.card] {
                assert_eq!(css_hex(inked.color), css_hex(c.hair));
            }
            assert_eq!(css_hex(theme.shadows.pop[0].color), css_hex(c.hair));
        }
        // A custom palette's card has no navy bevel either: it is the user's surface sunk
        // towards their window.
        let linen = CustomPalette::from_theme(&Theme::new(Look::Japandi, 0));
        let custom = custom(&linen);
        let bevel = &custom.shadows.card[1];
        assert!(bevel.inset);
        assert_eq!(css_hex(bevel.color), "#f0eae0");
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
        // `extra_css` writes Night arcade's yellow into the window controls for every palette.
        // The others' squares are their own title colour, as Night arcade's are.
        let base = entry().base();
        assert!(base.contains(".titlebar .win span{color:#ffd23f}"));
        assert_token(&base, "--title-text", "#ffd23f");
        assert_eq!(css_hex(theme(1).colors.controls), "#9bbc0f");
        assert_eq!(css_hex(theme(2).colors.controls), "#ff9f1c");
        // The tokens' 11px primary buttons read a size larger than the body face's labels
        // beside them, so the theme sets them where the two faces' x-heights match.
        assert!(entry().base().contains(
            ".btn.primary,.toggle.on,.tab.on{font-family:var(--display);font-size:11px}"
        ));
        let scale = theme(0).type_scale;
        assert_eq!(scale.primary_button.size, px(10.));
        assert_eq!(scale.primary_button_sm.size, px(8.));
        // The tokens' smallest sizes can't be read in VT323: the tool's letter keeps its 8px in
        // the display face, and a button's sub and the bulk bar's counts take the chip's size.
        assert!(entry().base().contains(".tool .k{font-size:8px}"));
        assert_eq!(scale.tool_key.family, crate::theme::Family::Display);
        assert_eq!(scale.tool_key.size, px(8.));
        assert_eq!(scale.button_sub.size, scale.chip.size);
        assert_eq!(scale.bulk_count.size, scale.chip.size);
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
