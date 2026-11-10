//! The editor's frame around the canvas (DESIGN.md §5): the top bar, the floating tool pill, the
//! zoom and undo corners and the hint chip.

use gpui::{
    AnyElement, Context, Div, Hsla, SharedString, Stateful, Window, div, prelude::*, px, relative,
};

use toru_core::settings::SaveFormat;

use super::{Editor, act, on, tools::*};
use crate::theme::{ActiveTheme, Theme, ThemedText, contrast};
use crate::ui::{Button, Choices, Icon, Menu, Pressable, Readable, TextField, shadowed};

/// The pill's top edge and the corners' distance from the canvas edges.
pub const EDGE: f32 = 16.;
const TOOL: f32 = 40.;
const PILL_PADDING: f32 = 4.;
const PILL_GAP: f32 = 2.;
const SEPARATOR_HEIGHT: f32 = 28.;
const SEPARATOR_MARGIN: f32 = 4.;
/// `.tool .k`: the shortcut letter's distance from the tool's right and bottom edges, and how
/// much of the tool's text colour it keeps.
const KEY_RIGHT: f32 = 4.;
const KEY_BOTTOM: f32 = 2.;
const KEY_OPACITY: f32 = 0.8;
pub const CORNER_BUTTON: f32 = 32.;
const CORNER_PADDING: f32 = 4.;
const ZOOM_READOUT_MIN_WIDTH: f32 = 56.;
pub const HINT_BOTTOM: f32 = 24.;
const NAME_WIDTH: f32 = 240.;

/// The pill's height, for laying the picture out under it.
pub const PILL_HEIGHT: f32 = TOOL + PILL_PADDING * 2.;
/// A corner's height, for laying the picture out above it.
pub const CORNER_HEIGHT: f32 = CORNER_BUTTON + CORNER_PADDING * 2.;

pub fn top_bar(editor: &Editor, window: &Window, cx: &Context<Editor>) -> Div {
    let theme = cx.theme();
    let c = &theme.colors;
    let hair = theme.borders.hair;
    let save_as = Menu::new(
        "save-as",
        "Save as",
        Button::new("save-as-button", "Save as ▾").named("Save as"),
    )
    .header("Save a copy as")
    .item("PNG…", on(cx, |this, cx| this.save_as(SaveFormat::Png, cx)))
    .item(
        "JPEG…",
        on(cx, |this, cx| this.save_as(SaveFormat::Jpg, cx)),
    )
    .item(
        "WebP…",
        on(cx, |this, cx| this.save_as(SaveFormat::WebP, cx)),
    )
    .align_right();
    let _ = window;
    div()
        .flex_none()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(12.))
        .bg(c.surface_2)
        .border_b(hair.width)
        .border_color(hair.color)
        .child(
            Button::new("back", "Back to session")
                .icon(Icon::Back)
                .track_focus(&editor.back)
                .on_click(on(cx, |this, cx| this.back(cx))),
        )
        .child(TextField::new(&editor.name, "Screenshot name").w(px(NAME_WIDTH)))
        // A refused name is said here in place of the status, right after its field, so the bar
        // keeps its shape, as the card does.
        .child(match &editor.name_error {
            Some(error) => status_text(theme, c.text)
                .read_as_text(error)
                .child(error.clone()),
            None => {
                let status = editor.status();
                status_text(theme, c.muted)
                    .read_as_text(&status)
                    .child(status)
            }
        })
        .child(div().flex_1().min_w(px(40.)))
        .child(Button::new("reset", "Reset edits").on_click(on(cx, |this, cx| this.reset(cx))))
        .child(
            Button::new("copy", "Copy")
                .icon(Icon::Copy)
                .check_mark(editor.copied.get().is_some())
                .on_click(on(cx, |this, cx| this.copy(cx))),
        )
        .child(
            Button::new("save", "Save")
                .primary()
                .on_click(on(cx, |this, cx| this.save(cx))),
        )
        .child(save_as)
}

