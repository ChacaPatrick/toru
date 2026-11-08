#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The controls use the theme's control values; the cards, gallery and editor parts wait for
// Phase 4 and Phase 8.
#[allow(dead_code)]
mod theme;
mod ui;

use std::time::Duration;

use gpui::{
    App, Bounds, Context, Div, ElementId, Entity, FocusHandle, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_kit::component::input::InputState;
use gpui_platform::application;
use toru_core::settings::{BUILT_IN_PALETTES, PaletteChoice, Settings};

use theme::{ActiveTheme, Theme, ThemedText, install_fonts, set_theme};
use ui::{Button, Chip, Flash, Icon, Menu, Segmented, TextField, Toast, Toggle};

const DELAYS: [&str; 3] = ["3 s", "5 s", "10 s"];
const FORMATS: [&str; 3] = ["PNG", "JPG", "WebP"];
const TOGGLES: [&str; 3] = [
    "Show previous screenshots",
    "Copy to clipboard on capture",
    "Auto-save every capture",
];

/// A gallery of the shared controls in every state. The real window arrives in Phase 4.
struct Gallery {
    settings: Settings,
    /// The first tab stop, focused when the window opens so Tab works from the start.
    first: FocusHandle,
    toast: Flash<SharedString>,
    copied: Flash<()>,
    toggles: [bool; 3],
    format: usize,
    delay: usize,
    last_action: SharedString,
    name: Entity<InputState>,
    search: Entity<InputState>,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let first = cx.focus_handle().tab_stop(true);
        first.focus(window, cx);
        let name =
            cx.new(|cx| InputState::new(window, cx).default_value("screenshot_11_07_2026_3"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search names…"));
        Gallery {
            settings: Settings::default(),
            first,
            toast: Flash::new(),
            copied: Flash::new(),
            toggles: [true, true, false],
            format: 0,
            delay: 1,
            last_action: "nothing yet".into(),
            name,
            search,
        }
    }

    fn next_palette(&mut self, cx: &mut Context<Self>) {
        let look = self.settings.look;
        let next = (cx.theme().palette + 1) % BUILT_IN_PALETTES;
        self.settings.appearance_mut(look).palette = PaletteChoice::BuiltIn(next);
        set_theme(Theme::from_settings(&self.settings), cx);
    }

    fn did(&mut self, action: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.last_action = action.into();
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let copied = cx.theme().strings.copied;
        self.toast
            .show(copied.into(), Duration::from_secs(2), cx, |this| {
                &mut this.toast
            });
        self.copied
            .show((), Duration::from_millis(1500), cx, |this| &mut this.copied);
        self.did("Copy", cx);
    }

    /// Runs `f` on this view from a control's handler.
    fn act(
        cx: &Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let view = cx.entity();
        move |_, cx| view.update(cx, |this, cx| f(this, cx))
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let text = &theme.type_scale;
        let palette = &Theme::palettes(theme.look)[theme.palette];

        let toolbar = row()
            .gap_2p5()
            .child(
                Button::new("region", "Region")
                    .icon(Icon::Region)
                    .primary()
                    .track_focus(&self.first)
                    .on_click(Self::act(cx, |this, cx| this.did("Region", cx))),
            )
            .child(
                Button::new("window", "Window")
                    .icon(Icon::Window)
                    .on_click(Self::act(cx, |this, cx| this.did("Window", cx))),
            )
            .child(
                Button::new("full-screen", "Full screen")
                    .icon(Icon::Screen)
                    .sub("Display 1 ▾")
                    .on_click(Self::act(cx, |this, cx| this.did("Full screen", cx))),
            )
            .child(
                DELAYS.iter().enumerate().fold(
                    Menu::new(
                        "delayed",
                        "Delay",
                        Button::new("delayed", "Delayed")
                            .icon(Icon::Clock)
                            .sub(format!("{} ▾", DELAYS[self.delay])),
                    ),
                    |menu, (index, label)| {
                        menu.item(
                            *label,
                            Self::act(cx, move |this, cx| {
                                this.delay = index;
                                this.did(format!("Delayed {label}"), cx);
                            }),
                        )
                    },
                ),
            )
            .child(hint(cx, "PrtScn · Ctrl+Shift+S"))
            .child(div().flex_1())
            .child(
                Button::new("new-session", "New session")
                    .icon(Icon::Plus)
                    .on_click(Self::act(cx, |this, cx| this.did("New session", cx))),
            )
            .child(
                Button::icon_only("settings", Icon::Gear, "Settings")
                    .on_click(Self::act(cx, |this, cx| this.did("Settings", cx))),
            );

        let unsaved_actions = row()
            .gap_1p5()
            .child(Chip::new("chip-unsaved", "Unsaved"))
            .child(
                Button::new("copy", "Copy")
                    .small()
                    .check_mark(self.copied.get().is_some())
                    .on_click(Self::act(cx, |this, cx| this.copy(cx))),
            )
            .child(
                Button::new("save", "Save")
                    .small()
                    .primary()
                    .on_click(Self::act(cx, |this, cx| this.did("Save", cx))),
            )
            .child(card_menu("unsaved", cx));

        let saved_actions = row()
            .gap_1p5()
            .child(Chip::new("chip-saved", "Saved ✓").saved(true))
            .child(
                Button::new("copy-saved", "Copy")
                    .small()
                    .on_click(Self::act(cx, |this, cx| this.copy(cx))),
            )
            .child(
                Button::new("open-folder", "Open folder")
                    .small()
                    .on_click(Self::act(cx, |this, cx| this.did("Open folder", cx))),
            )
            .child(card_menu("saved", cx).align_right());

        let toggles = div().w(px(264.)).flex().flex_col().gap(px(11.)).children(
            TOGGLES.iter().enumerate().map(|(index, label)| {
                row()
                    .justify_between()
                    .child(div().type_style(theme, text.setting).child(*label))
                    .child(
                        Toggle::new(("toggle", index), *label, self.toggles[index]).on_change({
                            let view = cx.entity();
                            move |on, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.toggles[index] = on;
                                    let state = if on { "on" } else { "off" };
                                    this.did(format!("{label}: {state}"), cx);
                                })
                            }
                        }),
                    )
            }),
        );

        let format = div().w(px(264.)).child(
            Segmented::new("format", "Image format", FORMATS, self.format).on_change({
                let view = cx.entity();
                move |index, _, cx| {
                    view.update(cx, |this, cx| {
                        this.format = index;
                        this.did(format!("Format {}", FORMATS[index]), cx);
                    })
                }
            }),
        );

        let fields = row()
            .gap_2()
            .child(TextField::new(&self.name, "Screenshot name").w(px(280.)))
            .child(
                TextField::new(&self.search, "Search names")
                    .height(px(32.))
                    .w(px(180.)),
            );

        let icons = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .children(Icon::ALL.map(|icon| icon.render(theme, c.text)));

        div()
            .id("gallery")
            .relative()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .bg(c.bg)
            .text_color(c.text)
            .type_style(theme, text.body)
            .child(
                row()
                    .justify_between()
                    .child(
                        div()
                            .type_style(theme, text.section)
                            .child(format!("Controls · {}", palette.name)),
                    )
                    .child(
                        Button::new("palette", "Next palette")
                            .small()
                            .on_click(Self::act(cx, |this, cx| this.next_palette(cx))),
                    ),
            )
            .child(section(cx, "Buttons", toolbar))
            .child(section(
                cx,
                "Card actions and chips",
                row().gap_6().child(unsaved_actions).child(saved_actions),
            ))
            .child(section(cx, "Toggles", toggles))
            .child(section(cx, "Segmented control", format))
            .child(section(cx, "Text fields", fields))
            .child(section(cx, "Icons", icons))
            .child(hint(cx, format!("Last action: {}", self.last_action)))
            .children(self.toast.get().map(|message| Toast::new(message.clone())))
    }
}

