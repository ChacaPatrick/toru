use gpui::{
    Context, CursorStyle, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, point,
};

use super::Editor;
use super::annotation::{Annotation, Badge, HandleIndex, Kind, Shape, Text, in_picture};
use super::canvas::HANDLE_SIZE;
use super::text::Font;
use super::tool_options::style_of;
use super::tools::{StrokeOptions, Tool};
use crate::theme::{ActiveTheme, Theme};

/// How far from a mark's stroke a click still selects it, in logical pixels on the canvas.
const HIT_TOLERANCE: f32 = 6.;

const HANDLE_HIT: f32 = HANDLE_SIZE / 2. + 4.;

/// The mockup's 28px badge on 14px digits, and its 64px sticker beside 16px text.
const BADGE_TO_DIGITS_RATIO: f32 = 2.;

const STICKER_TO_TEXT_RATIO: f32 = 4.;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Grip {
    /// The pointer's start, in picture pixels.
    Body(Point<f32>),
    Handle(HandleIndex),
}

pub(super) enum Drag {
    /// The pointer's last position.
    Pan(Point<Pixels>),
    Draw(Annotation),
    Edit {
        index: usize,
        grip: Grip,
        original: Annotation,
        current: Annotation,
    },
    /// `Editor::crop_draft` has the rectangle as it is under the pointer.
    Crop {
        grip: Grip,
        original: Annotation,
    },
    /// Whether a mark has gone yet, so the next folds into the same undo step, and the pointer's
    /// last position in picture pixels, so a fast sweep misses nothing between.
    Erase {
        removed: bool,
        last: Point<f32>,
    },
}

