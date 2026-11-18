use gpui::Context;

use super::annotation::{Annotation, Kind, Shape, Style};
use super::document::skia_color;
use super::tools::{Group, StrokeOptions, TextSize, Tool};
use super::{Editor, tools};
use crate::theme::{ActiveTheme, Family, Theme};

impl Editor {
    pub(super) fn options_for(&self, tool: Tool) -> StrokeOptions {
        self.options
            .iter()
            .find(|(t, _)| *t == tool)
            .unwrap_or(&self.options[0])
            .1
    }

    /// What the panel shows: a tool's options and its kind's groups, or the selected mark's, or
    /// those of the text being typed. `None` leaves the panel out.
    pub(super) fn panel_options(&self, theme: &Theme) -> Option<(StrokeOptions, &'static [Group])> {
        let typed = self.typing.as_ref().map(|typing| &typing.text);
        if let Some(kind) = self.tool.kind()
            && typed.is_none()
        {
            return Some((self.options_for(self.tool), tools::groups(kind)));
        }
        if !matches!(self.tool, Tool::Select | Tool::Text) {
            return None;
        }
        let annotation = match typed {
            Some(text) => text,
            None => self.selection()?.1,
        };
        Some((
            options_of(annotation, theme),
            tools::groups(annotation.shape.kind()),
        ))
    }

    /// A panel change: to the drawing tool's options, or to the selected mark as one undo step.
    pub(super) fn set_option(
        &mut self,
        cx: &mut Context<Self>,
        change: impl Fn(&mut StrokeOptions),
    ) {
        self.live_edit = false;
        self.change_option(false, cx, change);
    }

    /// A slider's change while it is dragged: the first is an undo step, the rest amend it,
    /// until `end_live_edit`.
    pub(super) fn set_option_live(
        &mut self,
        cx: &mut Context<Self>,
        change: impl Fn(&mut StrokeOptions),
    ) {
        let amend = self.live_edit;
        self.live_edit = self.change_option(amend, cx, change);
    }

    pub(super) fn end_live_edit(&mut self) {
        self.live_edit = false;
    }

    /// Changes the tool's options, the text being typed, or the selected mark: as a new undo
    /// step, or amending the last if `amend`. Text being typed or a mark takes only the one
    /// change, keeping the rest of its own style. Returns whether the mark changed under an
    /// undo step. Nothing changes while the canvas is being dragged, so the drag's mark stays
    /// where it was found.
    fn change_option(
        &mut self,
        amend: bool,
        cx: &mut Context<Self>,
        change: impl Fn(&mut StrokeOptions),
    ) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let theme = cx.theme();
        let tool_options = self.options.iter_mut().find(|(t, _)| *t == self.tool);
        let for_tool = tool_options.is_some();
        if let Some(slot) = tool_options {
            change(&mut slot.1);
        }
        if let Some(typing) = &mut self.typing {
            typing.text = restyled(&typing.text, theme, &change);
            self.preview_changed(cx);
            return false;
        }
        if for_tool {
            cx.notify();
            return false;
        }
        let Some((index, annotation)) = self.selection() else {
            return false;
        };
        let edited = restyled(annotation, theme, &change);
        let stepped = if amend {
            self.document.amend(index, edited);
            true
        } else {
            self.document.replace(index, edited)
        };
        self.document_changed(cx);
        stepped
    }
}

/// The panel's choices as a mark of `kind` is drawn with: the swatch resolved to its colour. A
/// sticker takes the look's sticker colours instead, and a badge's digits the look's.
pub(super) fn style_of(options: StrokeOptions, theme: &Theme, kind: Kind) -> Style {
    let swatches = theme.colors.stroke_swatches();
    let swatch = swatches.get(options.stroke).unwrap_or(&swatches[0]);
    let c = &theme.colors;
    let (color, second) = match kind {
        Kind::Sticker => (c.sticker, c.sticker_line),
        _ => (swatch.color, c.badge_text(swatch.color)),
    };
    Style {
        color: skia_color(color),
        second: skia_color(second),
        width: options.width,
        line: options.style,
        opacity: options.opacity,
    }
}

