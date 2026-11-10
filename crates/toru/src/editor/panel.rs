//! The left context panel (DESIGN.md §5): the active tool's options in labelled groups. A
//! tool shows the groups its kind of mark needs; Select shows the selected mark's, and changing
//! one changes the mark; Crop shows its rectangle's size with the buttons that apply or cancel
//! it.

use gpui::{
    AnyElement, Context, Div, ElementId, Hsla, Pixels, SharedString, div, img, prelude::*, px,
};
use gpui_kit::component::scroll::ScrollableElement;

use super::{Editor, TRAY_ICON, act, chrome, on, sticker::Sticker, tools::*};
use crate::theme::{ActiveTheme, Family, Theme, ThemedText};
use crate::ui::{Button, Choices, MIN_TARGET, Pressable, Readable, Slider};

pub const WIDTH: f32 = 200.;
pub const LEFT: f32 = 16.;
pub const TOP: f32 = 84.;
/// The least the panel may be given when the canvas is short: enough to show a group's label
/// and its control, so scrolling always has something to land on.
const MIN_HEIGHT: f32 = 120.;
const PADDING: f32 = 14.;
const GROUP_GAP: f32 = 14.;
const LABEL_GAP: f32 = 11.;
const SWATCH: f32 = 24.;
const SWATCH_GAP: f32 = 8.;
/// The chosen swatch's outline: this wide, and this far out.
const SWATCH_RING: f32 = 2.;
const OPTION_HEIGHT: f32 = 28.;
const OPTION_GAP: f32 = 6.;
/// The mockups draw swatches and options under §7's 32px target, so each is pressable this far
/// past its box: half the gap to its neighbour, so neither reaches into the other.
const SWATCH_HIT: f32 = (MIN_TARGET - SWATCH) / 2.;
const OPTION_HIT: f32 = (MIN_TARGET - OPTION_HEIGHT) / 2.;
const WIDTH_BAR: f32 = 14.;
/// A sticker's tile in the tray: §7's target, with the sticker drawn `TRAY_ICON` big in it.
const STICKER_TILE: f32 = MIN_TARGET;
/// The font options, in the panel's order.
const FAMILIES: [Family; 2] = [Family::Display, Family::Body];

/// The panel for the active tool, or nothing for a tool with no options yet. It is as tall as
/// its groups need, up to the room above the zoom corner, and scrolls past that; a control that
/// takes keyboard focus below the fold is scrolled to.
pub fn panel(editor: &Editor, cx: &Context<Editor>) -> Option<AnyElement> {
    let theme = cx.theme();
    let groups: Vec<Div> = if editor.tool == Tool::Crop {
        vec![crop_group(editor, theme, cx)]
    } else {
        let (options, shown) = editor.panel_options(theme)?;
        shown
            .iter()
            // A block's Fill is its solid colour, so it shows only while the block is solid.
            .filter(|group| **group != Group::Fill || options.block == BlockKind::Solid)
            .map(|group| stroke_group(*group, options, editor, theme, cx))
            .collect()
    };
    let pop = theme.borders.pop;
    let content = div()
        .id("panel-scroll")
        .track_focus(editor.panel_focus.focus())
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&editor.panel_scroll)
        .p(px(PADDING))
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .children(groups);
    Some(
        div()
            .id("panel")
            .occlude()
            .absolute()
            .left(px(LEFT))
            .top(px(TOP))
            .w(px(WIDTH))
            .max_h(max_height(editor.canvas.size.height))
            .flex()
            .flex_col()
            .bg(theme.colors.surface)
            .border(pop.width)
            .border_color(pop.color)
            .rounded(theme.radii.lg)
            .shadow(theme.shadows.pop.clone())
            .child(content)
            .vertical_scrollbar(&editor.panel_scroll)
            .into_any_element(),
    )
}

/// The tallest the panel may be on a canvas `canvas_height` tall: from its top down to the gap
/// above the zoom corner, and never less than `MIN_HEIGHT`.
fn max_height(canvas_height: Pixels) -> Pixels {
    let room = canvas_height - px(TOP + chrome::EDGE * 2. + chrome::CORNER_HEIGHT);
    room.max(px(MIN_HEIGHT))
}

