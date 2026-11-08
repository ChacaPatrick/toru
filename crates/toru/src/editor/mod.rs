//! The editor (DESIGN.md §5): a view that replaces the main window's body. The picture is a
//! `Document` that `tiny-skia` renders to one bitmap, shown on the canvas at the `Viewport`'s
//! zoom and position, under the floating tool pill, the context panel, the corners and the hint.
//! While a mark is dragged, only the region around it is rendered again, as a small overlay
//! on that bitmap, so a 4K picture isn't rebuilt and uploaded on every pointer move.
//! The drawing tools of 8.2 put `Annotation`s on the document and Select moves and resizes them;
//! 8.3 and 8.4 add the rest, each adding its `Tool`'s behaviour on the canvas and its groups to
//! `panel`.

pub mod annotation;
pub mod chrome;
pub mod document;
mod panel;
pub mod tools;
mod viewport;

use std::{sync::Arc, time::Duration};

use gpui::{
    App, BorderStyle, Bounds, Context, Corners, CursorStyle, Entity, EventEmitter, FocusHandle,
    Focusable, HitboxBehavior, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, RenderImage, ScrollDelta, ScrollEvent, ScrollHandle, SharedString,
    Size, Window, canvas, div, point, prelude::*, px, quad, size,
};
use gpui_kit::component::input::InputState;
use tiny_skia::{IntRect, Pixmap};
use toru_core::capture::Capture;

use crate::theme::{ActiveTheme, Theme};
use crate::ui::{Flash, FocusInView, fill};
use annotation::{Annotation, HandleIndex, Shape, Style, in_picture};
use document::{Document, skia_color};
pub use tools::Tool;
use tools::{Group, StrokeOptions};
use viewport::Viewport;

const COPIED_FOR: Duration = Duration::from_millis(1500);
/// A wheel reported in pixels counts this many per line.
const PIXELS_PER_LINE: f32 = 20.;
/// How far from a mark's stroke a click still selects it, in logical pixels on the canvas.
const HIT_TOLERANCE: f32 = 6.;
/// `.handle`: a selected mark's handle, its border, and how far from its centre a press takes it.
pub const HANDLE: f32 = 10.;
pub const HANDLE_BORDER: f32 = 2.;
const HANDLE_HIT: f32 = HANDLE / 2. + 4.;

pub enum EditorEvent {
    /// Back to session.
    Back,
    /// The picture was copied, as a mock until 8.5; the main window shows the toast.
    Copied,
}

/// What holds the selected mark while it is dragged.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    /// The mark itself: the pointer's start, in picture pixels.
    Body(Point<f32>),
    Handle(HandleIndex),
}

/// What a press on the canvas began, followed until the button comes up.
enum Drag {
    /// The Pan tool: the pointer's last position.
    Pan(Point<Pixels>),
    /// A drawing tool: the mark so far.
    Draw(Annotation),
    /// Select on a mark: the mark as it was, and as it is under the pointer.
    Edit {
        index: usize,
        grip: Grip,
        original: Annotation,
        current: Annotation,
    },
}

pub struct Editor {
    capture: Capture,
    /// The card's name field, shared, so a rename here is the card's too.
    name: Entity<InputState>,
    /// Why the store refused the name being typed, shown in the top bar while it stands.
    name_error: Option<SharedString>,
    document: Document,
    /// The document as last rendered, without the mark Select is dragging. `None` after an
    /// edit, until the next frame rebuilds it.
    image: Option<Arc<RenderImage>>,
    /// During a drag, the region of the picture around the mark as it is under the pointer,
    /// laid over `image`. `None` after a pointer move, until the next frame rebuilds it.
    overlay: Option<(IntRect, Arc<RenderImage>)>,
    /// Pictures the canvas no longer shows, to be taken out of the GPU atlas at the next frame,
    /// which is the first to have the window at hand.
    stale_images: Vec<Arc<RenderImage>>,
    tool: Tool,
    /// Each drawing tool's panel choices.
    options: Vec<(Tool, StrokeOptions)>,
    /// The mark Select has picked, as its index in the document.
    selected: Option<usize>,
    viewport: Viewport,
    /// The canvas's window bounds from the last frame, so zoom buttons can centre on it.
    canvas: Bounds<Pixels>,
    /// Device pixels per logical pixel at the last frame. 100% is one picture pixel per device
    /// pixel, so the picture's logical size depends on it.
    scale: f32,
    /// The picture is at Fit, so it follows the canvas as the window resizes; a zoom ends that.
    fitted: bool,
    drag: Option<Drag>,
    /// A slider drag has changed the selected mark: its later changes amend that undo step.
    live_edit: bool,
    /// The root's focus, so the shortcut letters work wherever focus is inside the editor.
    focus: FocusHandle,
    /// Back to session: focused when the editor opens.
    back: FocusHandle,
    panel_scroll: ScrollHandle,
    /// Scrolls the panel to whichever of its controls takes keyboard focus.
    panel_focus: FocusInView,
    copied: Flash<()>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.back.clone()
    }
}

