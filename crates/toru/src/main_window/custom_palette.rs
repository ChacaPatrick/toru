//! The custom palette editor (DESIGN.md §8 with CLAUDE.md's fourth slot): a modal panel over the
//! main window with the six colours as hex fields beside live swatches, the background photo
//! with its picker, and the contrast warning from `theme::contrast` run on the colours as typed.
//! Apply saves the colours and photo for the look and shows them; Cancel puts back what was
//! shown. While it is open the window wears one of the look's built-in palettes, so the editor
//! can be read whatever the custom colours are, and a bad palette can always be fixed from it.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Role, ScrollHandle,
    SharedString, Window, div, point, prelude::*, px,
};
use gpui_kit::component::{
    input::{InputEvent, InputState},
    scroll::ScrollableElement,
};
use toru_core::settings::{Look, PaletteChoice};

use super::{MainWindow, act};
use crate::theme::{
    ActiveTheme, CustomPalette, Theme, ThemedText, contrast,
    custom::{FIELDS, hex_text, parse_hex},
    custom_palette, scrim, set_theme,
};
use crate::ui::{Button, FocusInView, Readable, TextField, close_menus, dialog, menus_open};

const WIDTH: f32 = 480.;
const PADDING: f32 = 22.;
/// Between the title, the fields, the photo, the warning and the actions.
const GAP: f32 = 14.;
const ROW_GAP: f32 = 8.;
const ACTION_GAP: f32 = 8.;
const FIELD_WIDTH: f32 = 112.;
const FIELD_HEIGHT: f32 = 32.;
/// The swatch beside a field: as tall as the field, wider than it is tall.
const SWATCH_WIDTH: f32 = 44.;
/// `.hint`'s `line-height: 1.45`.
const LEADING: f32 = 1.45;
/// How many failing pairs the warning names before counting the rest.
const NAMED_FAILURES: usize = 3;

/// How many buttons the panel has besides the fields: Change, Remove, Cancel, Apply.
const BUTTONS: usize = 4;

pub struct CustomEditor {
    /// Which opening of the editor this is, so a picker's result reaches the editor that asked
    /// for it and not one opened since, perhaps on another look.
    session: u32,
    look: Look,
    /// What the window showed when the editor opened, put back on Cancel.
    was: PaletteChoice,
    /// One hex field per entry of `FIELDS`.
    fields: Vec<Entity<InputState>>,
    /// The colours as last parsed; a field that doesn't parse leaves its colour as it was.
    palette: CustomPalette,
    invalid: [bool; FIELDS.len()],
    background: Option<PathBuf>,
    /// Focus for the buttons, in `BUTTONS` order, so Tab can step through the panel.
    buttons: [FocusHandle; BUTTONS],
    /// The content above Cancel and Apply, which scrolls when the window is too short for it.
    scroll: ScrollHandle,
    /// Scrolls that content to whichever of its controls takes keyboard focus.
    focus: FocusInView,
}

/// The window's file picker for a background photo, while one is open: which editor session
/// asked for it. One window has one picker at a time, whatever happens to the editor meanwhile.
#[derive(Default)]
pub struct BackgroundPicker {
    open_for: Option<u32>,
    /// How many editors have been opened, which numbers the next one.
    sessions: u32,
}

