use std::sync::Arc;

use gpui::{
    BorderStyle, Bounds, Context, Corners, HitboxBehavior, Hsla, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, canvas, div, point, prelude::*, px,
    quad, size,
};
use tiny_skia::{IntRect, Pixmap};

use super::annotation::Annotation;
use super::tools::Tool;
use super::typing::TypingFrame;
use super::{Drag, Editor, document, text_input};
use crate::theme::ActiveTheme;
use crate::ui::fill;

/// `.handle`: a selected mark's handle, its border, and how far from its centre a press takes it.
pub const HANDLE: f32 = 10.;
pub const HANDLE_BORDER: f32 = 2.;

/// `.anno.text` while it is typed: the mockup's dashed frame, this far out from the text on
/// each side and this wide, with a caret this wide after the last character.
const TEXT_FRAME_PAD: (f32, f32) = (10., 6.);
const TEXT_FRAME_BORDER: f32 = 2.;
pub(super) const CARET: f32 = 2.;

impl Editor {
    /// Whether any of `mark` lies over the shown picture.
    pub(super) fn shows(&self, mark: &Annotation) -> bool {
        mark.paint_rect()
            .is_some_and(|rect| rect.intersect(&self.shown()).is_some())
    }

    /// The picture under the overlay: the shown part of the document, without the mark Select
    /// is moving or resizing or the text being typed. The overlay puts that mark where the
    /// pointer or the typing has it, at its own depth.
    pub(super) fn render_document(&self) -> Pixmap {
        let hidden = match &self.drag {
            Some(Drag::Edit { index, .. }) => Some(*index),
            _ => self.typing.as_ref().and_then(|typing| typing.index),
        };
        self.document
            .render_region(self.shown(), hidden, None)
            .expect("the shown part is within the picture")
            .1
    }

    /// The mark the overlay shows: the mark being drawn, on top, the mark being moved or resized,
    /// at its index, or the text being typed.
    fn preview(&self) -> Option<(Option<usize>, &Annotation)> {
        match (&self.drag, &self.typing) {
            (Some(Drag::Draw(annotation)), _) => Some((None, annotation)),
            (Some(Drag::Edit { index, current, .. }), _) => Some((Some(*index), current)),
            (_, Some(typing)) => Some((typing.index, &typing.text)),
            _ => None,
        }
    }

    /// The overlay for this frame: the region the drag's mark may paint, rendered as the
    /// document would render it there.
    pub(super) fn render_overlay(&self) -> Option<(IntRect, Arc<RenderImage>)> {
        let (index, preview) = self.preview()?;
        let region = preview.paint_rect()?.intersect(&self.shown())?;
        let (region, pixmap) = self.document.render_region(region, index, Some(preview))?;
        Some((region, document::render_image(&pixmap)))
    }

    /// The mark whose handles the canvas shows: the selected one, as it is under a drag, or the
    /// Crop tool's rectangle. Text being typed shows its frame instead.
    fn shown_selection(&self) -> Option<&Annotation> {
        if self.typing.is_some() {
            return None;
        }
        match &self.drag {
            Some(Drag::Edit { current, .. }) => Some(current),
            Some(Drag::Draw(_)) => None,
            _ if self.tool == Tool::Crop => self.crop_draft.as_ref(),
            _ => self.selection().map(|(_, annotation)| annotation),
        }
    }

