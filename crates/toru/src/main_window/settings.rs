//! The settings column (DESIGN.md §2 with CLAUDE.md's overrides): Session, Capture, Shortcuts,
//! Save to, Name pattern, Format and On close, each group a label over its controls, as the
//! mockups' `.settings` lays them out, then Look and Palette (§8), which have no mockup and
//! follow the Palettes boards. Every control shows what is in `Settings` and every change
//! goes through `MainWindow::change_settings`. The column scrolls on its own when the window is
//! short, and follows keyboard focus so a focused control always shows with its ring. Its hide
//! button, at the right of the first label, collapses it; the toolbar's gear brings it back.

use std::{path::PathBuf, sync::Arc};

use gpui::{App, Context, Div, ElementId, Hsla, SharedString, Window, div, point, prelude::*, px};
use gpui_kit::component::scroll::ScrollableElement;
use toru_core::{
    naming::NameError,
    settings::{Look, OnClose, PaletteChoice, SaveFormat, Settings},
};

use super::{MainWindow, act};
use crate::theme::{ActiveTheme, Theme, ThemedText, set_theme};
use crate::ui::{
    BUTTON_SMALL_HEIGHT, Button, Icon, MIN_TARGET, RadioGroup, Segmented, TextField, Toggle,
};

pub const WIDTH: f32 = 300.;
const PADDING: f32 = 18.;
const GROUP_GAP: f32 = 22.;
const GAP: f32 = 11.;
const ROW_GAP: f32 = 10.;
/// `.hint`'s `line-height: 1.45`.
const HINT_LEADING: f32 = 1.45;
const FIELD_HEIGHT: f32 = 36.;
const PATTERN_HEIGHT: f32 = 34.;
/// A label row is at least this tall. The hide button, centred on the first one, overhangs it
/// equally above and below, so the label stays where the mockups have it and the ring above the
/// button clears the column's top edge.
const LABEL_ROW_HEIGHT: f32 = 16.;
/// The least air between two click targets (DESIGN.md §7).
const TARGET_GAP: f32 = 8.;

/// The Palettes boards' `.swatches`: a strip of four, the width of its row.
const SWATCH_HEIGHT: f32 = 26.;
const SWATCH_GAP: f32 = 6.;

const FORMATS: [SaveFormat; 3] = [SaveFormat::Png, SaveFormat::Jpg, SaveFormat::WebP];
const ON_CLOSE: [OnClose; 3] = [OnClose::Ask, OnClose::Keep, OnClose::Discard];
const LOOKS: [Look; 3] = [Look::Japandi, Look::PixelArt, Look::NeoBrutalism];
const LOOK_NAMES: [&str; 3] = ["Japandi", "Pixel art", "Neo-brutalism"];