/// A card's `⋯` menu.
fn card_menu(id: &'static str, cx: &Context<Gallery>) -> Menu {
    const ITEMS: [&str; 4] = ["Open in editor", "Rename", "Reveal", "Delete"];
    ITEMS.iter().fold(
        Menu::new(
            ElementId::Name(format!("card-menu-{id}").into()),
            "More",
            Button::glyph_only(ElementId::Name(format!("more-{id}").into()), "⋯", "More").small(),
        ),
        |menu, label| {
            menu.item(
                *label,
                Gallery::act(cx, move |this, cx| this.did(*label, cx)),
            )
        },
    )
}

fn row() -> Div {
    div().flex().items_center()
}

fn hint(cx: &Context<Gallery>, text: impl Into<SharedString>) -> Div {
    let theme = cx.theme();
    div()
        .type_style(theme, theme.type_scale.hint)
        .text_color(theme.colors.muted)
        .child(text.into())
}

fn section(cx: &Context<Gallery>, label: &'static str, body: impl IntoElement) -> Div {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(11.))
        .child(
            div()
                .type_style(theme, theme.type_scale.label)
                .text_color(theme.colors.label)
                .child(label),
        )
        .child(body)
}

fn main() {
    application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        ui::init(cx);
        install_fonts(cx);
        let settings = Settings::default();
        set_theme(Theme::from_settings(&settings), cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Toru".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Gallery::new(window, cx)),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