impl Editor {
    /// Opens `capture`'s picture, `base`, with no annotations yet.
    pub fn new(
        capture: Capture,
        name: Entity<InputState>,
        base: Pixmap,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = Tool::all()
            .filter_map(|tool| {
                Some((
                    tool,
                    StrokeOptions::for_shape(tool.shape(tools::Head::Single)?),
                ))
            })
            .collect();
        Editor {
            capture,
            name,
            name_error: None,
            document: Document::new(base),
            image: None,
            overlay: None,
            stale_images: Vec::new(),
            tool: Tool::Select,
            options,
            selected: None,
            viewport: Viewport {
                zoom: 1.,
                origin: point(0., 0.),
            },
            canvas: Bounds::default(),
            scale: 1.,
            fitted: true,
            drag: None,
            live_edit: false,
            focus: cx.focus_handle(),
            back: cx.focus_handle().tab_stop(true),
            panel_scroll: ScrollHandle::new(),
            panel_focus: FocusInView::new(cx),
            copied: Flash::new(),
        }
    }

    /// "Toru — editing screenshot_11_07_2026_3", for the title bar.
    pub fn title(&self, cx: &App) -> SharedString {
        format!("Toru — editing {}", self.name.read(cx).value()).into()
    }

    /// Takes every picture this editor put in the GPU atlas back out. The main window calls it
    /// when the editor closes; replaced pictures go at the next frame.
    pub fn release_images(&mut self, window: &mut Window) {
        let overlay = self.overlay.take().map(|(_, image)| image);
        for image in self
            .stale_images
            .drain(..)
            .chain(self.image.take())
            .chain(overlay)
        {
            window.drop_image(image).ok();
        }
    }

    /// The card's name field.
    pub fn name_field(&self) -> Entity<InputState> {
        self.name.clone()
    }

    /// Whether the picture has annotations, which closing the editor would lose.
    pub fn is_edited(&self) -> bool {
        self.document.is_edited()
    }