pub fn settings_column(
    this: &mut MainWindow,
    window: &Window,
    cx: &mut Context<MainWindow>,
) -> Div {
    let by = this
        .settings_focus
        .scroll_by(this.settings_scroll.bounds(), window, cx);
    if by != px(0.) {
        this.settings_scroll
            .set_offset(this.settings_scroll.offset() + point(px(0.), by));
    }
    let cx = &*cx;
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let settings = &this.settings;

    let hide = Button::icon_only("hide-settings", Icon::ChevronRight, "Hide settings")
        .small()
        .on_click({
            let view = cx.entity();
            move |window, cx| {
                view.update(cx, |this, cx| {
                    // The gear brings the column back, so focus goes there rather than to
                    // whatever `on_focus_lost` would pick when the hide button vanishes.
                    this.gear.focus(window, cx);
                    this.change_settings(cx, |settings| settings.settings_collapsed = true);
                })
            }
        });

    let content = div()
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .p(px(PADDING))
        .child(
            group_with(theme, "Session", hide)
                .child(toggle(
                    cx,
                    "show-previous",
                    "Show previous screenshots",
                    settings.show_previous,
                    |settings, on| settings.show_previous = on,
                ))
                .child(
                    hint(
                        theme,
                        "Off = clean slate each session. On = earlier shots load below, a day \
                         at a time as you scroll.",
                    )
                    .mt(px(-4.)),
                ),
        )
        .child(
            group(theme, "Capture")
                .child(toggle(
                    cx,
                    "copy-on-capture",
                    "Copy to clipboard on capture",
                    settings.copy_on_capture,
                    |settings, on| settings.copy_on_capture = on,
                ))
                .child(toggle(
                    cx,
                    "auto-save",
                    "Auto-save every capture",
                    settings.auto_save,
                    |settings, on| settings.auto_save = on,
                ))
                .child(toggle(
                    cx,
                    "keep-in-tray",
                    "Keep running in the tray",
                    settings.keep_in_tray,
                    |settings, on| settings.keep_in_tray = on,
                ))
                .child(toggle(
                    cx,
                    "open-editor",
                    "Open editor after capture",
                    settings.open_editor_after_capture,
                    |settings, on| settings.open_editor_after_capture = on,
                )),
        )
        .child(
            group(theme, "Shortcuts")
                .child(shortcut(
                    theme,
                    settings.region_shortcut.as_deref(),
                    "Region (default)",
                ))
                .child(shortcut(
                    theme,
                    settings.window_shortcut.as_deref(),
                    "Window",
                ))
                .child(shortcut(
                    theme,
                    settings.full_screen_shortcut.as_deref(),
                    "Full screen",
                ))
                // Rebinding arrives in 6.2. The row keeps the button at its own width, as the
                // mockups' `align-self: flex-start` does.
                .child(
                    div().flex().child(
                        Button::new("edit-shortcuts", "Edit shortcuts…")
                            .small()
                            .on_click(|_, _| {}),
                    ),
                ),
        )
        .child(
            group(theme, "Save to").child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(folder_field(theme, this.save_folder()))
                    // The folder picker arrives in 5.6.
                    .child(
                        Button::new("change-folder", "Change")
                            .small()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        .child(
            group(theme, "Name pattern")
                .child(
                    TextField::new(&this.pattern, "Name pattern")
                        .height(px(PATTERN_HEIGHT))
                        .w_full(),
                )
                .child(pattern_hint(theme, this.pattern_error.as_ref())),
        )
        .child(
            group(theme, "Format").child(
                Segmented::new(
                    "format",
                    "Image format",
                    ["PNG", "JPG", "WebP"],
                    FORMATS
                        .iter()
                        .position(|format| *format == settings.format)
                        .unwrap_or(0),
                )
                .on_change(act_with(cx, |this, cx, index| {
                    this.change_settings(cx, |settings| settings.format = FORMATS[index])
                })),
            ),
        )
        .child(
            group(theme, "On close").child(
                RadioGroup::new(
                    "on-close",
                    "On close",
                    [
                        "Ask before discarding unsaved",
                        "Keep unsaved in history",
                        "Discard silently",
                    ],
                    ON_CLOSE
                        .iter()
                        .position(|choice| *choice == settings.on_close)
                        .unwrap_or(0),
                )
                .on_change(act_with(cx, |this, cx, index| {
                    this.change_settings(cx, |settings| settings.on_close = ON_CLOSE[index])
                })),
            ),
        )
        .child(
            group(theme, "Look").child(
                RadioGroup::new(
                    "look",
                    "Look",
                    LOOK_NAMES,
                    LOOKS
                        .iter()
                        .position(|look| *look == theme.look)
                        .unwrap_or(0),
                )
                .on_change(act_with(cx, |this, cx, index| {
                    this.change_appearance(cx, |settings, _| settings.look = LOOKS[index])
                })),
            ),
        )
        .child(group(theme, "Palette").child(palette_picker(theme, cx)));

    div()
        .relative()
        .flex_none()
        .w(px(WIDTH))
        .h_full()
        .bg(c.surface_2)
        .border_l(hair.width)
        .border_color(hair.color)
        .child(
            div()
                .id("settings-scroll")
                // Tells `FocusInView` the focus is in the column. A press on the column's blank
                // space focuses the column itself, so Tab then goes to its first control.
                .track_focus(this.settings_focus.focus())
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&this.settings_scroll)
                .child(content),
        )
        .vertical_scrollbar(&this.settings_scroll)
}

impl MainWindow {
    /// A change to the look or palette. The pickers show the active theme, which the start-up
    /// arguments can set apart from the saved look, so `change` gets the theme's look and the
    /// choice is saved under it; then every window takes the new theme.
    fn change_appearance(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Settings, Look),
    ) {
        let shown = cx.theme().look;
        self.change_settings(cx, |settings| {
            settings.look = shown;
            change(settings, shown);
        });
        set_theme(Theme::from_settings(&self.settings), cx);
    }
}

