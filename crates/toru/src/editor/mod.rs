//! The editor (DESIGN.md §5): a view that replaces the main window's body. The picture is a
//! `Document` that `tiny-skia` renders to one bitmap, shown on the canvas at the `Viewport`'s
//! zoom and position, under the floating tool pill, the context panel, the corners and the hint.
//! While a mark is dragged, only the region around it is rendered again, as a small overlay
//! on that bitmap, so a 4K picture isn't rebuilt and uploaded on every pointer move.
//! The drawing tools of 8.2 put `Annotation`s on the document and Select moves and resizes them;
//! 8.3 places text, typed in place, number badges and stickers. 8.4's Block is drawn the same
//! way, its pixels computed by `block`; its Crop is a rectangle of the document that the canvas
//! shows the whole picture around while it is adjusted; and its Eraser takes marks off. Each
//! tool adds its behaviour on the canvas here and its groups to `panel`.

pub mod annotation;
pub mod block;
pub mod chrome;
pub mod document;
mod panel;
pub mod sticker;
pub mod text;
mod text_input;
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
use toru_core::{settings::SaveFormat, store::CaptureId};

use crate::theme::{ActiveTheme, Family, Theme};
use crate::ui::{Flash, FocusInView, fill};
use annotation::{Annotation, Badge, HandleIndex, Kind, Shape, Style, Text, in_picture};
use document::{Document, Edits, Flattened, skia_color};
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
    /// Back to session, with the picture if the card doesn't show it yet.
    Back(Option<Flattened>),
    Copy(Flattened),
    Save(Flattened),
    /// Save pressed with the file already showing this.
    NothingToSave,
    SaveAs(Flattened, SaveFormat),
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
    /// The Crop tool on its rectangle, as it was; `Editor::crop_draft` has it under the pointer.
    Crop { grip: Grip, original: Annotation },
    /// The Eraser: whether a mark has gone yet, so the next folds into the same undo step, and
    /// the pointer's last position in picture pixels, so a fast sweep misses nothing between.
    Erase { removed: bool, last: Point<f32> },
}

/// Text being typed in place: a new one, or the document's text at `index` with its changes
/// so far. It goes on the document as one undo step when typing ends. `marked` is the end of
/// the content an input method is still composing, in bytes.
struct Typing {
    index: Option<usize>,
    text: Annotation,
    marked: Option<std::ops::Range<usize>>,
}

