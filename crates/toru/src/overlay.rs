//! The capture overlay (DESIGN.md §6), as `docs/spikes.md` §1.2 proved; geometry in physical pixels.

mod freeze;
mod geometry;
mod loupe;
mod panel;
mod session;

use std::{rc::Rc, sync::Arc};

use gpui::{
    Context, DispatchPhase, Div, FocusHandle, HitboxBehavior, Hsla, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point, Render, RenderImage,
    Window, canvas, div, img, prelude::*, px,
};
use toru_core::capture::CaptureMode;
use xcap::image::{RgbaImage, imageops};

use crate::theme::ActiveTheme;
use freeze::{MonitorFrame, TopWindow};
use geometry::{Pt, Rect, Selection};
pub use session::start;
use session::{Session, finish};

/// One monitor's overlay.
struct Overlay {
    mode: CaptureMode,
    /// This monitor on the desktop.
    bounds: Rect,
    frame: Arc<RgbaImage>,
    image: Arc<RenderImage>,
    /// Every monitor's frame, for a window that reaches beyond this one.
    frames: Rc<[MonitorFrame]>,
    /// Topmost first. Empty in Region.
    windows: Rc<[TopWindow]>,
    focus: FocusHandle,
    /// The frame pixel under the pointer, once it has moved over this monitor.
    pointer: Option<Pt>,
    /// Releasing confirms it, so it never outlives the drag.
    selection: Option<Selection>,
    /// An index into `windows`. Every overlay holds the same one, so a window spanning monitors
    /// lights up on each.
    picked: Option<usize>,
}

impl Overlay {
    fn frame_size(&self) -> (u32, u32) {
        self.frame.dimensions()
    }

    fn logical(position: Point<Pixels>) -> (f32, f32) {
        (f32::from(position.x), f32::from(position.y))
    }

    fn press(&mut self, position: Point<Pixels>, scale: f32, cx: &mut Context<Self>) {
        if self.mode == CaptureMode::Region {
            let edge = geometry::edge_at(Self::logical(position), scale, self.frame_size());
            self.selection = Some(Selection::new(edge));
            cx.notify();
        }
    }

    /// Perhaps while dragging past this monitor's edge.
    fn moved(&mut self, position: Point<Pixels>, scale: f32, cx: &mut Context<Self>) {
        let position = Self::logical(position);
        let frame = self.frame_size();
        let pixel = geometry::pixel_at(position, scale, frame);
        self.pointer = Some(pixel);
        match self.mode {
            CaptureMode::Region => {
                if let Some(selection) = &mut self.selection {
                    selection.cursor = geometry::edge_at(position, scale, frame);
                }
            }
            _ => {
                let at = (pixel.0 + self.bounds.x, pixel.1 + self.bounds.y);
                let picked = geometry::window_at(self.windows.iter().map(|w| w.bounds), at);
                self.pick(picked, cx);
            }
        }
        cx.notify();
    }

    /// On every overlay, since the window may stretch onto other monitors.
    fn pick(&mut self, picked: Option<usize>, cx: &mut Context<Self>) {
        if self.picked == picked {
            return;
        }
        self.picked = picked;
        cx.defer(move |cx| {
            let Some(session) = cx.try_global::<Session>() else {
                return;
            };
            for overlay in session.overlays.clone() {
                overlay
                    .update(cx, |this, _, cx| {
                        if this.picked != picked {
                            this.picked = picked;
                            cx.notify();
                        }
                    })
                    .ok();
            }
        });
    }

