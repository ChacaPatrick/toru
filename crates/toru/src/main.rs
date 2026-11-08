#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The whole theme is defined now; later sections read the parts the board doesn't show.
#[allow(dead_code)]
mod theme;

use gpui::{
    App, Bounds, Context, Div, Render, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_platform::application;
use toru_core::settings::{BUILT_IN_PALETTES, PaletteChoice, Settings};

use theme::{ActiveTheme, Theme, ThemedText, TypeStyle, install_fonts, set_theme};

/// A board that shows the active theme. The real window arrives in Phase 4.
struct Toru {
    settings: Settings,
}

impl Toru {
    fn next_palette(&mut self, cx: &mut Context<Self>) {
        let look = self.settings.look;
        let next = (cx.theme().palette + 1) % BUILT_IN_PALETTES;
        self.settings.appearance_mut(look).palette = PaletteChoice::BuiltIn(next);
        set_theme(Theme::from_settings(&self.settings), cx);
    }
}

impl Render for Toru {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let text = &theme.type_scale;
        let palette = &Theme::palettes(theme.look)[theme.palette];

        let heading = div()
            .flex()
            .items_baseline()
            .gap_3()
            .child(
                div()
                    .type_style(theme, text.section)
                    .child(text.section.case.apply(theme.strings.session)),
            )
            .child(
                div()
                    .type_style(theme, text.hint)
                    .text_color(colors.muted)
                    .child("3 captures · 1 saved · nothing saved to your folder until you save"),
            );

        let buttons = div()
            .flex()
            .gap_2p5()
            .child(
                sample_button(theme, text.primary_button)
                    .bg(colors.accent_bg)
                    .text_color(colors.accent_text)
                    .border_color(theme.borders.accent.color)
                    .shadow(theme.shadows.accent.clone())
                    .child(text.primary_button.case.apply("Region")),
            )
            .child(
                sample_button(theme, text.button)
                    .bg(colors.button_bg)
                    .text_color(colors.button_text)
                    .border_color(theme.borders.button.color)
                    .shadow(theme.shadows.button.clone())
                    .child(text.button.case.apply("Window")),
            );

        let card = div()
            .w(px(300.))
            .p_2p5()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.surface)
            .border(theme.borders.card.width)
            .border_color(theme.borders.card.color)
            .rounded(theme.radii.lg)
            .shadow(theme.shadows.card.clone())
            .child(
                div()
                    .h(px(140.))
                    .rounded(theme.radii.md)
                    .bg(colors.thumb.background()),
            )
            .child(
                div()
                    .h(px(34.))
                    .px_2p5()
                    .flex()
                    .items_center()
                    .bg(colors.input_bg)
                    .border(theme.borders.input.width)
                    .border_color(theme.borders.input.color)
                    .rounded(theme.radii.md)
                    .type_style(theme, text.field)
                    .child("screenshot_11_07_2026_3"),
            );

        let swatches = div().flex().gap_2().children(palette.swatches.map(|color| {
            div()
                .w(px(64.))
                .h(px(24.))
                .bg(color)
                .border(theme.borders.input.width)
                .border_color(theme.borders.input.color)
                .rounded(theme.radii.sm)
        }));

        div()
            .id("board")
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .bg(colors.bg)
            .text_color(colors.text)
            .type_style(theme, text.body)
            .on_click(cx.listener(|this, _, _, cx| this.next_palette(cx)))
            .child(
                div()
                    .h(px(38.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .bg(colors.title_bg)
                    .text_color(colors.title_text)
                    .type_style(theme, text.title_bar)
                    .border_b(theme.borders.hair.width)
                    .border_color(theme.borders.hair.color)
                    .child(
                        div()
                            .size(theme.icons.dot)
                            .rounded(theme.radii.dot)
                            .bg(colors.dot),
                    )
                    .child(text.title_bar.case.apply("Toru — session started 2:14 PM")),
            )
            .child(heading)
            .child(
                div()
                    .type_style(theme, text.hint)
                    .text_color(colors.muted)
                    .child("Off = clean slate each session. On = earlier shots load below, a day at a time as you scroll."),
            )
            .child(buttons)
            .child(card)
            .child(
                div()
                    .type_style(theme, text.label)
                    .text_color(colors.muted)
                    .child(format!(
                        "{} · palette {} of {} · click to switch",
                        palette.name,
                        theme.palette + 1,
                        BUILT_IN_PALETTES
                    )),
            )
            .child(swatches)
    }
}

fn sample_button(theme: &Theme, text: TypeStyle) -> Div {
    div()
        .h(px(40.))
        .px_4()
        .flex()
        .items_center()
        .border(theme.borders.button.width)
        .rounded(theme.radii.md)
        .type_style(theme, text)
}

fn main() {
    application().run(|cx: &mut App| {
        install_fonts(cx);
        let settings = Settings::default();
        set_theme(Theme::from_settings(&settings), cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(940.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Toru".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Toru { settings }),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
