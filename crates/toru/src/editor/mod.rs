//! The editor (DESIGN.md §5): a view that replaces the main window's body. The picture is a
//! `Document` that `tiny-skia` renders to one bitmap, shown on the canvas at the `Viewport`'s
//! zoom and position, under the floating tool pill, the context panel, the corners and the hint.
//! While a mark is dragged, only the region around it is rendered again, as a small overlay
//! on that bitmap, so a 4K picture isn't rebuilt and uploaded on every pointer move.
//! The drawing tools of 8.2 put `Annotation`s on the document and Select moves and resizes them;
//! 8.3 places text, typed in place, number badges and stickers; 8.4 adds the rest, each adding
//! its `Tool`'s behaviour on the canvas and its groups to `panel`.

pub mod annotation;
pub mod chrome;
pub mod document;
mod panel;
pub mod sticker;
pub mod text;
pub mod tools;
mod viewport;

use std::{sync::Arc, time::Duration};

use gpui::{
    App, BorderStyle, Bounds, Context, Corners, CursorStyle, Entity, EventEmitter, FocusHandle,
    Focusable, HitboxBehavior, Hsla, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, ScrollDelta, ScrollEvent,
    ScrollHandle, SharedString, Size, Window, canvas, div, point, prelude::*, px, quad, size,
};
use gpui_kit::component::input::InputState;
use tiny_skia::{IntRect, Pixmap};
use toru_core::capture::Capture;

use crate::theme::{ActiveTheme, Family, Theme};
use crate::ui::{Flash, FocusInView, fill};
use annotation::{Annotation, Badge, HandleIndex, Kind, Shape, Style, Text, in_picture};
use document::{Document, skia_color};
use sticker::Sticker;
use text::Font;
pub use tools::Tool;
use tools::{Group, StrokeOptions, TextSize};
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
/// `.anno.text` while it is typed: the mockup's dashed frame, this far out from the text on
/// each side and this wide, with a caret this wide after the last character.
const TEXT_FRAME_PAD: (f32, f32) = (10., 6.);
const TEXT_FRAME_BORDER: f32 = 2.;
const CARET: f32 = 2.;
/// A badge is this many times its digits' size across, and a sticker is placed this many times
/// the look's text size wide: the mockup's 28px badge on 14px digits and 64px sticker on 16px.
const BADGE_SIZES: f32 = 2.;
const STICKER_SIZES: f32 = 4.;
/// A sticker in the panel's tray, in logical pixels.
pub const TRAY_ICON: f32 = 28.;

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

/// Text being typed in place: a new one, or the document's text at `index` with its changes
/// so far. It goes on the document as one undo step when typing ends.
struct Typing {
    index: Option<usize>,
    text: Annotation,
}

/// Where the text being typed is, in picture pixels: its box, and the top and height of its
/// caret.
struct TypingFrame {
    bounds: Bounds<f32>,
    caret: Point<f32>,
    caret_height: f32,
}

/// What the sticker tray was drawn for: the colours, since the custom palette can change them
/// without changing which palette is shown, and the scale.
#[derive(Clone, Copy, PartialEq)]
struct TrayKey {
    sticker: Hsla,
    line: Hsla,
    scale: f32,
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
    typing: Option<Typing>,
    /// The sticker tray's pictures, one per `Sticker::ALL`, drawn as `tray_key` says.
    tray: Vec<Arc<RenderImage>>,
    tray_key: Option<TrayKey>,
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
        let theme = cx.theme();
        let options = Tool::all()
            .filter_map(|tool| Some((tool, StrokeOptions::for_kind(tool.kind()?, theme))))
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
            typing: None,
            tray: Vec::new(),
            tray_key: None,
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