impl Editor {
    /// What the eraser took stays taken.
    pub(super) fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        match self.drag.take() {
            Some(Drag::Edit { .. }) => self.document_changed(cx),
            Some(Drag::Crop { original, .. }) => {
                self.crop_draft = original.is_drawn().then_some(original);
                cx.notify();
            }
            Some(_) => self.preview_changed(cx),
            None => {}
        }
    }

    pub(super) fn placed(&self, tool: Tool, at: Point<f32>, theme: &Theme) -> Option<Annotation> {
        let options = self.options_for(tool);
        let scale = &theme.type_scale;
        let font = |style: crate::theme::TypeStyle| Font {
            family: theme.fonts.family(style.family),
            weight: style.weight,
        };
        let (shape, side) = match tool {
            Tool::Text => (
                Shape::Text(Text {
                    content: String::new(),
                    font: Font {
                        family: theme.fonts.family(options.font),
                        weight: scale.anno_text.weight,
                    },
                    size: options.size.pixels(theme),
                    upper: StrokeOptions::upper(theme),
                }),
                0.,
            ),
            Tool::Number => (
                Shape::Number(Badge {
                    number: next_number(self.document.annotations()),
                    diameter: f32::from(scale.anno_number.size) * BADGE_TO_DIGITS_RATIO,
                    font: font(scale.anno_number),
                }),
                0.,
            ),
            Tool::Stickers => (
                Shape::Sticker(options.sticker),
                f32::from(scale.anno_text.size) * STICKER_TO_TEXT_RATIO,
            ),
            _ => return None,
        };
        let style = style_of(options, theme, shape.kind());
        Some(Annotation::placed(shape, at, side, style))
    }

    /// Only on the shown picture: a mark hidden by the crop, or the part of one past it, can't be
    /// picked from the margin.
    fn mark_at(&self, at: Point<f32>) -> Option<usize> {
        self.shown_bounds()
            .contains(&at)
            .then(|| self.document.hit_test(at, self.hit_tolerance()))
            .flatten()
    }

    /// Sampled a tolerance apart, so a quick sweep across a thin line still takes it.
    fn erase(&mut self, from: Point<f32>, to: Point<f32>, cx: &mut Context<Self>) {
        let tolerance = self.hit_tolerance();
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let steps = ((dx * dx + dy * dy).sqrt() / tolerance).ceil().max(1.) as usize;
        let mut erased = false;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let at = point(from.x + dx * t, from.y + dy * t);
            while let Some(index) = self.mark_at(at) {
                let amend = match &mut self.drag {
                    Some(Drag::Erase { removed, .. }) => std::mem::replace(removed, true),
                    _ => false,
                };
                self.document.remove(index, amend);
                erased = true;
            }
        }
        if erased {
            self.document_changed(cx);
        }
    }

    /// In picture pixels.
    fn hit_tolerance(&self) -> f32 {
        HIT_TOLERANCE / self.viewport.zoom * self.scale
    }

    fn handle_at(&self, position: Point<Pixels>) -> Option<(usize, HandleIndex, &Annotation)> {
        let (index, annotation) = self.selection()?;
        Some((index, self.handle_on(annotation, position)?, annotation))
    }

    fn handle_on(&self, mark: &Annotation, position: Point<Pixels>) -> Option<HandleIndex> {
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        mark.handles().into_iter().position(|handle| {
            let at = self.canvas_point(handle);
            (at.x - on_canvas.x).abs() <= HANDLE_HIT && (at.y - on_canvas.y).abs() <= HANDLE_HIT
        })
    }

    /// Returns whether a drag began, so the canvas keeps the pointer for its length.
    pub(super) fn mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let at = self.image_point(event.position);
        // A press away from the text being typed only finishes it.
        if let Some(typing) = &self.typing {
            if !typing.text.hit(at, self.hit_tolerance()) {
                self.finish_typing(cx);
            }
            return false;
        }
        let options = self.options_for(self.tool);
        let drag = if self.tool == Tool::Pan {
            Drag::Pan(event.position)
        } else if let Some(shape) = self.tool.shape(&options) {
            self.select(None, cx);
            let style = style_of(options, cx.theme(), shape.kind());
            let at = in_picture(self.shown_bounds(), at);
            Drag::Draw(Annotation::new(shape, at, style))
        } else if self.tool == Tool::Text {
            let on_text = self
                .mark_at(at)
                .filter(|index| self.document.annotations()[*index].text().is_some());
            // A click off the picture begins nothing: text there would not show.
            match on_text {
                Some(index) => self.edit_text(index, cx),
                None if !self.shown_bounds().contains(&at) => {}
                None => {
                    if let Some(text) = self.placed(Tool::Text, at, cx.theme()) {
                        self.start_typing(None, text, cx);
                    }
                }
            }
            return false;
        } else if matches!(self.tool, Tool::Number | Tool::Stickers) {
            let at = in_picture(self.shown_bounds(), at);
            if let Some(mark) = self.placed(self.tool, at, cx.theme()) {
                self.document.add(mark);
                self.document_changed(cx);
            }
            return false;
        } else if self.tool == Tool::Crop {
            let draft = self.crop_draft.clone();
            let handle = draft
                .as_ref()
                .and_then(|draft| self.handle_on(draft, event.position));
            match (draft, handle) {
                (Some(original), Some(handle)) => Drag::Crop {
                    grip: Grip::Handle(handle),
                    original,
                },
                (Some(original), None) if original.bounds().contains(&at) => Drag::Crop {
                    grip: Grip::Body(at),
                    original,
                },
                _ => {
                    // Handle 3 is the far corner, which follows the pointer.
                    let at = in_picture(self.shown_bounds(), at);
                    let style = style_of(options, cx.theme(), Kind::Rectangle);
                    let fresh = Annotation::new(Shape::Rectangle, at, style);
                    self.crop_draft = Some(fresh.clone());
                    Drag::Crop {
                        grip: Grip::Handle(3),
                        original: fresh,
                    }
                }
            }
        } else if self.tool == Tool::Eraser {
            self.drag = Some(Drag::Erase {
                removed: false,
                last: at,
            });
            self.erase(at, at, cx);
            return true;
        } else if self.tool == Tool::Select {
            if let Some((index, handle, annotation)) = self.handle_at(event.position) {
                Drag::Edit {
                    index,
                    grip: Grip::Handle(handle),
                    original: annotation.clone(),
                    current: annotation.clone(),
                }
            } else if let Some(index) = self.mark_at(at) {
                self.select(Some(index), cx);
                let annotation = self.document.annotations()[index].clone();
                if event.click_count == 2 && annotation.text().is_some() {
                    self.edit_text(index, cx);
                    return false;
                }
                Drag::Edit {
                    index,
                    grip: Grip::Body(at),
                    original: annotation.clone(),
                    current: annotation,
                }
            } else {
                self.select(None, cx);
                return false;
            }
        } else {
            return false;
        };
        // The picture is rendered without the mark Select took, so the drag's overlay can put
        // it back wherever the pointer goes.
        let edit = matches!(drag, Drag::Edit { .. });
        self.drag = Some(drag);
        if edit {
            self.document_changed(cx);
        } else {
            self.preview_changed(cx);
        }
        true
    }

    pub(super) fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            self.mouse_up(cx);
            return;
        }
        let at = self.image_point(event.position);
        let shift = event.modifiers.shift;
        let picture = self.shown_bounds();
        // A mark may reach past the crop, so a move holds it within the whole picture, not the
        // shown part: a nudge is a nudge, never a jump to the crop's edge.
        let whole = self.document.size();
        match self.drag.as_mut() {
            Some(Drag::Pan(last)) => {
                let delta = (event.position - *last).map(f32::from);
                *last = event.position;
                self.viewport = self
                    .viewport
                    .panned(delta)
                    .clamped(self.picture(), self.canvas_size());
                cx.notify();
            }
            Some(Drag::Draw(annotation)) => {
                annotation.extend(at, shift, picture);
                self.preview_changed(cx);
            }
            Some(Drag::Edit {
                grip,
                original,
                current,
                ..
            }) => {
                *current = match *grip {
                    Grip::Body(start) => original.moved(at - start, shift, whole),
                    Grip::Handle(handle) => original.resized(handle, at, shift, picture),
                };
                self.preview_changed(cx);
            }
            Some(Drag::Crop { grip, original }) => {
                self.crop_draft = Some(match *grip {
                    Grip::Body(start) => original.moved(at - start, shift, whole),
                    Grip::Handle(handle) => original.resized(handle, at, shift, picture),
                });
                cx.notify();
            }
            Some(Drag::Erase { last, .. }) => {
                let from = std::mem::replace(last, at);
                self.erase(from, at, cx);
            }
            None => {}
        }
    }

    pub(super) fn mouse_up(&mut self, cx: &mut Context<Self>) {
        match self.drag.take() {
            Some(Drag::Draw(annotation)) => {
                if annotation.is_drawn() && self.shows(&annotation) {
                    self.document.add(annotation);
                }
                self.document_changed(cx);
            }
            Some(Drag::Edit { index, current, .. }) => {
                self.document.replace(index, current);
                self.document_changed(cx);
            }
            Some(Drag::Crop { .. }) => {
                if self
                    .crop_draft
                    .as_ref()
                    .is_some_and(|draft| !draft.is_drawn())
                {
                    self.crop_draft = None;
                }
                cx.notify();
            }
            Some(Drag::Pan(_) | Drag::Erase { .. }) => cx.notify(),
            None => {}
        }
    }

    pub(super) fn cursor(&self) -> Option<CursorStyle> {
        match self.tool {
            Tool::Pan => Some(if matches!(self.drag, Some(Drag::Pan(_))) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            }),
            Tool::Text => Some(CursorStyle::IBeam),
            Tool::Crop | Tool::Eraser => Some(CursorStyle::Crosshair),
            _ => self.tool.kind().map(|_| CursorStyle::Crosshair),
        }
    }
}