/// A mark's style back as the panel's choices, for showing and editing a selected mark. A
/// colour no swatch has, from another look, shows as the first; text in a face the look doesn't
/// have shows as its body face, and at the preset nearest its size.
fn options_of(annotation: &Annotation, theme: &Theme) -> StrokeOptions {
    let style = annotation.style;
    let mut options = StrokeOptions {
        stroke: theme
            .colors
            .stroke_swatches()
            .iter()
            .position(|swatch| skia_color(swatch.color) == style.color)
            .unwrap_or(0),
        width: style.width,
        style: style.line,
        opacity: style.opacity,
        ..StrokeOptions::for_kind(annotation.shape.kind(), theme)
    };
    match &annotation.shape {
        Shape::Arrow(head) => options.head = *head,
        Shape::Block(kind) => options.block = *kind,
        Shape::Text(text) => {
            options.font = if text.font.family == theme.fonts.display {
                Family::Display
            } else {
                Family::Body
            };
            options.size = TextSize::nearest(text.size, theme);
        }
        Shape::Sticker(sticker) => options.sticker = *sticker,
        _ => {}
    }
    options
}

/// `annotation` with `change` made to its panel choices, and only that: its colour, an arrow's
/// head, text's face and size, a sticker's kind, a block's kind, its width, line or opacity.
/// What the change leaves alone stays as the mark has it, even where no choice matches it, such
/// as a colour or face from another look or a text size set by hand.
fn restyled(
    annotation: &Annotation,
    theme: &Theme,
    change: impl Fn(&mut StrokeOptions),
) -> Annotation {
    let before = options_of(annotation, theme);
    let mut after = before;
    change(&mut after);
    // A colour or face no choice has shows as the first choice, and picking that one still
    // applies it, so whether the change sets them is tried from two different choices.
    let (mut first, mut second) = (before, before);
    (first.stroke, first.font) = (0, Family::Body);
    (second.stroke, second.font) = (1, Family::Display);
    change(&mut first);
    change(&mut second);
    let (recolours, refaces) = (first.stroke == second.stroke, first.font == second.font);
    let shape = match &annotation.shape {
        Shape::Arrow(_) => Shape::Arrow(after.head),
        Shape::Block(_) => Shape::Block(after.block),
        Shape::Text(text) => {
            let mut text = text.clone();
            if refaces {
                text.font.family = theme.fonts.family(after.font);
            }
            if after.size != before.size {
                text.size = after.size.pixels(theme);
            }
            Shape::Text(text)
        }
        Shape::Sticker(_) => Shape::Sticker(after.sticker),
        shape => shape.clone(),
    };
    let mut style = style_of(after, theme, shape.kind());
    if !recolours {
        style.color = annotation.style.color;
        style.second = annotation.style.second;
    }
    Annotation {
        style,
        shape,
        points: annotation.points.clone(),
    }
}

#[cfg(test)]
pub(super) mod tests {
    use gpui::point;
    use toru_core::settings::Look;

    use super::*;
    use crate::editor::Sticker;
    use crate::editor::annotation::Text;
    use crate::editor::text::Font;
    use crate::theme::Theme;

    #[test]
    fn a_style_round_trips_through_the_panels_choices() {
        let theme = Theme::new(Look::Japandi, 0);
        for stroke in 0..theme.colors.stroke_swatches().len() {
            let options = StrokeOptions {
                stroke,
                width: tools::Width::Thick,
                head: tools::Head::Double,
                style: tools::LineStyle::Dashed,
                opacity: 0.7,
                ..StrokeOptions::for_kind(Kind::Arrow, &theme)
            };
            let mark = Annotation::new(
                Shape::Arrow(options.head),
                point(0., 0.),
                style_of(options, &theme, Kind::Arrow),
            );
            assert_eq!(options_of(&mark, &theme), options);
        }
    }

    #[test]
    fn a_blocks_kind_round_trips_through_the_panels_choices() {
        let theme = Theme::new(Look::Japandi, 0);
        let options = StrokeOptions {
            block: tools::BlockKind::Solid,
            stroke: 2,
            ..StrokeOptions::for_kind(Kind::Block, &theme)
        };
        let mark = Annotation::new(
            Shape::Block(options.block),
            point(0., 0.),
            style_of(options, &theme, Kind::Block),
        );
        assert_eq!(options_of(&mark, &theme), options);
        assert_eq!(options_of(&mark, &theme).block, tools::BlockKind::Solid);
    }