/// The active look's palettes, each row its name over its four colours. The look's choice is
/// remembered when another look is shown. Section 9.2 adds the custom slot as a fourth row.
fn palette_picker(theme: &Theme, cx: &Context<MainWindow>) -> RadioGroup {
    let palettes = Theme::palettes(theme.look);
    RadioGroup::new(
        "palette",
        "Palette",
        palettes.iter().map(|palette| palette.name),
        theme.palette,
    )
    .details(
        palettes
            .iter()
            .map(|palette| swatches(theme, &palette.swatches).into_any_element()),
    )
    .on_change(act_with(cx, |this, cx, index| {
        this.change_appearance(cx, |settings, look| {
            settings.appearance_mut(look).palette = PaletteChoice::BuiltIn(index)
        })
    }))
}

/// `.swatches`: a palette's colours in equal boxes across the row, each edged like an input so
/// the ones matching the surface still show.
fn swatches(theme: &Theme, colors: &[Hsla; 4]) -> Div {
    let border = theme.borders.input;
    div()
        .flex()
        .gap(px(SWATCH_GAP))
        .children(colors.iter().map(|&color| {
            div()
                .flex_1()
                .h(px(SWATCH_HEIGHT))
                .bg(color)
                .border(border.width)
                .border_color(border.color)
                .rounded(theme.radii.sm)
        }))
}

/// A handler that takes the control's value, such as the chosen index, along to the view.
fn act_with<T: 'static>(
    cx: &Context<MainWindow>,
    f: impl Fn(&mut MainWindow, &mut Context<MainWindow>, T) + 'static,
) -> impl Fn(T, &mut Window, &mut App) + 'static {
    let view = cx.entity();
    move |value, _, cx| view.update(cx, |this, cx| f(this, cx, value))
}

/// `.group`: its label over its controls.
fn group(theme: &Theme, label: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .child(label_row(theme, label))
}

/// A group whose label row also holds a small button at the right, overhanging the row by the
/// same amount above and below so the label stays where the mockups put it. The group's gap is
/// less than that overhang plus §7's air, so the row makes up the difference below and the
/// control under it starts clear of the button.
fn group_with(theme: &Theme, label: &'static str, button: Button) -> Div {
    let overhang = (BUTTON_SMALL_HEIGHT - LABEL_ROW_HEIGHT) / 2.;
    let clearance = (overhang + TARGET_GAP - GAP).max(0.);
    div().flex().flex_col().gap(px(GAP)).child(
        label_row(theme, label).mb(px(clearance)).child(
            div()
                .flex_none()
                .mt(px(-overhang))
                .mb(px(-overhang))
                .child(button),
        ),
    )
}

fn label_row(theme: &Theme, label: &'static str) -> Div {
    let style = theme.type_scale.label;
    div()
        .flex()
        .items_center()
        .justify_between()
        .min_h(px(LABEL_ROW_HEIGHT))
        .child(
            div()
                .type_style(theme, style)
                .text_color(theme.colors.label)
                .child(style.case.apply(label)),
        )
}

/// `.row`: a setting's name with its switch at the right. Pressing the name toggles too, as a
/// `<label for>` does, and the name is a target as tall as the switch's box (DESIGN.md §7).
fn toggle(
    cx: &Context<MainWindow>,
    id: &'static str,
    label: &'static str,
    on: bool,
    set: impl Fn(&mut Settings, bool) + Clone + 'static,
) -> Div {
    let theme = cx.theme();
    let flip = act(cx, {
        let set = set.clone();
        move |this, cx| this.change_settings(cx, |settings| set(settings, !on))
    });
    row(theme)
        .child(
            div()
                .id(ElementId::NamedChild(Arc::new(id.into()), "label".into()))
                // Wraps, as the PixelArt mockup's longer labels do, instead of pushing the
                // switch out of the column.
                .flex_1()
                .min_w_0()
                .min_h(px(MIN_TARGET))
                .flex()
                .items_center()
                .type_style(theme, theme.type_scale.setting)
                .cursor_pointer()
                .on_click(move |_, window, cx| flip(window, cx))
                .child(label),
        )
        .child(
            Toggle::new(id, label, on).on_change(act_with(cx, move |this, cx, on| {
                this.change_settings(cx, |settings| set(settings, on))
            })),
        )
}