/// One past the highest on the picture, so a badge deleted or undone frees nothing below the rest.
fn next_number(annotations: &[Annotation]) -> u32 {
    annotations
        .iter()
        .filter_map(|annotation| match &annotation.shape {
            Shape::Number(badge) => Some(badge.number),
            _ => None,
        })
        .max()
        .unwrap_or(0)
        + 1
}

#[cfg(test)]
mod tests {
    use toru_core::settings::Look;

    use super::*;
    use crate::editor::tool_options::style_of;
    use crate::editor::tool_options::tests::text_mark;
    use crate::theme::Theme;

    #[test]
    fn badges_count_up_past_the_highest_there() {
        let theme = Theme::new(Look::Japandi, 0);
        let badge = |number| {
            Annotation::placed(
                Shape::Number(Badge {
                    number,
                    diameter: 28.,
                    font: Font {
                        family: theme.fonts.body.clone(),
                        weight: gpui::FontWeight::BOLD,
                    },
                }),
                point(0., 0.),
                0.,
                style_of(
                    StrokeOptions::for_kind(Kind::Number, &theme),
                    &theme,
                    Kind::Number,
                ),
            )
        };
        assert_eq!(next_number(&[]), 1);
        assert_eq!(
            next_number(&[badge(1), text_mark(&theme, 16.), badge(2)]),
            3
        );
        // With 2 deleted, the next is still 3, so no two badges share a number.
        assert_eq!(next_number(&[badge(1), badge(3)]), 4);
    }
}