/// One of the panel's groups, showing `options` and changing them through the editor.
fn stroke_group(
    which: Group,
    options: StrokeOptions,
    editor: &Editor,
    theme: &Theme,
    cx: &Context<Editor>,
) -> Div {
    let control: AnyElement = match which {
        Group::Stroke | Group::Fill => {
            let swatch_count = theme.colors.stroke_swatches().len();
            Choices::new(
                which.label(),
                which.label(),
                swatch_count,
                options.stroke,
                |i, on, theme| swatch(theme, i, on),
            )
            .gap(px(SWATCH_GAP))
            .on_change(act(cx, |this, i, cx| this.set_option(cx, |o| o.stroke = i)))
            .into_any_element()
        }
        Group::Block => Choices::new(
            "block",
            "Block",
            BlockKind::ALL.len(),
            index_of(&BlockKind::ALL, options.block),
            |i, on, theme| {
                let kind = BlockKind::ALL[i];
                let label = theme.type_scale.option.case.apply(kind.label());
                option(theme, ("block", i), kind.label(), on).child(label)
            },
        )
        .flex_wrap()
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.block = BlockKind::ALL[i])
        }))
        .into_any_element(),
        Group::Width => Choices::new(
            "width",
            "Width",
            Width::ALL.len(),
            index_of(&Width::ALL, options.width),
            |i, on, theme| {
                let width = Width::ALL[i];
                option(theme, ("width", i), width.label(), on).child(
                    div()
                        .w(px(WIDTH_BAR))
                        .h(px(width.pixels()))
                        .bg(option_text(theme, on)),
                )
            },
        )
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.width = Width::ALL[i])
        }))
        .into_any_element(),
        Group::Head => Choices::new(
            "head",
            "Arrow head",
            Head::ALL.len(),
            index_of(&Head::ALL, options.head),
            |i, on, theme| {
                let head = Head::ALL[i];
                option(theme, ("head", i), head.label(), on).child(head.glyph())
            },
        )
        .flex_wrap()
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.head = Head::ALL[i])
        }))
        .into_any_element(),
        Group::Style => Choices::new(
            "style",
            "Style",
            LineStyle::ALL.len(),
            index_of(&LineStyle::ALL, options.style),
            |i, on, theme| {
                let style = LineStyle::ALL[i];
                let label = theme.type_scale.option.case.apply(style.label());
                option(theme, ("style", i), style.label(), on).child(label)
            },
        )
        // The larger-type looks can't fit three words on one line of the panel.
        .flex_wrap()
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.style = LineStyle::ALL[i])
        }))
        .into_any_element(),
        // Each face is named in itself, so the choice shows what it does, at the label size:
        // the display faces are wide, and the panel is narrow.
        Group::Font => Choices::new(
            "font",
            "Font",
            FAMILIES.len(),
            index_of(&FAMILIES, options.font),
            |i, on, theme| {
                let name = theme.fonts.family(FAMILIES[i]);
                option(theme, ("font", i), name.clone(), on).child(
                    div()
                        .font_family(name.clone())
                        .text_size(theme.type_scale.label.size)
                        .child(name),
                )
            },
        )
        .flex_wrap()
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.font = FAMILIES[i])
        }))
        .into_any_element(),
        Group::Size => Choices::new(
            "size",
            "Size",
            TextSize::ALL.len(),
            index_of(&TextSize::ALL, options.size),
            |i, on, theme| {
                let size = TextSize::ALL[i];
                option(theme, ("size", i), size.label(), on).child(size.glyph())
            },
        )
        .gap(px(OPTION_GAP))
        .on_change(act(cx, |this, i, cx| {
            this.set_option(cx, |o| o.size = TextSize::ALL[i])
        }))
        .into_any_element(),
        Group::Sticker => {
            let images: Vec<_> = (0..Sticker::ALL.len())
                .map(|i| editor.tray_image(i))
                .collect();
            Choices::new(
                "sticker",
                "Sticker",
                Sticker::ALL.len(),
                index_of(&Sticker::ALL, options.sticker),
                move |i, on, theme| {
                    let tile = option(theme, ("sticker", i), Sticker::ALL[i].label(), on)
                        .size(px(STICKER_TILE))
                        .px(px(0.));
                    match &images[i] {
                        Some(image) => {
                            tile.child(img(image.clone()).size(px(TRAY_ICON)).flex_none())
                        }
                        None => tile,
                    }
                },
            )
            .flex_wrap()
            .gap(px(OPTION_GAP))
            .on_change(act(cx, |this, i, cx| {
                this.set_option(cx, |o| o.sticker = Sticker::ALL[i])
            }))
            .into_any_element()
        }
        // A drag of the slider is one undo step on a selected mark, however many moves it has.
        Group::Opacity => Slider::new("opacity", "Opacity", options.opacity)
            .on_change(act(cx, |this, value, cx| {
                this.set_option_live(cx, |o| o.opacity = value)
            }))
            .on_commit(on(cx, |this, _| this.end_live_edit()))
            .into_any_element(),
    };
    group(theme, which.label(), control)
}