/// A shortcut row: the keys, or "Not set", with what they capture at the right.
fn shortcut(theme: &Theme, keys: Option<&str>, action: &'static str) -> Div {
    let keys = keys.map_or_else(|| "Not set".to_string(), shortcut_text);
    row(theme)
        .child(
            div()
                .type_style(theme, theme.type_scale.setting)
                .child(keys),
        )
        .child(hint(theme, action).flex_none())
}

fn row(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(ROW_GAP))
        .text_color(theme.colors.text)
}

/// `.hint`, wrapping as the mockups' Session hint does.
fn hint(theme: &Theme, text: impl Into<SharedString>) -> Div {
    let style = theme.type_scale.hint;
    div()
        .type_style(theme, style)
        .line_height(style.size * HINT_LEADING)
        .text_color(theme.colors.muted)
        .child(text.into())
}

/// `.field`: the save folder as one line of the field font, clipped at the right.
fn folder_field(theme: &Theme, folder: Option<PathBuf>) -> Div {
    let border = theme.borders.input;
    let path = folder.map_or_else(
        || "Pictures folder not found".to_string(),
        |path| path.display().to_string(),
    );
    div()
        .flex_1()
        .min_w_0()
        .h(px(FIELD_HEIGHT))
        .flex()
        .items_center()
        .px(px(10.))
        .bg(theme.colors.input_bg)
        .border(border.width)
        .border_color(border.color)
        .rounded(theme.radii.md)
        .type_style(theme, theme.type_scale.field)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(path)
}

/// Under the pattern field: its hint, or, while the typed pattern is invalid, what is wrong, in
/// the text colour so the words carry it (DESIGN.md §7). Both are one line in the same place, so
/// nothing below moves when one replaces the other.
fn pattern_hint(theme: &Theme, error: Option<&NameError>) -> Div {
    let (text, color) = match error {
        Some(error) => (pattern_message(error), theme.colors.text),
        None => (
            "Same-day captures get _2, _3 … appended.".to_string(),
            theme.colors.muted,
        ),
    };
    hint(theme, text)
        .text_color(color)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
}

/// What is wrong with a pattern, short enough for one line of the column in every look.
pub fn pattern_message(error: &NameError) -> String {
    match error {
        NameError::Empty => "Type a name pattern.".to_string(),
        NameError::ReservedCharacter(c) if c.is_control() => {
            "Can't use control characters.".to_string()
        }
        NameError::ReservedCharacter(c) => format!("Can't use {c} in a name."),
        NameError::TrailingDotOrSpace => "Can't end with a dot or space.".to_string(),
        NameError::ReservedDeviceName => "That's a reserved Windows name.".to_string(),
        NameError::TooLong => "Too long for a file name.".to_string(),
        NameError::UnknownToken(_) => "Only {mm}, {dd} and {yyyy} work.".to_string(),
    }
}

/// "Ctrl+Shift+W" as the mockups space it: "Ctrl + Shift + W".
pub fn shortcut_text(keys: &str) -> String {
    keys.replace('+', " + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_spaced_like_the_mockups() {
        assert_eq!(shortcut_text("PrtScn"), "PrtScn");
        assert_eq!(shortcut_text("Ctrl+Shift+W"), "Ctrl + Shift + W");
    }

    #[test]
    fn pattern_messages_fit_one_line() {
        let errors = [
            NameError::Empty,
            NameError::ReservedCharacter(':'),
            NameError::ReservedCharacter('\0'),
            NameError::TrailingDotOrSpace,
            NameError::ReservedDeviceName,
            NameError::TooLong,
            NameError::UnknownToken("a_very_long_token_name".to_string()),
        ];
        for error in errors {
            let message = pattern_message(&error);
            assert!(message.chars().count() <= 34, "{message}");
            assert!(message.chars().all(|c| !c.is_control()), "{message}");
        }
        assert_eq!(
            pattern_message(&NameError::ReservedCharacter(':')),
            "Can't use : in a name."
        );
    }
}