impl MainWindow {
    /// Opens the editor on the look shown, starting from its stored custom colours or, when
    /// none are stored yet, from the palette on screen.
    pub(super) fn open_custom_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = cx.theme();
        let look = theme.look;
        let was = if theme.is_custom() {
            PaletteChoice::Custom
        } else {
            PaletteChoice::BuiltIn(theme.palette)
        };
        let appearance = self.settings.appearance(look);
        let palette = if appearance.custom_palette.is_empty() && !theme.is_custom() {
            CustomPalette::from_theme(theme)
        } else {
            custom_palette(look, appearance)
        };
        let background = appearance.custom_background.clone();
        let fields: Vec<Entity<InputState>> = FIELDS
            .iter()
            .enumerate()
            .map(|(index, (key, _))| {
                let state = cx.new(|cx| {
                    InputState::new(window, cx).default_value(hex_text(palette.get(key)))
                });
                cx.subscribe(&state, move |this, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        this.custom_field_changed(index, cx);
                    }
                })
                .detach();
                state
            })
            .collect();
        self.note_opener(window.focused(cx));
        fields[0].update(cx, |state, cx| state.focus(window, cx));
        self.background_picker.sessions += 1;
        self.custom_editor = Some(CustomEditor {
            session: self.background_picker.sessions,
            look,
            was,
            fields,
            palette,
            invalid: [false; FIELDS.len()],
            background,
            buttons: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            scroll: ScrollHandle::new(),
            focus: FocusInView::new(cx),
        });
        // The window wears a palette known to read while the custom one is edited.
        let safe = match was {
            PaletteChoice::BuiltIn(index) => index,
            PaletteChoice::Custom => 0,
        };
        set_theme(Theme::new(look, safe), cx);
    }

    /// Where focus goes when the control holding it goes and nothing in the panel is left to
    /// take it, as when a dialog over the panel closes: the first colour field.
    pub(super) fn custom_editor_focus(&self, cx: &gpui::App) -> Option<FocusHandle> {
        let editor = self.custom_editor.as_ref()?;
        Some(editor.fields[0].focus_handle(cx))
    }

    fn custom_field_changed(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.custom_editor else {
            return;
        };
        let text = editor.fields[index].read(cx).value();
        match parse_hex(&text) {
            Some(color) => {
                editor.palette.set(FIELDS[index].0, color);
                editor.invalid[index] = false;
            }
            None => editor.invalid[index] = true,
        }
        cx.notify();
    }

    /// Cancel or Esc: back to what the window showed before.
    fn cancel_custom_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.custom_editor.take() else {
            return;
        };
        let appearance = self.settings.appearance(editor.look);
        set_theme(Theme::for_choice(editor.look, editor.was, appearance), cx);
        cx.notify();
    }

    /// Apply: the colours and photo are saved for the look and the custom palette is chosen.
    fn apply_custom_palette(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.custom_editor.take() else {
            return;
        };
        let stored = editor.palette.to_stored();
        let background = editor.background;
        self.change_appearance(cx, |settings, look| {
            let appearance = settings.appearance_mut(look);
            appearance.custom_palette = stored;
            appearance.custom_background = background;
            appearance.palette = PaletteChoice::Custom;
        });
    }

    /// Change: the system file picker, for a picture, owned by Toru's window so it stays in front
    /// of it. The picker runs on its own thread and is awaited from a task, so the app is free to
    /// run frames and other tasks while it is open; a blocking picker on this call stack would
    /// run them inside the click handler, where anything borrowing the app panics. While it is
    /// open a second Change… does nothing, and its result goes only to the editor session that
    /// asked: the editor may have been closed, and another opened, meanwhile.
    fn pick_background(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.custom_editor else {
            return;
        };
        if self.background_picker.open_for.is_some() {
            return;
        }
        let session = editor.session;
        self.background_picker.open_for = Some(session);
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Background photo")
            .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .set_parent(window);
        cx.spawn(async move |this, cx| {
            let picked = dialog.pick_file().await;
            this.update(cx, |this, cx| {
                this.background_picker.open_for = None;
                let Some(file) = picked else {
                    return;
                };
                if let Some(editor) = this.custom_editor.as_mut().filter(|e| e.session == session) {
                    editor.background = Some(file.path().to_path_buf());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Remove: the photo goes, and with it its button, so the focus moves to Change…, which
    /// stays; focus left on the vanished button would fall out of the panel, past Esc and Tab.
    fn remove_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.custom_editor {
            editor.background = None;
            let [change, ..] = &editor.buttons;
            change.focus(window, cx);
            cx.notify();
        }
    }

    /// The editor laid over the window, when it is open.
    pub(super) fn custom_editor_view(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.custom_editor.as_mut()?;
        // The scrolling part follows the keyboard focus, as the settings column does.
        let by = editor.focus.scroll_by(editor.scroll.bounds(), window, cx);
        if by != px(0.) {
            editor
                .scroll
                .set_offset(editor.scroll.offset() + point(px(0.), by));
        }
        let editor = &*editor;
        let cx = &*cx;
        // As `Dialog` does: a menu open under the panel would paint above it.
        if menus_open(cx) {
            window.on_next_frame(|_, cx| close_menus(cx));
        }
        let theme = cx.theme();
        let c = &theme.colors;
        let pop = theme.borders.pop;
        let input = theme.borders.input;
        let [change, remove, cancel, apply] = &editor.buttons;

        let fields =
            FIELDS
                .iter()
                .zip(&editor.fields)
                .enumerate()
                .map(|(index, ((key, label), state))| {
                    let color = editor.palette.get(key);
                    div()
                        .flex()
                        .items_center()
                        .gap(px(ROW_GAP))
                        .child(
                            div()
                                .flex_1()
                                .type_style(theme, theme.type_scale.setting)
                                .child(*label),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(SWATCH_WIDTH))
                                .h(px(FIELD_HEIGHT))
                                .bg(color)
                                .border(input.width)
                                .border_color(input.color)
                                .rounded(theme.radii.sm),
                        )
                        .child(
                            TextField::new(state, format!("{label} colour"))
                                .height(px(FIELD_HEIGHT))
                                .w(px(FIELD_WIDTH))
                                .warning(editor.invalid[index]),
                        )
                });

        let photo = div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .child(label(theme, "Background photo"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(ROW_GAP))
                    .child(photo_field(theme, editor.background.as_deref()))
                    .child({
                        let view = cx.entity();
                        Button::new("change-background", "Change…")
                            .named("Change background photo")
                            .small()
                            .track_focus(change)
                            .on_click(move |window, cx| {
                                view.update(cx, |this, cx| this.pick_background(window, cx))
                            })
                    })
                    .when(editor.background.is_some(), |row| {
                        let view = cx.entity();
                        row.child(
                            Button::new("remove-background", "Remove")
                                .named("Remove background photo")
                                .small()
                                .track_focus(remove)
                                .on_click(move |window, cx| {
                                    view.update(cx, |this, cx| this.remove_background(window, cx))
                                }),
                        )
                    }),
            );

        let Warnings { lines, all_clear } = warnings(
            editor.look,
            &editor.palette,
            &editor.invalid,
            editor.background.as_deref(),
        );
        let hint = theme.type_scale.hint;
        let warning = div()
            .id("palette-warnings")
            .read_as_text(lines.join(" "))
            .flex()
            .flex_col()
            .gap(px(2.))
            .type_style(theme, hint)
            .line_height(hint.size * LEADING)
            // Warnings in the text colour, so the words carry them (DESIGN.md §7).
            .text_color(if all_clear { c.muted } else { c.text })
            .children(lines.into_iter().map(SharedString::from));

        // Tab steps through the fields and buttons, wrapping, as in `Dialog`.
        let mut order: Vec<FocusHandle> = editor
            .fields
            .iter()
            .map(|state| state.focus_handle(cx))
            .collect();
        order.push(change.clone());
        if editor.background.is_some() {
            order.push(remove.clone());
        }
        order.extend([cancel.clone(), apply.clone()]);
        let step = |step: isize| {
            let order = order.clone();
            move |window: &mut Window, cx: &mut gpui::App| {
                let count = order.len() as isize;
                let current = order
                    .iter()
                    .position(|focus| focus.is_focused(window))
                    .unwrap_or(0) as isize;
                order[(current + step).rem_euclid(count) as usize].focus(window, cx);
            }
        };
        let (next, previous) = (step(1), step(-1));
        let escape = act(cx, |this, cx| this.cancel_custom_editor(cx));

        let about = format!(
            "Your own colours for {}. Six hex colours; the rest of the palette follows them. \
             Apply shows them.",
            look_name(editor.look)
        );
        let panel = div()
            .id("custom-palette")
            .role(Role::Dialog)
            .aria_label("Custom palette")
            .aria_description(about.clone())
            .modal_dialog()
            .key_context(dialog::KEY_CONTEXT)
            // Only Esc is stopped. A stopped key-down gets no WM_CHAR on Windows, so stopping
            // every key would leave the hex fields deaf to typing. The panel is a sibling of the
            // window frame, so other keys pressed in it never reach the frame's shortcuts, and
            // the frame ignores keys while the panel is open, should focus be left behind it.
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    escape(window, cx);
                }
            })
            .on_action(move |_: &dialog::NextAction, window, cx| next(window, cx))
            .on_action(move |_: &dialog::PreviousAction, window, cx| previous(window, cx))
            .flex()
            .flex_col()
            .gap(px(GAP))
            .w(px(WIDTH))
            .max_w_full()
            // In a short window the content scrolls and Cancel and Apply stay in view.
            .max_h_full()
            .p(px(PADDING))
            .bg(c.surface)
            .text_color(c.text)
            .border(pop.width)
            .border_color(pop.color)
            .rounded(theme.radii.lg)
            .shadow(theme.shadows.pop.clone())
            .child(
                div()
                    .relative()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("custom-palette-scroll")
                            // Tells `FocusInView` the focus is in the scrolling part.
                            .track_focus(editor.focus.focus())
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&editor.scroll)
                            .flex()
                            .flex_col()
                            .gap(px(GAP))
                            .child(
                                div()
                                    .type_style(theme, theme.type_scale.section)
                                    .child(theme.type_scale.section.case.apply("Custom palette")),
                            )
                            .child(
                                div()
                                    .type_style(theme, hint)
                                    .line_height(hint.size * LEADING)
                                    .text_color(c.muted)
                                    .child(about),
                            )
                            .child(div().flex().flex_col().gap(px(ROW_GAP)).children(fields))
                            .child(photo)
                            .child(warning),
                    )
                    .vertical_scrollbar(&editor.scroll),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_end()
                    .gap(px(ACTION_GAP))
                    .child(
                        Button::new("cancel-custom", "Cancel")
                            .track_focus(cancel)
                            .on_click(act(cx, |this, cx| this.cancel_custom_editor(cx))),
                    )
                    .child(
                        Button::new("apply-custom", "Apply")
                            .primary()
                            .track_focus(apply)
                            .on_click(act(cx, |this, cx| this.apply_custom_palette(cx))),
                    ),
            );

        Some(
            div()
                .id("custom-palette-scrim")
                .absolute()
                .inset_0()
                .occlude()
                .bg(scrim(c.text))
                .flex()
                .items_center()
                .justify_center()
                .p(px(PADDING))
                .child(panel)
                .into_any_element(),
        )
    }
}