/// What `Editor::type_key` did with a key while text is typed.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Typed {
    /// Not a typing key, so the editor's shortcuts may have it.
    Not,
    /// Taken, and nothing else acts on it.
    Taken,
    /// A character, which reaches the text through `text_input`. The shortcuts don't have it,
    /// but it propagates: Windows makes the character only from a key-down nothing handled.
    Character,
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
    stored: CaptureId,
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
    /// The Crop tool's rectangle, in whole-picture pixels, while there is one: held as a
    /// rectangle mark so the handle, move and resize arithmetic is the marks'.
    crop_draft: Option<Annotation>,
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
    /// The panel's Crop button, so Enter crops from it and the canvas but not from Cancel.
    crop_button: FocusHandle,
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
    /// Opens the picture of the capture `stored` holds, with no annotations yet. `document` says
    /// what the card and the saved file show.
    pub fn new(
        stored: CaptureId,
        name: Entity<InputState>,
        document: Document,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = cx.theme();
        let options = Tool::all()
            .filter_map(|tool| Some((tool, StrokeOptions::for_kind(tool.kind()?, theme))))
            .collect();
        Editor {
            stored,
            name,
            name_error: None,
            document,
            image: None,
            overlay: None,
            stale_images: Vec::new(),
            tool: Tool::Select,
            options,
            selected: None,
            typing: None,
            tray: Vec::new(),
            tray_key: None,
            crop_draft: None,
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
            crop_button: cx.focus_handle().tab_stop(true),
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
        let picture = self.shown_bounds().size;
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

    /// Which capture is being edited.
    pub fn stored(&self) -> CaptureId {
        self.stored
    }

    /// The card's name field.
    pub fn name_field(&self) -> Entity<InputState> {
        self.name.clone()
    }

    /// Whether the picture has annotations, or text is being typed, which closing the editor
    /// would lose.
    pub fn is_edited(&self) -> bool {
        self.document.is_edited()
            || self.typing.as_ref().is_some_and(|typing| {
                typing
                    .text
                    .text()
                    .is_some_and(|t| !t.content.trim().is_empty())
            })
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
        parts.push(if self.document.is_saved() {
            "saved".into()
        } else {
            "not saved".into()
        });
        parts.join(" · ")
    }

    /// Switches tools. Into or out of Crop the canvas changes what it shows, the whole picture
    /// or the crop, and is laid out again; leaving Crop drops the rectangle unless `apply_crop`
    /// took it first.
    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        let was_cropping = self.tool == Tool::Crop;
        self.tool = tool;
        self.cancel_drag(cx);
        self.select(None, cx);
        if (tool == Tool::Crop) != was_cropping {
            self.crop_draft = (tool == Tool::Crop)
                .then(|| self.document_crop_draft())
                .flatten();
            self.shown_changed(cx);
        }
        cx.notify();
    }

    /// The document's crop as the Crop tool's rectangle, or `None` without one.
    fn document_crop_draft(&self) -> Option<Annotation> {
        self.document
            .is_cropped()
            .then(|| rectangle(self.document.crop()))
    }

    /// After the canvas changes what it shows, the whole picture or the crop: it is laid out
    /// again at Fit, so the new picture sits centred at a zoom that fits, and rendered again.
    fn shown_changed(&mut self, cx: &mut Context<Self>) {
        self.fitted = true;
        if self.measured() {
            self.viewport = self.laid_out();
        }
        self.document_changed(cx);
    }

    /// The hint chip's text: what the tool does, or how to finish the text being typed.
    pub fn hint(&self) -> &'static str {
        if self.typing.is_some() {
            Tool::TYPING_HINT
        } else {
            self.tool.hint()
        }
    }

    /// The Crop tool's rectangle as whole pixels, if there is one.
    pub(super) fn crop_rect(&self) -> Option<IntRect> {
        whole_pixels(self.crop_draft.as_ref()?.bounds())
    }

    /// Crops to the rectangle, as one undo step, and returns to Select. With no rectangle it
    /// just returns to Select.
    fn apply_crop(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        if let Some(rect) = self.crop_rect() {
            self.document.set_crop(Some(rect));
        }
        self.set_tool(Tool::Select, cx);
    }

    /// Drops a drag without committing it: the half-drawn mark goes, a mark being moved returns
    /// to where it was, and the crop rectangle too. What the eraser took stays taken.
    fn cancel_drag(&mut self, cx: &mut Context<Self>) {
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
    fn set_option(&mut self, cx: &mut Context<Self>, change: impl Fn(&mut StrokeOptions)) {
        self.live_edit = false;
        self.change_option(false, cx, change);
    }

    /// A slider's change while it is dragged: the first is an undo step, the rest amend it,
    /// until `end_live_edit`.
    fn set_option_live(&mut self, cx: &mut Context<Self>, change: impl Fn(&mut StrokeOptions)) {
        let amend = self.live_edit;
        self.live_edit = self.change_option(amend, cx, change);
    }

    fn end_live_edit(&mut self) {
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
        self.step_history(Document::undo, cx);
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        self.step_history(Document::redo, cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.step_history(Document::reset, cx);
    }

    /// Undo, redo or Reset: typing ends, then `step` the document and, if it did anything,
    /// drop the selection. A step that changes the crop changes what the canvas shows, so it is
    /// laid out again, and while the Crop tool is out its rectangle follows, so Enter can't
    /// bring back a crop that was just undone or reset.
    fn step_history(&mut self, step: fn(&mut Document) -> bool, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.drag.is_some() {
            return;
        }
        let (crop, shown) = (self.document.crop(), self.shown());
        if !step(&mut self.document) {
            return;
        }
        self.select(None, cx);
        if self.tool == Tool::Crop && self.document.crop() != crop {
            self.crop_draft = self.document_crop_draft();
        }
        if self.shown() == shown {
            self.document_changed(cx);
        } else {
            self.shown_changed(cx);
        }
    }

    /// Begins typing into `text`: a new one placed by the Text tool, or the document's at
    /// `index`, which the picture is rendered without until typing ends.
    fn start_typing(&mut self, index: Option<usize>, text: Annotation, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        self.typing = Some(Typing {
            index,
            text,
            marked: None,
        });
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
        let Some(Typing { index, text, .. }) = self.typing.take() else {
            return;
        };
        let empty = text.text().is_none_or(|t| t.content.trim().is_empty());
        match (index, empty) {
            (Some(index), true) => {
                self.document.remove(index, false);
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

    /// A key while text is typed: Backspace takes a character out, Enter breaks the line,
    /// Ctrl+V pastes and Esc finishes. Characters come through `text_input`, composed by the
    /// keyboard layout or an input method, so AltGr and accent keys type what they show; their
    /// keys are kept from the shortcuts here. Ctrl+Z and Ctrl+Y are left to `key_down`, whose
    /// undo and redo put the text on the document first.
    fn type_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> Typed {
        let Some(typing) = &self.typing else {
            return Typed::Not;
        };
        let m = keystroke.modifiers;
        // AltGr is Ctrl+Alt on Windows; Ctrl alone is a shortcut.
        let ctrl = m.control && !m.alt;
        if keystroke.key == "escape" {
            self.finish_typing(cx);
            return Typed::Taken;
        }
        let Some(mut content) = typing.text.text().map(|t| t.content.clone()) else {
            return Typed::Not;
        };
        match keystroke.key.as_str() {
            "enter" if !m.control => content.push('\n'),
            "backspace" => {
                content.pop();
            }
            "v" if ctrl => {
                if let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    content.push_str(&text_input::line_breaks(&pasted));
                }
            }
            "z" | "y" if ctrl => return Typed::Not,
            "tab" => return Typed::Not,
            _ if types_character(keystroke) => return Typed::Character,
            _ => return Typed::Taken,
        }
        if let Some(typing) = &mut self.typing {
            typing.text = typing.text.with_content(content);
            typing.marked = None;
        }
        self.preview_changed(cx);
        Typed::Taken
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
            self.document.remove(index, false);
            self.document_changed(cx);
        }
    }

    /// The topmost mark under `at`, a pixel of the whole picture, if `at` is on the shown
    /// picture: a mark hidden by the crop, or the part of one past it, can't be picked from
    /// the margin.
    fn mark_at(&self, at: Point<f32>) -> Option<usize> {
        self.shown_bounds()
            .contains(&at)
            .then(|| self.document.hit_test(at, self.hit_tolerance()))
            .flatten()
    }

    /// Takes off every mark the eraser passes between `from` and `to`, in picture pixels: the
    /// first as an undo step and the rest of the drag within it. The way is sampled a
    /// tolerance apart, so a quick sweep across a thin line still takes it, and only where it
    /// crosses the shown picture, so marks hidden by the crop stay.
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

    /// Text being typed, and a crop rectangle drawn but not yet applied, go on the document, so
    /// what leaves the editor is what the user sees. The crop is applied as Apply would, as one
    /// undo step, and the editor returns to Select.
    fn commit_pending(&mut self, cx: &mut Context<Self>) {
        self.finish_typing(cx);
        if self.tool == Tool::Crop {
            self.apply_crop(cx);
        }
    }

    /// What Copy, Save and Save as take, once `commit_pending` has run; a drag waits, as the
    /// mark under the pointer isn't on the document yet.
    fn flattened(&mut self, cx: &mut Context<Self>) -> Option<Flattened> {
        self.commit_pending(cx);
        self.drag.is_none().then(|| self.document.flatten())
    }

    /// The edited picture for the card, if it shows something else: what Back gives it, and
    /// what quitting with the editor open gives it so the edits aren't lost.
    pub fn take_unwritten(&mut self, cx: &mut Context<Self>) -> Option<Flattened> {
        self.commit_pending(cx);
        self.document.take_unwritten()
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        let changed = self.take_unwritten(cx);
        cx.emit(EditorEvent::Back(changed));
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(flattened) = self.flattened(cx) {
            cx.emit(EditorEvent::Copy(flattened));
        }
    }

    /// The Copy button's check mark, once the clipboard has taken the picture.
    pub fn show_copied(&mut self, cx: &mut Context<Self>) {
        self.copied
            .show((), COPIED_FOR, cx, |this| &mut this.copied);
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(flattened) = self.flattened(cx) else {
            return;
        };
        cx.emit(if self.document.is_saved() {
            EditorEvent::NothingToSave
        } else {
            EditorEvent::Save(flattened)
        });
    }

    /// The file now shows `edits`, which the user may have changed since.
    pub fn saved(&mut self, edits: Edits, cx: &mut Context<Self>) {
        self.document.mark_saved(edits);
        cx.notify();
    }

    /// The card shows `edits`, which no file does: a redacted Save stored them on the card
    /// first, then couldn't write the file.
    pub fn written(&mut self, edits: Edits) {
        self.document.mark_written(edits);
    }

    /// Writing `edits` to the card failed, so Back tries again.
    pub fn write_failed(&mut self, edits: &Edits) {
        self.document.write_failed(edits);
    }

    /// A save of `picture`, flattened from `saved`, deleted the capture's original, as the
    /// picture hid some of it. The saved picture becomes the one edited, with no undo back past
    /// it, so neither Reset edits nor Undo can bring back what was hidden. `to_card` says the
    /// card shows it too (Save, not Save as). Typing under way is finished first, and a drag
    /// under way is dropped, as the marks it holds by index change.
    pub fn original_removed(
        &mut self,
        picture: Arc<Pixmap>,
        saved: &Edits,
        to_card: bool,
        cx: &mut Context<Self>,
    ) {
        self.finish_typing(cx);
        self.cancel_drag(cx);
        let shown = self.shown();
        self.document.forget_original(picture, saved, to_card);
        self.select(None, cx);
        if self.tool == Tool::Crop {
            self.crop_draft = self.document_crop_draft();
        }
        if self.shown() == shown {
            self.document_changed(cx);
        } else {
            self.shown_changed(cx);
        }
    }

    fn save_as(&mut self, format: SaveFormat, cx: &mut Context<Self>) {
        if let Some(flattened) = self.flattened(cx) {
            cx.emit(EditorEvent::SaveAs(flattened, format));
        }
    }

    /// The part of the whole picture the canvas shows: the crop, or all of it while the Crop
    /// tool is out so the rectangle can be drawn anywhere on it.
    fn shown(&self) -> IntRect {
        if self.tool == Tool::Crop {
            self.document.whole()
        } else {
            self.document.crop()
        }
    }

    /// `shown` in picture pixels, as the marks are held within it.
    fn shown_bounds(&self) -> Bounds<f32> {
        bounds_of(self.shown())
    }

    /// The shown picture's logical size at 100%: one picture pixel per device pixel, so it is
    /// sharp on screen and the same size as the capture was.
    fn picture(&self) -> Size<f32> {
        self.shown_bounds().size.map(|v| v / self.scale)
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

    /// A window position as a pixel of the whole picture.
    fn image_point(&self, position: Point<Pixels>) -> Point<f32> {
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        let v = &self.viewport;
        let shown = self.shown_bounds().origin;
        point(
            shown.x + (on_canvas.x - v.origin.x) / v.zoom * self.scale,
            shown.y + (on_canvas.y - v.origin.y) / v.zoom * self.scale,
        )
    }

    /// A pixel of the whole picture as a logical point on the canvas.
    fn canvas_point(&self, at: Point<f32>) -> Point<f32> {
        let v = &self.viewport;
        let shown = self.shown_bounds().origin;
        point(
            v.origin.x + (at.x - shown.x) / self.scale * v.zoom,
            v.origin.y + (at.y - shown.y) / self.scale * v.zoom,
        )
    }

    /// `HIT_TOLERANCE` in picture pixels at the current zoom.
    fn hit_tolerance(&self) -> f32 {
        HIT_TOLERANCE / self.viewport.zoom * self.scale
    }

    /// The selected mark's handle under `position`, if any.
    fn handle_at(&self, position: Point<Pixels>) -> Option<(usize, HandleIndex, &Annotation)> {
        let (index, annotation) = self.selection()?;
        Some((index, self.handle_on(annotation, position)?, annotation))
    }

    /// `mark`'s handle under `position`, if any.
    fn handle_on(&self, mark: &Annotation, position: Point<Pixels>) -> Option<HandleIndex> {
        let on_canvas = (position - self.canvas.origin).map(f32::from);
        mark.handles().into_iter().position(|handle| {
            let at = self.canvas_point(handle);
            (at.x - on_canvas.x).abs() <= HANDLE_HIT && (at.y - on_canvas.y).abs() <= HANDLE_HIT
        })
    }

    /// A left press on the canvas starts a drag that lasts until the button is released,
    /// wherever the pointer goes: the Pan tool moves the picture, a drawing tool begins a mark,
    /// Select takes a handle of the selected mark, or picks the mark under the pointer and
    /// starts moving it, or clears the selection, Crop takes a handle of its rectangle, or moves
    /// it, or begins a new one, and the Eraser begins taking marks off. Returns whether a drag
    /// began, so the canvas keeps the pointer for its length.
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
        let drag = if self.tool == Tool::Pan {
            Drag::Pan(event.position)
        } else if let Some(shape) = self.tool.shape(&options) {
            self.select(None, cx);
            let style = style_of(options, cx.theme(), shape.kind());
            let at = in_picture(self.shown_bounds(), at);
            Drag::Draw(Annotation::new(shape, at, style))
        } else if self.tool == Tool::Text {
            // On a text already there, the click edits it; anywhere else it begins a new one.
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
                    // A fresh rectangle: its far corner follows the pointer from the press.
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

    /// The left button, which began the drag, is released and the drag ends: a mark drawn to any
    /// size goes on the document, and a moved or resized mark replaces its old self, each as one
    /// undo step. A mark that would show nothing is dropped, and so is a crop rectangle of no
    /// size. Another button's release is not the drag's end.
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

    /// The shortcuts of DESIGN.md §5: a tool's letter, Esc back to Select, Ctrl+Z / Ctrl+Y for
    /// undo and redo, Delete on a selected mark, Enter to crop, and Enter on selected text to
    /// edit it. Typing in the name field is left alone, but Esc still works there; text being
    /// typed on the canvas has every key first.
    fn key_down(&mut self, event: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if self.name.focus_handle(cx).is_focused(window) && keystroke.key != "escape" {
            return;
        }
        match self.type_key(keystroke, cx) {
            Typed::Taken => {
                cx.stop_propagation();
                return;
            }
            Typed::Character => return,
            Typed::Not => {}
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
                "c" if !m.shift => self.copy(cx),
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
        // Enter crops from the canvas or the panel's Crop button only: on any other control
        // it is that control's, so Cancel cancels and Back goes back.
        if keystroke.key == "enter"
            && self.tool == Tool::Crop
            && (self.focus.is_focused(window) || self.crop_button.is_focused(window))
        {
            self.apply_crop(cx);
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

    /// Whether any of `mark` lies over the shown picture.
    fn shows(&self, mark: &Annotation) -> bool {
        mark.paint_rect()
            .is_some_and(|rect| rect.intersect(&self.shown()).is_some())
    }

    /// The picture under the overlay: the shown part of the document, without the mark Select
    /// is moving or resizing or the text being typed. The overlay puts that mark where the
    /// pointer or the typing has it, at its own depth.
    fn render_document(&self) -> Pixmap {
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

    /// The pointer over the canvas: a crosshair for the tools that draw or place, drag something
    /// out or sweep over it, an I-beam for Text, a hand for Pan.
    fn cursor(&self) -> Option<CursorStyle> {
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
    /// placement is exact, then, while cropping, the picture outside the rectangle dimmed as the
    /// capture overlay dims it, then the selected mark's or the rectangle's handles. Its hitbox
    /// takes the tools' drags and keeps the pointer while one lasts, so crossing the pill or a
    /// corner doesn't end it.
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

/// Whether a key types a character: the keyboard layout gives it one, with no modifier but
/// Shift, or with AltGr, which Windows reports as Ctrl+Alt. Ctrl or Alt alone makes it a
/// shortcut, and a dead key has its accent as its character.
fn types_character(keystroke: &Keystroke) -> bool {
    let m = keystroke.modifiers;
    keystroke.key_char.is_some() && m.control == m.alt && !m.platform && !m.function
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

/// `rect` as a rectangle mark, for the Crop tool's draft; its style is never drawn.
fn rectangle(rect: IntRect) -> Annotation {
    Annotation {
        shape: Shape::Rectangle,
        points: vec![
            point(rect.x() as f32, rect.y() as f32),
            point(rect.right() as f32, rect.bottom() as f32),
        ],
        style: Style {
            color: tiny_skia::Color::BLACK,
            second: tiny_skia::Color::BLACK,
            width: tools::Width::Thin,
            line: tools::LineStyle::Solid,
            opacity: 1.,
        },
    }
}

/// `rect` in picture pixels, as the marks are held within it.
fn bounds_of(rect: IntRect) -> Bounds<f32> {
    Bounds::new(
        point(rect.x() as f32, rect.y() as f32),
        size(rect.width() as f32, rect.height() as f32),
    )
}

/// `b` rounded to whole pixels, at least one each way, for the crop.
fn whole_pixels(b: Bounds<f32>) -> Option<IntRect> {
    let (left, top) = (b.left().round() as i32, b.top().round() as i32);
    IntRect::from_ltrb(
        left,
        top,
        (b.right().round() as i32).max(left + 1),
        (b.bottom().round() as i32).max(top + 1),
    )
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
        let picture = Bounds::new(point(0., 0.), size(100., 50.));
        assert_eq!(in_picture(picture, point(-5., 20.)), point(0., 20.));
        assert_eq!(in_picture(picture, point(120., 70.)), point(100., 50.));
        assert_eq!(in_picture(picture, point(40., 20.)), point(40., 20.));
        // A cropped picture starts where the crop does.
        let cropped = Bounds::new(point(20., 10.), size(50., 30.));
        assert_eq!(in_picture(cropped, point(5., 5.)), point(20., 10.));
        assert_eq!(
            bounds_of(IntRect::from_xywh(20, 10, 50, 30).unwrap()),
            cropped
        );
    }

    #[test]
    fn a_crop_rectangle_rounds_to_whole_pixels_and_never_vanishes() {
        let whole = IntRect::from_xywh(0, 0, 100, 50).unwrap();
        assert_eq!(
            rectangle(whole).bounds(),
            Bounds::new(point(0., 0.), size(100., 50.))
        );
        let tiny = whole_pixels(Bounds::new(point(10.4, 5.6), size(0.2, 0.1))).unwrap();
        assert_eq!(tiny, IntRect::from_xywh(10, 6, 1, 1).unwrap());
        let rounded = whole_pixels(Bounds::new(point(9.5, 0.4), size(20.2, 10.))).unwrap();
        assert_eq!(rounded, IntRect::from_ltrb(10, 0, 30, 10).unwrap());
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

    #[test]
    fn character_keys_are_told_from_shortcuts() {
        let key = |key: &str, key_char: Option<&str>, modifiers: gpui::Modifiers| Keystroke {
            modifiers,
            key: key.into(),
            key_char: key_char.map(Into::into),
        };
        let none = gpui::Modifiers::default();
        let shift = gpui::Modifiers::shift();
        let alt_gr = gpui::Modifiers {
            control: true,
            alt: true,
            ..none
        };
        assert!(types_character(&key("a", Some("a"), none)));
        assert!(types_character(&key("a", Some("A"), shift)));
        assert!(types_character(&key("space", Some(" "), none)));
        assert!(types_character(&key("e", Some("€"), alt_gr)));
        // A dead key, whose accent Windows composes with the next key.
        assert!(types_character(&key("´", Some("´"), none)));
        assert!(!types_character(&key(
            "a",
            None,
            gpui::Modifiers::control()
        )));
        assert!(!types_character(&key(
            "e",
            Some("e"),
            gpui::Modifiers::alt()
        )));
        assert!(!types_character(&key("left", None, none)));
        assert!(!types_character(&key("enter", None, none)));
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