/// The Crop tool's group: the size the picture will be, and the buttons that crop it or go
/// back to Select. Enter and Esc do the same.
fn crop_group(editor: &Editor, theme: &Theme, cx: &Context<Editor>) -> Div {
    let size = match editor.crop_rect() {
        Some(rect) => format!("{} × {}", rect.width(), rect.height()),
        None => "Drag out the part to keep".to_string(),
    };
    let content = div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .child(
            div()
                .id("crop-size")
                .read_as_text(&size)
                .type_style(theme, theme.type_scale.hint)
                .text_color(theme.colors.muted)
                .child(size),
        )
        .child(
            div()
                .flex()
                .gap(px(OPTION_GAP))
                .child(
                    Button::new("crop-apply", "Crop")
                        .track_focus(&editor.crop_button)
                        .primary()
                        .small()
                        .on_click(on(cx, |this, cx| this.apply_crop(cx))),
                )
                .child(
                    Button::new("crop-cancel", "Cancel")
                        .small()
                        .on_click(on(cx, |this, cx| this.set_tool(Tool::Select, cx))),
                ),
        );
    group(theme, "Crop", content)
}

/// `.group`: a label over its control.
fn group(theme: &Theme, label: &str, content: impl IntoElement) -> Div {
    let style = theme.type_scale.label;
    div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .child(
            div()
                .type_style(theme, style)
                .text_color(theme.colors.label)
                .child(style.case.apply(label)),
        )
        .child(content)
}

/// `.sw`: a colour square; chosen, it has an outline in the look's swatch colour.
fn swatch(theme: &Theme, index: usize, on: bool) -> Pressable {
    let swatch = theme.colors.stroke_swatches()[index];
    let border = theme.borders.input;
    let radius = theme.radii.sm;
    // Absolute children sit inside the border, so the ring's inset adds it back.
    let out = px(-(SWATCH_RING * 2.)) - border.width;
    Pressable::new(("swatch", index), swatch.name, theme.colors.text)
        .bg(swatch.color)
        .border(border)
        .radius(radius)
        .no_hover()
        .hit_margin(px(SWATCH_HIT))
        .size(px(SWATCH))
        .when(on, |this| {
            this.child(
                div()
                    .absolute()
                    .top(out)
                    .left(out)
                    .right(out)
                    .bottom(out)
                    .rounded(if radius > px(0.) {
                        radius + px(SWATCH_RING * 2.)
                    } else {
                        px(0.)
                    })
                    .border(px(SWATCH_RING))
                    .border_color(theme.colors.swatch_on),
            )
        })
}

/// `.opt`: a small bordered option; chosen, it takes the accent.
fn option(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> Pressable {
    let c = &theme.colors;
    let (bg, border) = if on {
        (c.accent_bg, theme.borders.accent)
    } else {
        (c.input_bg, theme.borders.input)
    };
    Pressable::new(id, label, option_text(theme, on))
        .bg(bg)
        .border(border)
        .radius(theme.radii.sm)
        .hit_margin(px(OPTION_HIT))
        .h(px(OPTION_HEIGHT))
        .min_w(px(OPTION_HEIGHT))
        .px(px(8.))
        .type_style(theme, theme.type_scale.option)
}

fn option_text(theme: &Theme, on: bool) -> Hsla {
    if on {
        theme.colors.accent_text
    } else {
        theme.colors.text
    }
}

fn index_of<T: PartialEq>(all: &[T], value: T) -> usize {
    all.iter().position(|item| *item == value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_stops_above_the_zoom_corner_and_keeps_a_usable_minimum() {
        // The mockups' canvas: room from 84 down to 16 + 40 + 16 above the bottom.
        let corner = chrome::EDGE * 2. + chrome::CORNER_HEIGHT;
        assert_eq!(max_height(px(800.)), px(800. - TOP - corner));
        // A canvas at the window's minimum height still leaves a panel worth scrolling.
        assert_eq!(max_height(px(150.)), px(MIN_HEIGHT));
    }
}
