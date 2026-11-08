//! The editor (DESIGN.md §5): a view that replaces the main window's body. The picture is a
//! `Document` that `tiny-skia` renders to one bitmap, shown on the canvas at the `Viewport`'s
//! zoom and position, under the floating tool pill, the context panel, the corners and the hint.
//! The drawing tools arrive in 8.2 to 8.4: each adds a `Tool`'s behaviour on the canvas, its
//! `Annotation` to the document and its groups to `panel`.

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
use tiny_skia::Pixmap;
use toru_core::capture::Capture;

use crate::theme::ActiveTheme;
use crate::ui::{Flash, FocusInView, fill};
use document::Document;
use tools::ArrowOptions;
pub use tools::Tool;
use viewport::Viewport;

const COPIED_FOR: Duration = Duration::from_millis(1500);
/// A wheel reported in pixels counts this many per line.
const PIXELS_PER_LINE: f32 = 20.;

pub enum EditorEvent {
    /// Back to session.
    Back,
    /// The picture was copied, as a mock until 8.5; the main window shows the toast.
    Copied,
}

pub struct Editor {
    capture: Capture,
    /// The card's name field, shared, so a rename here is the card's too.
    name: Entity<InputState>,
    document: Document,
    /// The document as last rendered. `None` after an edit, until the next frame rebuilds it.
    image: Option<Arc<RenderImage>>,
    /// Pictures the canvas no longer shows, to be taken out of the GPU atlas at the next frame,
    /// which is the first to have the window at hand.
    stale_images: Vec<Arc<RenderImage>>,
    tool: Tool,
    arrow: ArrowOptions,
    viewport: Viewport,
    /// The canvas's window bounds from the last frame, so zoom buttons can centre on it.
    canvas: Bounds<Pixels>,
    /// Device pixels per logical pixel at the last frame. 100% is one picture pixel per device
    /// pixel, so the picture's logical size depends on it.
    scale: f32,
    /// The picture is at Fit, so it follows the canvas as the window resizes; a zoom ends that.
    fitted: bool,
    /// The pointer's last position while the Pan tool drags the picture.
    drag: Option<Point<Pixels>>,
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
        Editor {
            capture,
            name,
            document: Document::new(base),
            image: None,
            stale_images: Vec::new(),
            tool: Tool::Select,
            arrow: ArrowOptions::default(),
            viewport: Viewport {
                zoom: 1.,
                origin: point(0., 0.),
            },
            canvas: Bounds::default(),
            scale: 1.,
            fitted: true,
            drag: None,
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
        for image in self.stale_images.drain(..).chain(self.image.take()) {
            window.drop_image(image).ok();
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
        self.drag = None;
        cx.notify();
    }

    fn set_arrow(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut ArrowOptions)) {
        change(&mut self.arrow);
        cx.notify();
    }

    /// After any change to the document: the canvas redraws it at the next frame.
    fn document_changed(&mut self, cx: &mut Context<Self>) {
        self.stale_images.extend(self.image.take());
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.document.undo() {
            self.document_changed(cx);
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if self.document.redo() {
            self.document_changed(cx);
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        if self.document.reset() {
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

    /// A press on the canvas with the Pan tool starts a drag that lasts until the button is
    /// released, wherever the pointer goes: the canvas keeps the pointer for the drag's length.
    fn mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) -> bool {
        if self.tool != Tool::Pan || event.button != MouseButton::Left {
            return false;
        }
        self.drag = Some(event.position);
        cx.notify();
        true
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(last) = self.drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            cx.notify();
            return;
        }
        let delta = (event.position - last).map(f32::from);
        self.viewport = self
            .viewport
            .panned(delta)
            .clamped(self.picture(), self.canvas_size());
        self.drag = Some(event.position);
        cx.notify();
    }

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    /// The shortcuts of DESIGN.md §5: a tool's letter, Esc back to Select, and Ctrl+Z / Ctrl+Y
    /// for undo and redo. Typing in the name field is left alone, but Esc still works there.
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

    /// The canvas: the look's fill, then the picture at the viewport, drawn directly so its
    /// placement is exact. Its hitbox takes the Pan tool's drags and keeps the pointer while one
    /// lasts, so crossing the pill or a corner doesn't end it.
    fn canvas_layer(&self, cx: &Context<Self>) -> impl IntoElement {
        let editor = cx.entity();
        let theme = cx.theme();
        let radius = theme.radii.sm;
        let border = theme.borders.card;
        let shadows = theme.shadows.shot.clone();
        let panning = self.tool == Tool::Pan;
        let dragging = self.drag.is_some();
        let picture = canvas(
            move |bounds, window, cx| {
                editor.update(cx, |this, _| this.canvas_laid_out(bounds, window));
                (
                    editor.clone(),
                    window.insert_hitbox(bounds, HitboxBehavior::Normal),
                )
            },
            move |bounds, (editor, hitbox), window, cx| {
                if panning {
                    let cursor = if dragging {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::OpenHand
                    };
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
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() {
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
                window.paint_quad(quad(
                    picture,
                    corners,
                    gpui::transparent_black(),
                    border.width,
                    border.color,
                    BorderStyle::Solid,
                ));
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
            self.image = Some(document::render_image(&self.document.render()));
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