    /// A bare click in Region, or Enter over bare desktop in Window, waits.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let image = match self.mode {
            CaptureMode::Region => self.crop_selection(cx),
            _ => self.crop_picked(),
        };
        if let Some(image) = image {
            // A window can't be closed from inside its own update.
            cx.defer(move |cx| finish(Some(image), cx));
        }
    }

    fn crop_selection(&mut self, cx: &mut Context<Self>) -> Option<RgbaImage> {
        let selection = self.selection.take()?;
        if selection.is_empty() {
            cx.notify();
            return None;
        }
        let Rect {
            x,
            y,
            width,
            height,
        } = selection.rect();
        let crop = imageops::crop_imm(
            self.frame.as_ref(),
            x as u32,
            y as u32,
            width as u32,
            height as u32,
        )
        .to_image();
        Some(crop)
    }

    /// A window spanning two monitors is stitched from both frames.
    fn crop_picked(&self) -> Option<RgbaImage> {
        let window = self.windows.get(self.picked?)?;
        let monitors: Vec<(Rect, &RgbaImage)> = self
            .frames
            .iter()
            .map(|monitor| (monitor.bounds, monitor.frame.as_ref()))
            .collect();
        geometry::compose(window.bounds, &monitors)
    }

    fn cancel(&self, cx: &mut Context<Self>) {
        cx.defer(|cx| finish(None, cx));
    }

    /// In frame pixels.
    fn lit(&self, frame: Rect) -> Option<Rect> {
        match self.mode {
            CaptureMode::Region => self.selection.map(Selection::rect),
            _ => {
                let window = self.windows.get(self.picked?)?;
                window
                    .bounds
                    .relative_to((self.bounds.x, self.bounds.y))
                    .intersect(frame)
            }
        }
    }

    /// A plain div's listeners stop once a drag crosses onto the next monitor, although Windows
    /// keeps sending moves and the release to the window holding the mouse (`docs/spikes.md`
    /// §1.2). Capturing the pointer on the press keeps them coming until the button comes up.
    ///
    /// The capture is checked as `captured_hitbox().is_some()`, not by the hitbox's id: GPUI hands
    /// out a fresh `HitboxId` on every redraw, so after the press redraws no listener holds the
    /// captured id. This layer is the only thing in the window that captures.
    fn mouse_layer(&self, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let id = hitbox.id;
                let on_down = view.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && id.is_hovered(window)
                    {
                        window.capture_pointer(id);
                        let scale = window.scale_factor();
                        on_down.update(cx, |this, cx| this.press(event.position, scale, cx));
                    }
                });
                let on_move = view.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && (window.captured_hitbox().is_some() || id.is_hovered(window))
                    {
                        let scale = window.scale_factor();
                        on_move.update(cx, |this, cx| this.moved(event.position, scale, cx));
                    }
                });
                // A release anywhere ends the drag. GPUI clears the capture after this event.
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && (window.captured_hitbox().is_some() || id.is_hovered(window))
                    {
                        view.update(cx, |this, cx| this.confirm(cx));
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }
}

/// Dividing by the scale keeps its edges on device pixels.
fn place(rect: Rect, scale: f32) -> Div {
    let logical = |value: i32| px(value as f32 / scale);
    div()
        .absolute()
        .left(logical(rect.x))
        .top(logical(rect.y))
        .w(logical(rect.width))
        .h(logical(rect.height))
}

/// Drawn `outside` logical pixels beyond the rect's edge, so the pixels inside stay untouched.
fn outline(rect: Rect, scale: f32, outside: Pixels, width: Pixels, color: Hsla) -> Div {
    let logical = |value: i32| px(value as f32 / scale);
    div()
        .absolute()
        .left(logical(rect.x) - outside)
        .top(logical(rect.y) - outside)
        .w(logical(rect.width) + outside * 2.)
        .h(logical(rect.height) + outside * 2.)
        .border(width)
        .border_color(color)
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let (frame_width, frame_height) = self.frame_size();
        let frame_rect = Rect {
            x: 0,
            y: 0,
            width: frame_width as i32,
            height: frame_height as i32,
        };
        let theme = cx.theme();
        let o = &theme.overlay;
        let rect = self.lit(frame_rect);
        let dimmed = match rect {
            Some(rect) => geometry::around(rect, frame_rect.width, frame_rect.height),
            None => vec![frame_rect],
        };
        // The panel belongs to the monitor the pointer is on. No mouse event reports the pointer
        // leaving, but the window's hovered flag clears; a drag keeps the panel, since the
        // pointer may be past this monitor's edge while the drag goes on.
        let panel = self
            .pointer
            .filter(|_| self.selection.is_some() || window.is_window_hovered())
            .map(|pointer| self.panel(pointer, rect, scale, frame_rect, window, cx));

        div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .map(|this| match self.mode {
                CaptureMode::Region => this.cursor_crosshair(),
                _ => this.cursor_default(),
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.cancel(cx),
                    "enter" => this.confirm(cx),
                    _ => {}
                }
            }))
            .child(
                img(self.image.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .children(dimmed.into_iter().map(|part| place(part, scale).bg(o.dim)))
            .children(
                rect.filter(|rect| rect.width > 0 && rect.height > 0)
                    .map(|rect| {
                        let edge = o.edge.width;
                        div()
                            .child(outline(rect, scale, edge + px(1.), px(1.), o.edge_halo))
                            .child(outline(rect, scale, edge, edge, o.edge.color))
                    }),
            )
            .children(panel)
            .child(self.mouse_layer(cx))
    }
}