    /// Why the store refused the name in the field, or `None` once it takes one. The main window
    /// keeps this in step with the card's.
    pub fn set_name_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        if self.name_error != error {
            self.name_error = error;
            cx.notify();
        }
    }

    /// "1440×900 · edited · not saved".
    fn status(&self) -> String {
        let mut parts = vec![format!(
            "{}×{}",
            self.document.width(),
            self.document.height()
        )];
        if self.document.is_edited() {
            parts.push("edited".into());
        }
        parts.push(if self.capture.is_saved() {
            "saved".into()
        } else {
            "not saved".into()
        });
        parts.join(" · ")
    }

    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        self.cancel_drag(cx);
        self.select(None, cx);
        cx.notify();
    }

    /// Drops a drag without committing it: the half-drawn mark goes, and a mark being moved
    /// returns to where it was.
    fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        match self.drag.take() {
            Some(Drag::Edit { .. }) => self.document_changed(cx),
            Some(_) => self.preview_changed(cx),
            None => {}
        }
    }

    fn select(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.live_edit = false;
        if self.selected != index {
            self.selected = index;
            cx.notify();
        }
    }

    fn options_for(&self, tool: Tool) -> StrokeOptions {
        self.options
            .iter()
            .find(|(t, _)| *t == tool)
            .map(|(_, options)| *options)
            .unwrap_or_else(|| StrokeOptions::for_shape(Shape::Line))
    }

    /// The selected mark, if Select has one.
    fn selection(&self) -> Option<(usize, &Annotation)> {
        let index = self.selected?;
        Some((index, self.document.annotations().get(index)?))
    }

    /// What the panel shows: a drawing tool's options and its shape's groups, or the selected
    /// mark's. `None` leaves the panel out.
    fn panel_options(&self, theme: &Theme) -> Option<(StrokeOptions, &'static [Group])> {
        if let Some(shape) = self.tool.shape(self.options_for(self.tool).head) {
            return Some((self.options_for(self.tool), tools::groups(shape)));
        }
        if self.tool != Tool::Select {
            return None;
        }
        let (_, annotation) = self.selection()?;
        Some((
            options_of(annotation, theme),
            tools::groups(annotation.shape),
        ))
    }

    /// A panel change: to the drawing tool's options, or to the selected mark as one undo step.
    fn set_option(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut StrokeOptions)) {
        self.live_edit = false;
        self.change_option(false, cx, change);
    }

    /// A slider's change while it is dragged: the first is an undo step, the rest amend it,
    /// until `end_live_edit`.
    fn set_option_live(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut StrokeOptions)) {
        let amend = self.live_edit;
        self.live_edit = self.change_option(amend, cx, change);
    }

    fn end_live_edit(&mut self) {
        self.live_edit = false;
    }

    /// Changes the drawing tool's options, or the selected mark: as a new undo step, or amending
    /// the last if `amend`. Returns whether the mark changed under an undo step. Nothing changes
    /// while the canvas is being dragged, so the drag's mark stays where it was found.
    fn change_option(
        &mut self,
        amend: bool,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut StrokeOptions),
    ) -> bool {
        if self.drag.is_some() {
            return false;
        }
        if let Some(slot) = self.options.iter_mut().find(|(t, _)| *t == self.tool) {
            change(&mut slot.1);
            cx.notify();
            return false;
        }
        let Some((index, annotation)) = self.selection() else {
            return false;
        };
        let theme = cx.theme();
        let mut options = options_of(annotation, theme);
        change(&mut options);
        let edited = Annotation {
            shape: match annotation.shape {
                Shape::Arrow(_) => Shape::Arrow(options.head),
                shape => shape,
            },
            style: style_of(options, theme),
            points: annotation.points.clone(),
        };
        let stepped = if amend {
            self.document.amend(index, edited);
            true
        } else {
            self.document.replace(index, edited)
        };
        self.document_changed(cx);
        stepped
    }

    /// After any change to the document: the canvas's picture is rendered again at the next
    /// frame, and so is the drag's region.
    fn document_changed(&mut self, cx: &mut Context<Self>) {
        self.stale_images.extend(self.image.take());
        self.preview_changed(cx);
    }

    /// After a pointer move during a drag: only the region around the mark is rendered again.
    fn preview_changed(&mut self, cx: &mut Context<Self>) {
        self.stale_images
            .extend(self.overlay.take().map(|(_, image)| image));
        cx.notify();
    }

    /// Undo, redo, Reset and Delete wait while the canvas is being dragged: the drag holds the
    /// index of the mark it found, which they would move from under it.
    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_none() && self.document.undo() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_none() && self.document.redo() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_none() && self.document.reset() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    /// Delete: takes the selected mark off, as one undo step.
    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        if let Some(index) = self.selected.take() {
            self.document.remove(index);
            self.document_changed(cx);
        }
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::Back);
    }

    /// Mock until 8.5: the feedback is real, the clipboard untouched.
    fn copy(&mut self, cx: &mut Context<Self>) {
        self.copied
            .show((), COPIED_FOR, cx, |this| &mut this.copied);
        cx.emit(EditorEvent::Copied);
    }

    /// The picture's logical size at 100%: one picture pixel per device pixel, so it is sharp on
    /// screen and the same size as the capture was.
    fn picture(&self) -> Size<f32> {
        self.document.size().map(|v| v / self.scale)
    }

    /// The canvas area the picture is fitted into: inside the pill, the corners and, on both
    /// sides so it stays centred, the panel.
    fn fit_area(&self) -> Bounds<f32> {
        let edge = chrome::EDGE;
        let side = panel::LEFT + panel::WIDTH + edge;
        let top = edge + chrome::PILL_HEIGHT + edge;
        let bottom = edge + chrome::CORNER_HEIGHT + edge;
        let canvas = self.canvas_size();
        Bounds::new(
            point(side, top),
            size(
                (canvas.width - side * 2.).max(1.),
                (canvas.height - top - bottom).max(1.),
            ),
        )
    }

    fn canvas_size(&self) -> Size<f32> {
        self.canvas.size.map(f32::from)
    }

    /// The viewport for the canvas as it is now: fitted again while at Fit, else kept on the
    /// canvas.
    fn laid_out(&self) -> Viewport {
        if self.fitted {
            Viewport::fit(self.picture(), self.fit_area())
        } else {
            self.viewport.clamped(self.picture(), self.canvas_size())
        }
    }

    fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.fitted = true;
        self.viewport = self.laid_out();
        cx.notify();
    }

    /// Zooms around the canvas centre, as the buttons do.
    fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let canvas = self.canvas_size();
        let centre = point(canvas.width / 2., canvas.height / 2.);
        self.zoom_at(zoom, centre, cx);
    }

    fn zoom_step(&mut self, direction: i32, cx: &mut Context<Self>) {
        self.zoom_to(self.viewport.step(direction), cx);
    }

    fn zoom_at(&mut self, zoom: f32, anchor: Point<f32>, cx: &mut Context<Self>) {
        self.fitted = false;
        self.viewport = self
            .viewport
            .zoom_to(zoom, anchor)
            .clamped(self.picture(), self.canvas_size());
        cx.notify();
    }

    /// Zooms around the pointer.
    fn wheel(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        let lines = match event.delta {
            ScrollDelta::Lines(lines) => lines.y,
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_LINE,
        };
        if lines == 0. {
            return;
        }
        let anchor = (event.position - self.canvas.origin).map(f32::from);
        let zoom = self.viewport.zoom * Viewport::wheel_factor(lines);
        self.zoom_at(zoom, anchor, cx);
    }

    /// A window position as a picture pixel.
    fn image_point(&self, position: Point<Pixels>) -> Point<f32> {
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        let v = &self.viewport;
        point(
            (on_canvas.x - v.origin.x) / v.zoom * self.scale,
            (on_canvas.y - v.origin.y) / v.zoom * self.scale,
        )
    }

    /// A picture pixel as a logical point on the canvas.
    fn canvas_point(&self, at: Point<f32>) -> Point<f32> {
        let v = &self.viewport;
        point(
            v.origin.x + at.x / self.scale * v.zoom,
            v.origin.y + at.y / self.scale * v.zoom,
        )
    }

    /// `HIT_TOLERANCE` in picture pixels at the current zoom.
    fn hit_tolerance(&self) -> f32 {
        HIT_TOLERANCE / self.viewport.zoom * self.scale
    }

    /// The selected mark's handle under `position`, if any.
    fn handle_at(&self, position: Point<Pixels>) -> Option<(usize, HandleIndex, &Annotation)> {
        let (index, annotation) = self.selection()?;
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        let handle = annotation.handles().into_iter().position(|handle| {
            let at = self.canvas_point(handle);
            (at.x - on_canvas.x).abs() <= HANDLE_HIT && (at.y - on_canvas.y).abs() <= HANDLE_HIT
        })?;
        Some((index, handle, annotation))
    }

    /// A left press on the canvas starts a drag that lasts until the button is released,
    /// wherever the pointer goes: the Pan tool moves the picture, a drawing tool begins a mark,
    /// and Select takes a handle of the selected mark, or picks the mark under the pointer and
    /// starts moving it, or clears the selection. Returns whether a drag began, so the canvas
    /// keeps the pointer for its length.
    fn mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) -> bool {
        if event.button != MouseButton::Left {
            return false;
        }
        let at = self.image_point(event.position);
        let options = self.options_for(self.tool);
        let drag = if self.tool == Tool::Pan {
            Drag::Pan(event.position)
        } else if let Some(shape) = self.tool.shape(options.head) {
            self.select(None, cx);
            let style = style_of(options, cx.theme());
            let at = in_picture(self.document.size(), at);
            Drag::Draw(Annotation::new(shape, at, style))
        } else if self.tool == Tool::Select {
            if let Some((index, handle, annotation)) = self.handle_at(event.position) {
                Drag::Edit {
                    index,
                    grip: Grip::Handle(handle),
                    original: annotation.clone(),
                    current: annotation.clone(),
                }
            } else if let Some(index) = self.document.hit_test(at, self.hit_tolerance()) {
                self.select(Some(index), cx);
                let annotation = self.document.annotations()[index].clone();
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

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            self.mouse_up(cx);
            return;
        }
        let at = self.image_point(event.position);
        let shift = event.modifiers.shift;
        let picture = self.document.size();
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
                    Grip::Body(start) => {
                        kept_in_picture(picture, original.moved(at - start, shift))
                    }
                    Grip::Handle(handle) => original.resized(handle, at, shift, picture),
                };
                self.preview_changed(cx);
            }
            None => {}
        }
    }

    /// The left button, which began the drag, is released and the drag ends: a mark drawn to any
    /// size goes on the document, and a moved or resized mark replaces its old self, each as one
    /// undo step. A mark that would show nothing is dropped. Another button's release is not the
    /// drag's end.
    fn mouse_up(&mut self, cx: &mut Context<Self>) {
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
            Some(Drag::Pan(_)) => cx.notify(),
            None => {}
        }
    }

    /// The shortcuts of DESIGN.md §5: a tool's letter, Esc back to Select, Ctrl+Z / Ctrl+Y for
    /// undo and redo, and Delete on a selected mark. Typing in the name field is left alone, but
    /// Esc still works there.
    fn key_down(&mut self, event: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if m.alt || m.platform || m.function {
            return;
        }
        if keystroke.key == "escape" && !m.control {
            self.set_tool(Tool::Select, cx);
            cx.stop_propagation();
            return;
        }
        if self.name.focus_handle(cx).is_focused(window) {
            return;
        }
        if m.control {
            match keystroke.key.as_str() {
                "z" if m.shift => self.redo(cx),
                "z" => self.undo(cx),
                "y" => self.redo(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if matches!(keystroke.key.as_str(), "delete" | "backspace") && self.selected.is_some() {
            self.delete_selected(cx);
            cx.stop_propagation();
            return;
        }
        if let Some(tool) = Tool::for_key(&keystroke.key) {
            self.set_tool(tool, cx);
            cx.stop_propagation();
        }
    }

    /// Called with the canvas's bounds each frame: the first frame, a resize and a move to a
    /// display of another scale each lay the picture out again. The chrome built for this frame
    /// read the old viewport, so the next frame is asked for; the picture itself paints at the
    /// new one.
    fn canvas_laid_out(&mut self, bounds: Bounds<Pixels>, window: &mut Window) {
        let scale = window.scale_factor();
        if bounds.size == self.canvas.size && scale == self.scale {
            self.canvas = bounds;
            return;
        }
        let first = !self.measured();
        self.canvas = bounds;
        self.scale = scale;
        let viewport = self.laid_out();
        if first || viewport != self.viewport {
            self.viewport = viewport;
            window.request_animation_frame();
        }
    }

    /// Whether a frame has measured the canvas yet. Until one has, the viewport is provisional,
    /// so the chrome that reads it waits for the next frame rather than show a zoom the picture
    /// isn't at.
    fn measured(&self) -> bool {
        self.canvas.size.width > px(0.)
    }

    /// Whether any of `mark` lies over the picture.
    fn shows(&self, mark: &Annotation) -> bool {
        let picture = IntRect::from_xywh(0, 0, self.document.width(), self.document.height());
        match (mark.paint_rect(), picture) {
            (Some(rect), Some(picture)) => rect.intersect(&picture).is_some(),
            _ => false,
        }
    }

    /// The picture under the drag's overlay: the document, without the mark Select is moving or
    /// resizing. The overlay puts that mark where the pointer has it, at its own depth.
    fn render_document(&self) -> Pixmap {
        match &self.drag {
            Some(Drag::Edit { index, .. }) => self.document.render_with(Some(*index), None),
            _ => self.document.render(),
        }
    }

    /// The drag's mark as the overlay shows it: the mark being drawn, on top, or the mark being
    /// moved or resized, at its index.
    fn preview(&self) -> Option<(Option<usize>, &Annotation)> {
        match &self.drag {
            Some(Drag::Draw(annotation)) => Some((None, annotation)),
            Some(Drag::Edit { index, current, .. }) => Some((Some(*index), current)),
            _ => None,
        }
    }

    /// The overlay for this frame: the region the drag's mark may paint, rendered as the
    /// document would render it there.
    fn render_overlay(&self) -> Option<(IntRect, Arc<RenderImage>)> {
        let (index, preview) = self.preview()?;
        let (region, pixmap) =
            self.document
                .render_region(preview.paint_rect()?, index, Some(preview))?;
        Some((region, document::render_image(&pixmap)))
    }

    /// The mark whose handles the canvas shows: the selected one, as it is under a drag.
    fn shown_selection(&self) -> Option<&Annotation> {
        match &self.drag {
            Some(Drag::Edit { current, .. }) => Some(current),
            Some(Drag::Draw(_)) => None,
            _ => self.selection().map(|(_, annotation)| annotation),
        }
    }

    /// The pointer over the canvas: a crosshair for the drawing tools, a hand for Pan.
    fn cursor(&self) -> Option<CursorStyle> {
        if self.tool == Tool::Pan {
            return Some(if matches!(self.drag, Some(Drag::Pan(_))) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            });
        }
        self.tool
            .shape(tools::Head::Single)
            .map(|_| CursorStyle::Crosshair)
    }

    /// The canvas: the look's fill, then the picture at the viewport, drawn directly so its
    /// placement is exact, then the selected mark's handles. Its hitbox takes the tools' drags
    /// and keeps the pointer while one lasts, so crossing the pill or a corner doesn't end it.
    fn canvas_layer(&self, cx: &Context<Self>) -> impl IntoElement {
        let editor = cx.entity();
        let theme = cx.theme();
        let radius = theme.radii.sm;
        let border = theme.borders.card;
        let shadows = theme.shadows.shot.clone();
        let handle_fill = theme.colors.surface;
        let handle_line = theme.colors.anno;
        let handle_radius = theme.radii.sm / 2.;
        let outline = theme.borders.hair.width;
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
                    let document = this.document.size();
                    let sx = f32::from(picture.size.width) / document.width;
                    let sy = f32::from(picture.size.height) / document.height;
                    let bounds = Bounds::new(
                        point(
                            picture.origin.x + snap(region.x() as f32 * sx),
                            picture.origin.y + snap(region.y() as f32 * sy),
                        ),
                        size(
                            snap(region.width() as f32 * sx),
                            snap(region.height() as f32 * sy),
                        ),
                    );
                    let (left, top) = (region.left() == 0, region.top() == 0);
                    let right = region.right() == this.document.width() as i32;
                    let bottom = region.bottom() == this.document.height() as i32;
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
                let Some(selected) = this.shown_selection() else {
                    return;
                };
                let on_window = |at: Point<f32>| {
                    let at = this.canvas_point(at);
                    point(bounds.origin.x + px(at.x), bounds.origin.y + px(at.y))
                };
                if selected.shape.is_trail() {
                    // A trail's handles sit on its box, so the box is shown as a hairline.
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

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.stale_images.drain(..) {
            window.drop_image(image).ok();
        }
        if self.image.is_none() {
            self.image = Some(document::render_image(&self.render_document()));
        }
        if self.overlay.is_none() {
            self.overlay = self.render_overlay();
        }
        let by = self
            .panel_focus
            .scroll_by(self.panel_scroll.bounds(), window, cx);
        if by != px(0.) {
            self.panel_scroll
                .set_offset(self.panel_scroll.offset() + point(px(0.), by));
        }
        let chrome = self.measured().then(|| {
            let pill = div()
                .absolute()
                .top(px(chrome::EDGE))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(chrome::tool_pill(self, cx));
            div()
                .absolute()
                .inset_0()
                .child(pill)
                .children(panel::panel(self, cx))
                .child(chrome::zoom_corner(self, cx))
                .child(chrome::undo_corner(self, cx))
                .child(chrome::hint_chip(self, cx))
        });
        div()
            .id("editor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, window, cx| this.key_down(event, window, cx)))
            .size_full()
            .flex()
            .flex_col()
            .child(chrome::top_bar(self, window, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.canvas_layer(cx))
                    .children(chrome),
            )
    }
}

/// The panel's choices as a mark is drawn with: the swatch resolved to its colour.
fn style_of(options: StrokeOptions, theme: &Theme) -> Style {
    let swatches = theme.colors.stroke_swatches();
    let swatch = swatches.get(options.stroke).unwrap_or(&swatches[0]);
    Style {
        color: skia_color(swatch.color),
        width: options.width,
        line: options.style,
        opacity: options.opacity,
    }
}

/// A mark's style back as the panel's choices, for showing and editing a selected mark. A
/// colour no swatch has, from another look, shows as the first.
fn options_of(annotation: &Annotation, theme: &Theme) -> StrokeOptions {
    let style = annotation.style;
    StrokeOptions {
        stroke: theme
            .colors
            .stroke_swatches()
            .iter()
            .position(|swatch| skia_color(swatch.color) == style.color)
            .unwrap_or(0),
        width: style.width,
        head: match annotation.shape {
            Shape::Arrow(head) => head,
            _ => tools::Head::Single,
        },
        style: style.line,
        opacity: style.opacity,
    }
}

/// `mark` shifted back into the picture if a move took it out.
fn kept_in_picture(picture: Size<f32>, mark: Annotation) -> Annotation {
    let b = mark.bounds();
    let back = point(
        (-b.left()).max(0.) - (b.right() - picture.width).max(0.),
        (-b.top()).max(0.) - (b.bottom() - picture.height).max(0.),
    );
    if back == point(0., 0.) {
        mark
    } else {
        mark.moved(back, false)
    }
}

/// Runs `f` on the editor from a control's handler.
fn on(
    cx: &Context<Editor>,
    f: impl Fn(&mut Editor, &mut Context<Editor>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let editor = cx.entity();
    move |_, cx| editor.update(cx, |this, cx| f(this, cx))
}

/// Runs `f` on the editor with a control's value, such as a chosen index.
fn act<A: 'static>(
    cx: &Context<Editor>,
    f: impl Fn(&mut Editor, A, &mut Context<Editor>) + 'static,
) -> impl Fn(A, &mut Window, &mut App) + 'static {
    let editor = cx.entity();
    move |value, _, cx| editor.update(cx, |this, cx| f(this, value, cx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use toru_core::settings::Look;

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
            };
            let mark = Annotation::new(
                Shape::Arrow(options.head),
                point(0., 0.),
                style_of(options, &theme),
            );
            assert_eq!(options_of(&mark, &theme), options);
        }
    }

    #[test]
    fn drags_stay_within_the_picture() {
        let picture = size(100., 50.);
        assert_eq!(in_picture(picture, point(-5., 20.)), point(0., 20.));
        assert_eq!(in_picture(picture, point(120., 70.)), point(100., 50.));
        assert_eq!(in_picture(picture, point(40., 20.)), point(40., 20.));
        // A box wherever a move left it, in the picture or not.
        let mark = |from: (f32, f32), to: (f32, f32)| Annotation {
            shape: Shape::Rectangle,
            points: vec![point(from.0, from.1), point(to.0, to.1)],
            style: Style {
                color: tiny_skia::Color::BLACK,
                width: tools::Width::Thin,
                line: tools::LineStyle::Solid,
                opacity: 1.,
            },
        };
        // Moved past the right and bottom, the box comes back to the edges.
        let out = kept_in_picture(picture, mark((90., 40.), (110., 60.)));
        assert_eq!(out.bounds(), Bounds::new(point(80., 30.), size(20., 20.)));
        let out = kept_in_picture(picture, mark((-10., -5.), (10., 15.)));
        assert_eq!(out.bounds().origin, point(0., 0.));
        let inside = mark((10., 10.), (30., 30.));
        assert_eq!(kept_in_picture(picture, inside.clone()), inside);
    }

    #[test]
    fn an_unknown_colour_shows_as_the_first_swatch() {
        let theme = Theme::new(Look::Japandi, 0);
        let mark = Annotation::new(
            Shape::Line,
            point(0., 0.),
            Style {
                color: tiny_skia::Color::from_rgba8(1, 2, 3, 255),
                width: tools::Width::Thin,
                line: tools::LineStyle::Solid,
                opacity: 1.,
            },
        );
        let options = options_of(&mark, &theme);
        assert_eq!(options.stroke, 0);
        assert_eq!(options.head, tools::Head::Single);
    }
}