    #[test]
    fn an_unknown_colour_shows_as_the_first_swatch() {
        let theme = Theme::new(Look::Japandi, 0);
        let mark = Annotation::new(
            Shape::Line,
            point(0., 0.),
            Style {
                color: tiny_skia::Color::from_rgba8(1, 2, 3, 255),
                second: tiny_skia::Color::WHITE,
                width: tools::Width::Thin,
                line: tools::LineStyle::Solid,
                opacity: 1.,
            },
        );
        let options = options_of(&mark, &theme);
        assert_eq!(options.stroke, 0);
        assert_eq!(options.head, tools::Head::Single);
    }

    pub(crate) fn text_mark(theme: &Theme, size: f32) -> Annotation {
        let options = StrokeOptions::for_kind(Kind::Text, theme);
        Annotation::placed(
            Shape::Text(Text {
                content: "Hi".into(),
                font: Font {
                    family: theme.fonts.body.clone(),
                    weight: gpui::FontWeight::BOLD,
                },
                size,
                upper: false,
            }),
            point(10., 10.),
            0.,
            style_of(options, theme, Kind::Text),
        )
    }

    #[test]
    fn text_shows_its_face_and_nearest_size_and_keeps_a_hand_set_size() {
        let theme = Theme::new(Look::Japandi, 0);
        let text = text_mark(&theme, 26.);
        let options = options_of(&text, &theme);
        assert_eq!(options.font, Family::Body);
        assert_eq!(options.size, TextSize::Medium);
        assert_eq!(options.stroke, 2);
        // Changing the colour alone leaves the 26 as it is; picking a preset sets it.
        let recoloured = restyled(&text, &theme, |o| o.stroke = 0);
        assert_eq!(recoloured.text().unwrap().size, 26.);
        assert_eq!(
            recoloured.style.color,
            skia_color(theme.colors.stroke_swatches()[0].color)
        );
        let larger = restyled(&text, &theme, |o| {
            o.size = TextSize::Huge;
            o.font = Family::Display;
        });
        assert_eq!(larger.text().unwrap().size, 48.);
        assert_eq!(larger.text().unwrap().font.family, theme.fonts.display);
        assert_eq!(options_of(&larger, &theme).font, Family::Display);
        // A sticker's options are its kind, and it is drawn in the sticker colours.
        let sticker = Annotation::placed(
            Shape::Sticker(Sticker::Cross),
            point(0., 0.),
            10.,
            style_of(options, &theme, Kind::Sticker),
        );
        assert_eq!(options_of(&sticker, &theme).sticker, Sticker::Cross);
        assert_eq!(sticker.style.color, skia_color(theme.colors.sticker));
        assert_eq!(sticker.style.second, skia_color(theme.colors.sticker_line));
        let turned = restyled(&sticker, &theme, |o| o.sticker = Sticker::Hand(3));
        assert_eq!(turned.shape, Shape::Sticker(Sticker::Hand(3)));
    }

    #[test]
    fn one_change_leaves_the_rest_of_a_marks_own_style() {
        let theme = Theme::new(Look::Japandi, 0);
        // Text in a colour and face no choice has, at a size set by hand, at half opacity.
        let mut text = text_mark(&theme, 26.);
        text.style.color = tiny_skia::Color::from_rgba8(1, 2, 3, 255);
        text.style.opacity = 0.5;
        if let Shape::Text(t) = &mut text.shape {
            t.font.family = "Some Other Face".into();
        }
        let bigger = restyled(&text, &theme, |o| o.size = TextSize::Huge);
        assert_eq!(bigger.text().unwrap().size, 48.);
        assert_eq!(bigger.style.color, text.style.color);
        assert_eq!(bigger.style.opacity, 0.5);
        assert_eq!(bigger.text().unwrap().font.family, "Some Other Face");
        let recoloured = restyled(&text, &theme, |o| o.stroke = 1);
        assert_eq!(
            recoloured.style.color,
            skia_color(theme.colors.stroke_swatches()[1].color)
        );
        assert_eq!(recoloured.text().unwrap().size, 26.);
        assert_eq!(recoloured.text().unwrap().font.family, "Some Other Face");
        // The colour and face shown for ones no choice has can still be picked.
        let picked = restyled(&text, &theme, |o| {
            o.stroke = 0;
            o.font = Family::Body;
        });
        assert_eq!(
            picked.style.color,
            skia_color(theme.colors.stroke_swatches()[0].color)
        );
        assert_eq!(picked.text().unwrap().font.family, theme.fonts.body);
    }
}
