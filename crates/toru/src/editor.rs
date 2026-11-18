//! DESIGN.md §5. Replaces the main window's body while open.

pub mod annotation;
pub mod block;
mod canvas;
pub mod chrome;
mod crop;
pub mod document;
mod history;
mod keyboard;
mod panel;
mod pointer;
mod saving;
pub mod skia;
pub mod sticker;
mod sticker_tray;
pub mod text;
mod text_input;
mod tool_options;
pub mod tools;
mod typing;
mod viewport;
mod zoom;

use std::sync::Arc;

use gpui::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, RenderImage,
    ScrollHandle, SharedString, Window, div, point, prelude::*, px,
};
use gpui_kit::component::input::InputState;
use tiny_skia::IntRect;
use toru_core::{settings::SaveFormat, store::CaptureId};

use crate::theme::ActiveTheme;
use crate::ui::{Flash, FocusInView};
use annotation::Annotation;
use document::{Document, Flattened};
use pointer::Drag;
use sticker::Sticker;
use sticker_tray::TrayKey;
pub use tools::Tool;
use tools::{StrokeOptions, TextSize};
use typing::Typing;
use viewport::Viewport;

pub enum EditorEvent {
    /// `None` when the card already shows the picture.
    Back(Option<Flattened>),
    Copy(Flattened),
    Save(Flattened),
    /// Save pressed with the file already showing this.
    NothingToSave,
    SaveAs(Flattened, SaveFormat),
}

pub struct Editor {
    stored: CaptureId,
    /// Shared with the card, so a rename here is the card's too.
    name: Entity<InputState>,
    name_error: Option<SharedString>,
    document: Document,
    /// Without the mark being dragged. `None` until the next frame renders it again.
    image: Option<Arc<RenderImage>>,
    /// During a drag only the region around the mark is rendered again, laid over `image`, so a
    /// 4K picture isn't rebuilt and uploaded on every pointer move.
    overlay: Option<(IntRect, Arc<RenderImage>)>,
    /// Taken out of the GPU atlas at the next frame, the first to have the window at hand.
    stale_images: Vec<Arc<RenderImage>>,
    tool: Tool,
    options: Vec<(Tool, StrokeOptions)>,
    selected: Option<usize>,
    typing: Option<Typing>,
    tray: Vec<Arc<RenderImage>>,
    tray_key: Option<TrayKey>,
    /// In whole-picture pixels; a rectangle mark, so the marks' handle arithmetic serves it.
    crop_draft: Option<Annotation>,
    viewport: Viewport,
    canvas: Bounds<Pixels>,
    /// Device pixels per logical pixel at the last frame. 100% is one picture pixel per device
    /// pixel, so the picture's logical size depends on it.
    scale: f32,
    /// At Fit the picture follows the canvas as the window resizes; a zoom ends that.
    fitted: bool,
    drag: Option<Drag>,
    /// A slider drag has changed the selected mark: its later changes amend that undo step.
    live_edit: bool,
    /// The root's, so the shortcut letters work wherever focus is inside the editor.
    focus: FocusHandle,
    back_button: FocusHandle,
    /// So Enter crops from the Crop button and the canvas but not from Cancel.
    crop_button: FocusHandle,
    panel_scroll: ScrollHandle,
    panel_focus: FocusInView,
    copied: Flash<()>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.back_button.clone()
    }
}

impl Editor {
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
            back_button: cx.focus_handle().tab_stop(true),
            crop_button: cx.focus_handle().tab_stop(true),
            panel_scroll: ScrollHandle::new(),
            panel_focus: FocusInView::new(cx),
            copied: Flash::new(),
        }
    }

    /// The `demo` start-up argument: the Editor mockup's marks where the mockup has them on its
    /// `1440 × 900` picture, with a second text left being typed.
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

    pub fn title(&self, cx: &App) -> SharedString {
        format!("Toru — editing {}", self.name.read(cx).value()).into()
    }

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

    pub fn stored(&self) -> CaptureId {
        self.stored
    }

    pub fn name_field(&self) -> Entity<InputState> {
        self.name.clone()
    }

    pub fn is_edited(&self) -> bool {
        self.document.is_edited()
            || self.typing.as_ref().is_some_and(|typing| {
                typing
                    .text
                    .text()
                    .is_some_and(|t| !t.content.trim().is_empty())
            })
    }

    /// The main window keeps this in step with the card's.
    pub fn set_name_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        if self.name_error != error {
            self.name_error = error;
            cx.notify();
        }
    }

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

    /// For when the canvas switches between showing the whole picture and the crop.
    fn shown_changed(&mut self, cx: &mut Context<Self>) {
        self.fitted = true;
        if self.measured() {
            self.viewport = self.laid_out();
        }
        self.document_changed(cx);
    }

    pub fn hint(&self) -> &'static str {
        if self.typing.is_some() {
            Tool::TYPING_HINT
        } else {
            self.tool.hint()
        }
    }

    fn select(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.live_edit = false;
        if self.selected != index {
            self.selected = index;
            cx.notify();
        }
    }

    fn selection(&self) -> Option<(usize, &Annotation)> {
        let index = self.selected?;
        Some((index, self.document.annotations().get(index)?))
    }

    fn document_changed(&mut self, cx: &mut Context<Self>) {
        self.stale_images.extend(self.image.take());
        self.preview_changed(cx);
    }

    fn preview_changed(&mut self, cx: &mut Context<Self>) {
        self.stale_images
            .extend(self.overlay.take().map(|(_, image)| image));
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.step_history(Document::undo, cx);
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        self.step_history(Document::redo, cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.step_history(Document::reset, cx);
    }

    fn step_history(&mut self, step: fn(&mut Document) -> bool, cx: &mut Context<Self>) {
        // Text being typed goes on the document first, so the step taken is the one the user sees.
        self.finish_typing(cx);
        // The drag holds the index of the mark it found, which a step (or Delete) would move from
        // under it.
        if self.drag.is_some() {
            return;
        }
        let (crop, shown) = (self.document.crop(), self.shown());
        if !step(&mut self.document) {
            return;
        }
        self.select(None, cx);
        // So Enter can't bring back a crop that was just undone or reset.
        if self.tool == Tool::Crop && self.document.crop() != crop {
            self.crop_draft = self.document_crop_draft();
        }
        if self.shown() == shown {
            self.document_changed(cx);
        } else {
            self.shown_changed(cx);
        }
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        if let Some(index) = self.selected.take() {
            self.document.remove(index, false);
            self.document_changed(cx);
        }
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.stale_images.drain(..) {
            window.drop_image(image).ok();
        }
        self.refresh_tray(cx.theme(), window.scale_factor());
        if self.image.is_none() {
            self.image = Some(skia::render_image(&self.render_document()));
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
                .top(px(chrome::CHROME_INSET))
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

fn on(
    cx: &Context<Editor>,
    f: impl Fn(&mut Editor, &mut Context<Editor>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let editor = cx.entity();
    move |_, cx| editor.update(cx, |this, cx| f(this, cx))
}

fn act<A: 'static>(
    cx: &Context<Editor>,
    f: impl Fn(&mut Editor, A, &mut Context<Editor>) + 'static,
) -> impl Fn(A, &mut Window, &mut App) + 'static {
    let editor = cx.entity();
    move |value, _, cx| editor.update(cx, |this, cx| f(this, value, cx))
}