/// What the editor says about the colours as typed.
pub struct Warnings {
    /// Never empty.
    pub lines: Vec<String>,
    /// Whether every field is a colour and every pair passes, so the lines only inform.
    pub all_clear: bool,
}

/// What the editor says about the colours as typed: each field that isn't a colour, then the
/// pairs that fall below DESIGN.md §7, or that none do. With a photo the pairs include text on
/// the window over a dark and over a light picture, and the line about the photo says so.
pub fn warnings(
    look: Look,
    palette: &CustomPalette,
    invalid: &[bool; FIELDS.len()],
    background: Option<&Path>,
) -> Warnings {
    let mut lines: Vec<String> = FIELDS
        .iter()
        .zip(invalid)
        .filter(|&(_, &bad)| bad)
        .map(|((_, label), _)| format!("{label} isn't a colour. Use six hex digits, like #2b2a26."))
        .collect();
    let mut theme = Theme::custom(look, palette);
    theme.background = background.map(Path::to_path_buf);
    let failures = contrast::failures(&theme);
    let all_clear = lines.is_empty() && failures.is_empty();
    if failures.is_empty() {
        lines.push(format!("Every text pair reaches {}:1.", contrast::TEXT));
    } else {
        for failure in failures.iter().take(NAMED_FAILURES) {
            lines.push(format!(
                "Low contrast: {} is {:.1}:1, needs {}:1.",
                failure.name, failure.ratio, failure.min
            ));
        }
        let more = failures.len().saturating_sub(NAMED_FAILURES);
        if more > 0 {
            lines.push(format!("…and {more} more pairs are low too."));
        }
    }
    if background.is_some() {
        lines.push(
            "With a photo, a tint of the background colour is laid over it, hints on the \
             window take the text colour, and text on the window is checked over a dark and \
             a light picture."
                .to_string(),
        );
    }
    Warnings { lines, all_clear }
}