fn status_text(theme: &Theme, color: Hsla) -> Stateful<Div> {
    div()
        .id("status")
        .type_style(theme, theme.type_scale.hint)
        .text_color(color)
        .whitespace_nowrap()
}

/// The tool pill, top-centre. It is one tab stop: the arrow keys move between tools.
pub fn tool_pill(editor: &Editor, cx: &Context<Editor>) -> impl IntoElement {
    let theme = cx.theme();
    let tools: Vec<Tool> = Tool::all().collect();
    let chosen = tools
        .iter()
        .position(|tool| *tool == editor.tool)
        .unwrap_or(0);
    // A rule before each group but the first; drawn as a left margin on the group's first tool.
    let starts_group: Vec<bool> = GROUPS
        .iter()
        .enumerate()
        .flat_map(|(g, group)| group.iter().enumerate().map(move |(i, _)| g > 0 && i == 0))
        .collect();
    let choices = Choices::new(
        "tools",
        "Tools",
        tools.len(),
        chosen,
        move |i, on, theme| tool_button(theme, tools[i], i, on, starts_group[i]),
    )
    .gap(px(PILL_GAP))
    .on_change(act(cx, move |this, i, cx| {
        this.set_tool(Tool::all().nth(i).unwrap_or(Tool::Select), cx)
    }));
    let pill = div().id("pill").occlude();
    shadowed(pill, &theme.shadows.pop, theme.radii.lg)
        .relative()
        .flex_none()
        .p(px(PILL_PADDING))
        .bg(theme.colors.surface)
        .border(theme.borders.pop.width)
        .border_color(theme.borders.pop.color)
        .rounded(theme.radii.lg)
        .child(choices)
}

/// `.tool`: the icon with its shortcut letter in the corner; the chosen one is filled.
fn tool_button(
    theme: &Theme,
    tool: Tool,
    index: usize,
    on: bool,
    separator_before: bool,
) -> Pressable {
    let c = &theme.colors;
    let (bg, text) = if on {
        (Some(c.on_bg), c.on_text)
    } else {
        (None, c.text)
    };
    let key_style = theme.type_scale.tool_key;
    let key_color = tool_key_color(theme, on);
    let mut button = Pressable::new(("tool", index), tool.aria_label(), text)
        .radius(theme.radii.md)
        .size(px(TOOL))
        .child(tool.icon().render(theme, text))
        .child(
            div()
                .absolute()
                .right(px(KEY_RIGHT))
                .bottom(px(KEY_BOTTOM))
                .type_style(theme, key_style)
                .line_height(key_style.size)
                .text_color(key_color)
                .child(tool.key()),
        );
    if let Some(bg) = bg {
        button = button.bg(bg);
    }
    if separator_before {
        // `.sep`: a 1px rule 28px tall with 4px either side, plus the pill's 2px gap.
        button = button.child(
            div()
                .absolute()
                .left(px(-(SEPARATOR_MARGIN + 1. + PILL_GAP)))
                .top(px((TOOL - SEPARATOR_HEIGHT) / 2.))
                .w(px(1.))
                .h(px(SEPARATOR_HEIGHT))
                .bg(c.hair),
        );
        button = button.ml(px(SEPARATOR_MARGIN * 2. + 1.));
    }
    button
}

/// The shortcut letter's colour on a tool, chosen (`on`) or not. The mockups fade the letter to
/// 80% of the tool's text; on the chosen tool's fill that falls short of §7's 4.5:1 in some
/// palettes, so the chosen letter keeps the full `on_text`. The contrast test covers both.
pub fn tool_key_color(theme: &Theme, on: bool) -> Hsla {
    let c = &theme.colors;
    if on {
        return c.on_text;
    }
    contrast::over(
        Hsla {
            a: KEY_OPACITY,
            ..c.text
        },
        c.surface,
    )
}