    /// The canvas: the look's fill, then the picture at the viewport, drawn directly so its
    /// placement is exact, then, while cropping, the picture outside the rectangle dimmed as the
    /// capture overlay dims it, then the selected mark's or the rectangle's handles. Its hitbox
    /// takes the tools' drags and keeps the pointer while one lasts, so crossing the pill or a
    /// corner doesn't end it.
    pub(super) fn canvas_layer(&self, cx: &Context<Self>) -> impl IntoElement {
        let editor = cx.entity();
        let theme = cx.theme();
        let radius = theme.radii.sm;
        let border = theme.borders.card;
        let shadows = theme.shadows.shot.clone();
        let handle_fill = theme.colors.surface;
        let handle_line = theme.colors.anno;
        let handle_radius = theme.radii.sm / 2.;
        let outline = theme.borders.hair.width;
        let frame_line = theme.colors.anno;
        let dim = theme.overlay.dim;
        let cursor = self.cursor();
        let picture = canvas(
            move |bounds, window, cx| {
                editor.update(cx, |this, _| this.canvas_laid_out(bounds, window));
                (
                    editor.clone(),
                    window.insert_hitbox(bounds, HitboxBehavior::Normal),
                )
            },
            move |bounds, (editor, hitbox), window, cx| {
                if let Some(cursor) = cursor {
                    window.set_cursor_style(cursor, &hitbox);
                }
                {
                    let editor = editor.clone();
                    let hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase.bubble() && hitbox.is_hovered(window) {
                            let started = editor.update(cx, |this, cx| this.mouse_down(event, cx));
                            if started {
                                window.capture_pointer(hitbox.id);
                            }
                        }
                    });
                }
                {
                    let editor = editor.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase.bubble() {
                            editor.update(cx, |this, cx| this.mouse_move(event, cx));
                        }
                    });
                }
                {
                    let editor = editor.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() && event.button == MouseButton::Left {
                            editor.update(cx, |this, cx| this.mouse_up(cx));
                        }
                    });
                }
                text_input::handle_input(&editor, bounds, window, cx);
                let Some(image) = editor.read(cx).image.clone() else {
                    return;
                };
                let this = editor.read(cx);
                let picture = this.viewport.image_bounds(this.picture());
                // Snapped to device pixels, so the picture stays crisp at 100%.
                let scale = window.scale_factor();
                let snap = |v: f32| px((v * scale).round() / scale);
                let picture = Bounds::new(
                    point(
                        bounds.origin.x + snap(picture.origin.x),
                        bounds.origin.y + snap(picture.origin.y),
                    ),
                    size(snap(picture.size.width), snap(picture.size.height)),
                );
                let corners = Corners::all(radius);
                window.paint_drop_shadows(picture, corners, &shadows);
                let _ = window.paint_image(picture, picture, corners, image, 0, false);
                if let Some((region, overlay)) = this.overlay.clone() {
                    // The region at the picture's scale, rounded where it shares a corner with
                    // the picture.
                    let shown = this.shown();
                    let sx = f32::from(picture.size.width) / shown.width() as f32;
                    let sy = f32::from(picture.size.height) / shown.height() as f32;
                    let bounds = Bounds::new(
                        point(
                            picture.origin.x + snap((region.x() - shown.x()) as f32 * sx),
                            picture.origin.y + snap((region.y() - shown.y()) as f32 * sy),
                        ),
                        size(
                            snap(region.width() as f32 * sx),
                            snap(region.height() as f32 * sy),
                        ),
                    );
                    let (left, top) = (region.left() == shown.left(), region.top() == shown.top());
                    let right = region.right() == shown.right();
                    let bottom = region.bottom() == shown.bottom();
                    let rounded = |shared: bool| if shared { radius } else { px(0.) };
                    let corners = Corners {
                        top_left: rounded(left && top),
                        top_right: rounded(right && top),
                        bottom_left: rounded(left && bottom),
                        bottom_right: rounded(right && bottom),
                    };
                    let _ = window.paint_image(bounds, bounds, corners, overlay, 0, false);
                }
                window.paint_quad(quad(
                    picture,
                    corners,
                    gpui::transparent_black(),
                    border.width,
                    border.color,
                    BorderStyle::Solid,
                ));
                let on_window = |at: Point<f32>| {
                    let at = this.canvas_point(at);
                    point(bounds.origin.x + px(at.x), bounds.origin.y + px(at.y))
                };
                if let Some(TypingFrame {
                    bounds: b,
                    caret,
                    caret_height,
                }) = this.typing_frame()
                {
                    // `.anno.text`'s dashed frame around the text being typed, and its caret.
                    let (pad_x, pad_y) = (px(TEXT_FRAME_PAD.0), px(TEXT_FRAME_PAD.1));
                    let (tl, br) = (on_window(b.origin), on_window(b.bottom_right()));
                    window.paint_quad(quad(
                        Bounds::from_corners(
                            point(tl.x - pad_x, tl.y - pad_y),
                            point(br.x + pad_x, br.y + pad_y),
                        ),
                        Corners::default(),
                        gpui::transparent_black(),
                        px(TEXT_FRAME_BORDER),
                        frame_line,
                        BorderStyle::Dashed,
                    ));
                    let top = on_window(caret);
                    let height = px(caret_height / this.scale * this.viewport.zoom);
                    window.paint_quad(fill_quad(
                        Bounds::new(top, size(px(CARET), height)),
                        frame_line,
                    ));
                    return;
                }
                let Some(selected) = this.shown_selection() else {
                    return;
                };
                let cropping = this.tool == Tool::Crop;
                if cropping {
                    // What the crop leaves out is dimmed: the picture above, below, left and
                    // right of the rectangle.
                    let b = selected.bounds();
                    let (tl, br) = (on_window(b.origin), on_window(b.bottom_right()));
                    let p = picture;
                    for part in [
                        Bounds::from_corners(p.origin, point(p.right(), tl.y)),
                        Bounds::from_corners(point(p.left(), br.y), p.bottom_right()),
                        Bounds::from_corners(point(p.left(), tl.y), point(tl.x, br.y)),
                        Bounds::from_corners(point(br.x, tl.y), point(p.right(), br.y)),
                    ] {
                        let part = part.intersect(&p);
                        if part.size.width > px(0.) && part.size.height > px(0.) {
                            window.paint_quad(fill_quad(part, dim));
                        }
                    }
                }
                if cropping || selected.shape.is_trail() || selected.shape.is_placed() {
                    // A trail's or a placed mark's handles sit on its box, so the box is shown
                    // as a hairline; so is the crop rectangle.
                    let b = selected.bounds();
                    let (tl, br) = (on_window(b.origin), on_window(b.bottom_right()));
                    window.paint_quad(quad(
                        Bounds::from_corners(tl, br),
                        Corners::default(),
                        gpui::transparent_black(),
                        outline,
                        handle_line,
                        BorderStyle::Solid,
                    ));
                }
                let half = px(HANDLE / 2.);
                for handle in selected.handles() {
                    let centre = on_window(handle);
                    window.paint_quad(quad(
                        Bounds::new(
                            point(centre.x - half, centre.y - half),
                            size(px(HANDLE), px(HANDLE)),
                        ),
                        Corners::all(handle_radius),
                        handle_fill,
                        px(HANDLE_BORDER),
                        handle_line,
                        BorderStyle::Solid,
                    ));
                }
            },
        )
        .size_full();

        div()
            .id("canvas")
            .absolute()
            .inset_0()
            .overflow_hidden()
            .on_scroll(cx.listener(|this, event, _, cx| this.wheel(event, cx)))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(fill(&theme.colors.canvas, px(0.))),
            )
            .child(picture)
    }
}

/// A solid quad with no border.
fn fill_quad(bounds: Bounds<Pixels>, color: Hsla) -> gpui::PaintQuad {
    quad(
        bounds,
        Corners::default(),
        color,
        px(0.),
        gpui::transparent_black(),
        BorderStyle::Solid,
    )
}