pub fn look_name(look: Look) -> &'static str {
    match look {
        Look::Japandi => "Japandi",
        Look::PixelArt => "Pixel art",
        Look::NeoBrutalism => "Neo-brutalism",
    }
}

fn label(theme: &Theme, text: &'static str) -> impl IntoElement {
    let style = theme.type_scale.label;
    div()
        .type_style(theme, style)
        .text_color(theme.colors.label)
        .child(style.case.apply(text))
}

/// The photo's file name in a field, or "None".
fn photo_field(theme: &Theme, path: Option<&Path>) -> impl IntoElement {
    let border = theme.borders.input;
    let name = path.and_then(|path| path.file_name()).map_or_else(
        || "None".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    div()
        .id("background-photo")
        .read_as_text(format!("Background photo: {name}"))
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
        .text_ellipsis()
        .child(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::css::hex;

    fn linen() -> CustomPalette {
        CustomPalette::from_theme(&Theme::new(Look::Japandi, 0))
    }

    #[test]
    fn a_sound_palette_gets_the_all_clear() {
        let Warnings { lines, all_clear } = warnings(Look::Japandi, &linen(), &[false; 6], None);
        assert_eq!(lines, vec!["Every text pair reaches 4.5:1."]);
        assert!(all_clear);
    }

    #[test]
    fn failing_pairs_are_named_and_counted() {
        let mut palette = linen();
        palette.text = palette.bg;
        palette.muted = palette.surface;
        let Warnings { lines, all_clear } = warnings(Look::Japandi, &palette, &[false; 6], None);
        assert!(
            lines[0].starts_with("Low contrast: text on bg is 1.0:1"),
            "{lines:?}"
        );
        assert_eq!(lines.len(), NAMED_FAILURES + 1);
        assert!(lines[NAMED_FAILURES].ends_with("more pairs are low too."));
        assert!(!all_clear);
    }

    /// One failing pair is one line, as the all-clear is, and still a warning. In Japandi's
    /// linen only the bulk bar's faded count fails with this text colour.
    #[test]
    fn a_single_failing_pair_is_not_the_all_clear() {
        let mut palette = linen();
        palette.text = hex(0x4b4b4b);
        let Warnings { lines, all_clear } = warnings(Look::Japandi, &palette, &[false; 6], None);
        assert_eq!(
            lines,
            vec!["Low contrast: bulk count is 4.5:1, needs 4.5:1."]
        );
        assert!(!all_clear);
    }

    #[test]
    fn bad_fields_and_a_photo_are_mentioned() {
        let mut invalid = [false; 6];
        invalid[3] = true;
        let photo = Path::new("photo.jpg");
        let Warnings { lines, all_clear } =
            warnings(Look::PixelArt, &linen(), &invalid, Some(photo));
        assert!(lines[0].starts_with("Muted text isn't a colour."));
        assert!(lines.last().unwrap().starts_with("With a photo,"));
        assert!(!all_clear);
    }

    /// With a photo, text on the window is judged over a dark and a light picture as well.
    #[test]
    fn a_photo_counts_against_text_on_the_window() {
        let mut palette = linen();
        palette.text = hex(0x555555);
        let plain = warnings(Look::Japandi, &palette, &[false; 6], None).lines;
        assert!(
            !plain.iter().any(|line| line.contains("photo")),
            "{plain:?}"
        );
        let photo = Path::new("p.jpg");
        let lines = warnings(Look::Japandi, &palette, &[false; 6], Some(photo)).lines;
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Low contrast: text on bg over a dark photo")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_dark_palette_in_a_hard_look_passes() {
        let palette = CustomPalette {
            bg: hex(0x101010),
            surface: hex(0x202020),
            text: hex(0xf0f0f0),
            muted: hex(0xb0b0b0),
            accent: hex(0xffcc00),
            accent_text: hex(0x101010),
        };
        for look in [Look::PixelArt, Look::NeoBrutalism] {
            let Warnings { lines, all_clear } = warnings(look, &palette, &[false; 6], None);
            assert_eq!(lines.len(), 1, "{look:?}: {lines:?}");
            assert!(all_clear, "{look:?}");
        }
    }
}