/// Zoom (− / % / + / Fit), bottom-left.
pub fn zoom_corner(editor: &Editor, cx: &Context<Editor>) -> Stateful<Div> {
    let theme = cx.theme();
    let percent: SharedString = format!("{}%", editor.viewport.percent()).into();
    corner(theme, "zoom")
        .left(px(EDGE))
        .child(
            corner_button(theme, "zoom-out", "Zoom out")
                .child("−")
                .on_click(on(cx, |this, cx| this.zoom_step(-1, cx))),
        )
        .child(
            corner_button(
                theme,
                "zoom-level",
                format!("Zoom {percent}, press for 100%"),
            )
            .min_w(px(ZOOM_READOUT_MIN_WIDTH))
            .child(percent)
            .on_click(on(cx, |this, cx| this.zoom_to(1., cx))),
        )
        .child(
            corner_button(theme, "zoom-in", "Zoom in")
                .child("+")
                .on_click(on(cx, |this, cx| this.zoom_step(1, cx))),
        )
        .child(
            div()
                .flex_none()
                .mx(px(SEPARATOR_MARGIN))
                .w(px(1.))
                .h(px(SEPARATOR_HEIGHT))
                .bg(theme.colors.hair),
        )
        .child(
            corner_button(theme, "zoom-fit", "Fit")
                .child(theme.type_scale.corner.case.apply("Fit"))
                .on_click(on(cx, |this, cx| this.zoom_fit(cx))),
        )
}

/// Undo and Redo, bottom-right.
pub fn undo_corner(editor: &Editor, cx: &Context<Editor>) -> Stateful<Div> {
    let theme = cx.theme();
    let c = &theme.colors;
    let faded = |can: bool| if can { c.text } else { c.muted };
    corner(theme, "history")
        .right(px(EDGE))
        .child(
            corner_button(theme, "undo", "Undo")
                .child(Icon::Undo.render(theme, faded(editor.document.can_undo())))
                .on_click(on(cx, |this, cx| this.undo(cx))),
        )
        .child(
            corner_button(theme, "redo", "Redo")
                .child(Icon::Redo.render(theme, faded(editor.document.can_redo())))
                .on_click(on(cx, |this, cx| this.redo(cx))),
        )
}

/// `.corner`: a small pop surface holding borderless buttons.
fn corner(theme: &Theme, id: &'static str) -> Stateful<Div> {
    let pop = theme.borders.pop;
    let corner = div().id(id).occlude();
    shadowed(corner, &theme.shadows.pop, theme.radii.md)
        .absolute()
        .bottom(px(EDGE))
        .flex()
        .items_center()
        .gap(px(PILL_GAP))
        .p(px(CORNER_PADDING))
        .bg(theme.colors.surface)
        .border(pop.width)
        .border_color(pop.color)
        .rounded(theme.radii.md)
}

fn corner_button(theme: &Theme, id: &'static str, label: impl Into<SharedString>) -> Pressable {
    Pressable::new(id, label, theme.colors.text)
        .radius(theme.radii.sm)
        .h(px(CORNER_BUTTON))
        .min_w(px(CORNER_BUTTON))
        .px(px(8.))
        .type_style(theme, theme.type_scale.corner)
}

/// The hint chip, bottom-centre: what the active tool does.
pub fn hint_chip(editor: &Editor, cx: &Context<Editor>) -> AnyElement {
    let theme = cx.theme();
    let pop = theme.borders.pop;
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(HINT_BOTTOM))
        .flex()
        .justify_center()
        .child(
            div()
                .id("hint")
                .read_as_text(editor.hint())
                .occlude()
                .max_w(relative(0.8))
                .px(px(12.))
                .py(px(6.))
                .bg(theme.colors.surface)
                .text_color(theme.colors.muted)
                .border(pop.width)
                .border_color(pop.color)
                .rounded(theme.radii.md)
                .shadow(theme.shadows.pop.clone())
                .type_style(theme, theme.type_scale.hint)
                .child(editor.hint()),
        )
        .into_any_element()
}