    /// The `demo` start-up argument: the Editor mockup's marks on the picture, placed as the
    /// tools would place them, with a second text left being typed, so the editor can be looked
    /// at without clicking. Each sits where the mockup has it, in a picture `1440 × 900` big.
    pub fn add_demo_marks(&mut self, cx: &mut Context<Self>) {
        // Large text and a hand pointing up and left, so they read at the mockup's zoom.
        for (tool, options) in &mut self.options {
            match tool {
                Tool::Text => options.size = TextSize::Large,
                Tool::Stickers => options.sticker = Sticker::Hand(7),
                _ => {}
            }
        }
        let theme = cx.theme();
        let picture = self.document.size();
        let at = |x: f32, y: f32| point(x / 1440. * picture.width, y / 900. * picture.height);
        let text = self
            .placed(Tool::Text, at(170., 290.), theme)
            .unwrap()
            .with_content("This total is wrong".into());
        let badge = self.placed(Tool::Number, at(740., 238.), theme).unwrap();
        let sticker = self.placed(Tool::Stickers, at(560., 650.), theme).unwrap();
        for mark in [text, badge, sticker] {
            self.document.add(mark);
        }
        let typing = self
            .placed(Tool::Text, at(790., 560.), theme)
            .unwrap()
            .with_content("Check this".into());
        self.tool = Tool::Text;
        self.start_typing(None, typing, cx);
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
            .chain(self.tray.drain(..))
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
        self.finish_typing(cx);
        self.tool = tool;
        self.cancel_drag(cx);
        self.select(None, cx);
        cx.notify();
    }

    /// The hint chip's text: what the tool does, or how to finish the text being typed.
    pub fn hint(&self) -> &'static str {
        if self.typing.is_some() {
            Tool::TYPING_HINT
        } else {
            self.tool.hint()
        }
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
            .unwrap_or(&self.options[0])
            .1
    }

    /// The selected mark, if Select has one.
    fn selection(&self) -> Option<(usize, &Annotation)> {
        let index = self.selected?;
        Some((index, self.document.annotations().get(index)?))
    }

    /// What the panel shows: a tool's options and its kind's groups, or the selected mark's, or
    /// those of the text being typed. `None` leaves the panel out.
    fn panel_options(&self, theme: &Theme) -> Option<(StrokeOptions, &'static [Group])> {
        if let Some(kind) = self.tool.kind() {
            return Some((self.options_for(self.tool), tools::groups(kind)));
        }
        if self.tool != Tool::Select {
            return None;
        }
        let annotation = match &self.typing {
            Some(typing) => &typing.text,
            None => self.selection()?.1,
        };
        Some((
            options_of(annotation, theme),
            tools::groups(annotation.shape.kind()),
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

    /// Changes the tool's options, the text being typed, or the selected mark: as a new undo
    /// step, or amending the last if `amend`. Returns whether the mark changed under an undo
    /// step. Nothing changes while the canvas is being dragged, so the drag's mark stays where
    /// it was found.
    fn change_option(
        &mut self,
        amend: bool,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut StrokeOptions),
    ) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let theme = cx.theme();
        if let Some(slot) = self.options.iter_mut().find(|(t, _)| *t == self.tool) {
            change(&mut slot.1);
            let options = slot.1;
            // The Text tool's options restyle the text being typed with it as well.
            if let Some(typing) = &mut self.typing {
                typing.text = restyled(&typing.text, options, theme);
                self.preview_changed(cx);
            }
            cx.notify();
            return false;
        }
        if let Some(typing) = &mut self.typing {
            let mut options = options_of(&typing.text, theme);
            change(&mut options);
            typing.text = restyled(&typing.text, options, theme);
            self.preview_changed(cx);
            return false;
        }
        let Some((index, annotation)) = self.selection() else {
            return false;
        };
        let mut options = options_of(annotation, theme);
        change(&mut options);
        let edited = restyled(annotation, options, theme);
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
    /// index of the mark it found, which they would move from under it. Text being typed goes
    /// on the document first, so the step they take is the one the user sees.
    fn undo(&mut self, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.drag.is_none() && self.document.undo() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.drag.is_none() && self.document.redo() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.drag.is_none() && self.document.reset() {
            self.select(None, cx);
            self.document_changed(cx);
        }
    }

    /// Begins typing into `text`: a new one placed by the Text tool, or the document's at
    /// `index`, which the picture is rendered without until typing ends.
    fn start_typing(&mut self, index: Option<usize>, text: Annotation, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        self.typing = Some(Typing { index, text });
        self.document_changed(cx);
    }

    /// Edits the document's text at `index` in place.
    fn edit_text(&mut self, index: usize, cx: &mut Context<Self>) {
        let text = self.document.annotations()[index].clone();
        self.start_typing(Some(index), text, cx);
    }

    /// Typing ends: the text goes on the document as one undo step, replacing the one it was
    /// if it edited one; text with nothing in it is dropped, or taken off if it was there.
    fn finish_typing(&mut self, cx: &mut Context<Self>) {
        let Some(Typing { index, text }) = self.typing.take() else {
            return;
        };
        let empty = text.text().is_none_or(|t| t.content.trim().is_empty());
        match (index, empty) {
            (Some(index), true) => {
                self.document.remove(index);
                self.select(None, cx);
            }
            (Some(index), false) => {
                self.document.replace(index, text);
            }
            (None, true) => {}
            (None, false) => {
                self.document.add(text);
            }
        }
        self.document_changed(cx);
    }

    /// A key while text is typed: characters go in, Backspace takes one out, Enter breaks the
    /// line, Ctrl+V pastes and Esc finishes. Returns whether the key was taken.
    fn type_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(typing) = &self.typing else {
            return false;
        };
        let m = keystroke.modifiers;
        if keystroke.key == "escape" {
            self.finish_typing(cx);
            return true;
        }
        let Some(mut content) = typing.text.text().map(|t| t.content.clone()) else {
            return false;
        };
        match keystroke.key.as_str() {
            "enter" if !m.control => content.push('\n'),
            "backspace" => {
                content.pop();
            }
            "v" if m.control => {
                if let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    content.push_str(&pasted);
                }
            }
            "tab" => return false,
            _ if m.control || m.alt || m.platform || m.function => return true,
            "space" => content.push(' '),
            _ => match &keystroke.key_char {
                Some(ch) if !ch.chars().any(char::is_control) => content.push_str(ch),
                _ => return true,
            },
        }
        if let Some(typing) = &mut self.typing {
            typing.text = typing.text.with_content(content);
        }
        self.preview_changed(cx);
        true
    }

    /// A new text, badge or sticker as the tool's options and the look have it, placed at `at`.
    fn placed(&self, tool: Tool, at: Point<f32>, theme: &Theme) -> Option<Annotation> {
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
                    diameter: f32::from(scale.anno_number.size) * BADGE_SIZES,
                    font: font(scale.anno_number),
                }),
                0.,
            ),
            Tool::Stickers => (
                Shape::Sticker(options.sticker),
                f32::from(scale.anno_text.size) * STICKER_SIZES,
            ),
            _ => return None,
        };
        let style = style_of(options, theme, shape.kind());
        Some(Annotation::placed(shape, at, side, style))
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
        // A press away from the text being typed finishes it, and does nothing more: the press
        // was to finish, not to place or pick. A press on the text itself keeps typing.
        if let Some(typing) = &self.typing {
            if !typing.text.hit(at, self.hit_tolerance()) {
                self.finish_typing(cx);
            }
            return false;
        }
        let options = self.options_for(self.tool);
        let tolerance = self.hit_tolerance();
        let drag = if self.tool == Tool::Pan {
            Drag::Pan(event.position)
        } else if let Some(shape) = self.tool.shape(options.head) {
            self.select(None, cx);
            let style = style_of(options, cx.theme(), shape.kind());
            let at = in_picture(self.document.size(), at);
            Drag::Draw(Annotation::new(shape, at, style))
        } else if self.tool == Tool::Text {
            // On a text already there, the click edits it; anywhere else it begins a new one.
            let on_text = self
                .document
                .hit_test(at, tolerance)
                .filter(|index| self.document.annotations()[*index].text().is_some());
            match on_text {
                Some(index) => self.edit_text(index, cx),
                None => {
                    let at = in_picture(self.document.size(), at);
                    if let Some(text) = self.placed(Tool::Text, at, cx.theme()) {
                        self.start_typing(None, text, cx);
                    }
                }
            }
            return false;
        } else if matches!(self.tool, Tool::Number | Tool::Stickers) {
            let at = in_picture(self.document.size(), at);
            if let Some(mark) = self.placed(self.tool, at, cx.theme()) {
                self.document.add(mark);
                self.document_changed(cx);
            }
            return false;
        } else if self.tool == Tool::Select {
            if let Some((index, handle, annotation)) = self.handle_at(event.position) {
                Drag::Edit {
                    index,
                    grip: Grip::Handle(handle),
                    original: annotation.clone(),
                    current: annotation.clone(),
                }
            } else if let Some(index) = self.document.hit_test(at, tolerance) {
                self.select(Some(index), cx);
                let annotation = self.document.annotations()[index].clone();
                // A double-click on text edits it in place.
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
                    Grip::Body(start) => original.moved(at - start, shift, picture),
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
    /// undo and redo, Delete on a selected mark, and Enter on selected text to edit it. Typing
    /// in the name field is left alone, but Esc still works there; text being typed on the
    /// canvas takes every key first.
    fn key_down(&mut self, event: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if self.name.focus_handle(cx).is_focused(window) && keystroke.key != "escape" {
            return;
        }
        if self.type_key(keystroke, cx) {
            cx.stop_propagation();
            return;
        }
        if m.alt || m.platform || m.function {
            return;
        }
        if keystroke.key == "escape" && !m.control {
            self.set_tool(Tool::Select, cx);
            cx.stop_propagation();
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
        if keystroke.key == "enter"
            && let Some((index, annotation)) = self.selection()
            && annotation.text().is_some()
        {
            self.edit_text(index, cx);
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

    /// The picture under the overlay: the document, without the mark Select is moving or
    /// resizing or the text being typed. The overlay puts that mark where the pointer or the
    /// typing has it, at its own depth.
    fn render_document(&self) -> Pixmap {
        let hidden = match &self.drag {
            Some(Drag::Edit { index, .. }) => Some(*index),
            _ => self.typing.as_ref().and_then(|typing| typing.index),
        };
        match hidden {
            Some(index) => self.document.render_with(Some(index), None),
            None => self.document.render(),
        }
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

    /// The frame around the text being typed, in picture pixels, with where its caret stands.
    fn typing_frame(&self) -> Option<TypingFrame> {
        let text = &self.typing.as_ref()?.text;
        let (caret, caret_height) = text.caret()?;
        Some(TypingFrame {
            bounds: text.bounds(),
            caret,
            caret_height,
        })
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

    /// The mark whose handles the canvas shows: the selected one, as it is under a drag. Text
    /// being typed shows its frame instead.
    fn shown_selection(&self) -> Option<&Annotation> {
        if self.typing.is_some() {
            return None;
        }
        match &self.drag {
            Some(Drag::Edit { current, .. }) => Some(current),
            Some(Drag::Draw(_)) => None,
            _ => self.selection().map(|(_, annotation)| annotation),
        }
    }

    /// The pointer over the canvas: a crosshair for the tools that draw or place, an I-beam for
    /// Text, a hand for Pan.
    fn cursor(&self) -> Option<CursorStyle> {
        match self.tool {
            Tool::Pan => Some(if matches!(self.drag, Some(Drag::Pan(_))) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            }),
            Tool::Text => Some(CursorStyle::IBeam),
            _ => self.tool.kind().map(|_| CursorStyle::Crosshair),
        }
    }

    /// The sticker tray's pictures, drawn again when the sticker colours or the scale have
    /// changed since. The old ones leave the GPU at the next frame.
    fn refresh_tray(&mut self, theme: &Theme, scale: f32) {
        let key = TrayKey {
            sticker: theme.colors.sticker,
            line: theme.colors.sticker_line,
            scale,
        };
        if self.tray_key == Some(key) {
            return;
        }
        self.tray_key = Some(key);
        self.stale_images.append(&mut self.tray);
        self.tray = Sticker::ALL
            .iter()
            .filter_map(|sticker| tray_image(*sticker, theme, scale))
            .collect();
    }

    /// The tray's picture of sticker `index`, if drawn.
    pub fn tray_image(&self, index: usize) -> Option<Arc<RenderImage>> {
        self.tray.get(index).cloned()
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
        let frame_line = theme.colors.anno;
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
                if selected.shape.is_trail() || selected.shape.is_placed() {
                    // A trail's or a placed mark's handles sit on its box, so the box is shown
                    // as a hairline.
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
        self.refresh_tray(cx.theme(), window.scale_factor());
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

/// The panel's choices as a mark of `kind` is drawn with: the swatch resolved to its colour. A
/// sticker takes the look's sticker colours instead, and a badge's digits the look's.
fn style_of(options: StrokeOptions, theme: &Theme, kind: Kind) -> Style {
    let swatches = theme.colors.stroke_swatches();
    let swatch = swatches.get(options.stroke).unwrap_or(&swatches[0]);
    let c = &theme.colors;
    let (color, second) = match kind {
        Kind::Sticker => (c.sticker, c.sticker_line),
        _ => (swatch.color, c.badge_text()),
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

/// `annotation` as `options` have it: its style, an arrow's head, text's face and size, a
/// sticker's kind. Text keeps a size set by hand unless its preset was changed.
fn restyled(annotation: &Annotation, options: StrokeOptions, theme: &Theme) -> Annotation {
    let shape = match &annotation.shape {
        Shape::Arrow(_) => Shape::Arrow(options.head),
        Shape::Text(text) => {
            let mut text = text.clone();
            text.font.family = theme.fonts.family(options.font);
            if options.size != TextSize::nearest(text.size, theme) {
                text.size = options.size.pixels(theme);
            }
            Shape::Text(text)
        }
        Shape::Sticker(_) => Shape::Sticker(options.sticker),
        shape => shape.clone(),
    };
    Annotation {
        style: style_of(options, theme, shape.kind()),
        shape,
        points: annotation.points.clone(),
    }
}

/// The number the next badge gets: one past the highest on the picture, so numbers count up
/// and a badge deleted or undone frees nothing below the rest.
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

/// `sticker` drawn for the panel's tray at `TRAY_ICON` logical pixels, `scale` device pixels
/// each, in the look's colours, with a little room for its outline.
fn tray_image(sticker: Sticker, theme: &Theme, scale: f32) -> Option<Arc<RenderImage>> {
    let side = (TRAY_ICON * scale).round();
    let mut pixmap = Pixmap::new(side as u32, side as u32)?;
    let margin = sticker::OUTLINE / sticker::UNITS * side;
    let options = StrokeOptions::for_kind(Kind::Sticker, theme);
    let mark = Annotation::placed(
        Shape::Sticker(sticker),
        point(side / 2., side / 2.),
        side - margin * 2.,
        style_of(options, theme, Kind::Sticker),
    );
    mark.draw_at(&mut pixmap, point(0, 0));
    Some(document::render_image(&pixmap))
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
    fn drags_stay_within_the_picture() {
        let picture = size(100., 50.);
        assert_eq!(in_picture(picture, point(-5., 20.)), point(0., 20.));
        assert_eq!(in_picture(picture, point(120., 70.)), point(100., 50.));
        assert_eq!(in_picture(picture, point(40., 20.)), point(40., 20.));
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

    fn text_mark(theme: &Theme, size: f32) -> Annotation {
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
        let recoloured = restyled(
            &text,
            StrokeOptions {
                stroke: 0,
                ..options
            },
            &theme,
        );
        assert_eq!(recoloured.text().unwrap().size, 26.);
        assert_eq!(
            recoloured.style.color,
            skia_color(theme.colors.stroke_swatches()[0].color)
        );
        let larger = restyled(
            &text,
            StrokeOptions {
                size: TextSize::Huge,
                font: Family::Display,
                ..options
            },
            &theme,
        );
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
        let turned = restyled(
            &sticker,
            StrokeOptions {
                sticker: Sticker::Hand(3),
                ..options
            },
            &theme,
        );
        assert_eq!(turned.shape, Shape::Sticker(Sticker::Hand(3)));
    }

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

    #[test]
    fn the_tray_draws_every_sticker_at_the_scale() {
        let theme = Theme::new(Look::PixelArt, 1);
        for sticker in Sticker::ALL {
            let image = tray_image(sticker, &theme, 1.5).unwrap();
            assert_eq!(image.size(0), size(42.into(), 42.into()));
        }
    }
}
