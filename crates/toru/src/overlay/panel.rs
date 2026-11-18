use gpui::{App, Div, Window, div, prelude::*, px};
use toru_core::capture::CaptureMode;

use super::geometry::{Pt, Rect};
use super::loupe::{LOUPE_CELLS, LOUPE_ZOOM};
use super::{Overlay, geometry};
use crate::theme::{ActiveTheme, ThemedText};

/// From the pointer, in logical pixels, as are the other sizes here.
const PANEL_GAP: f32 = 20.;

const PANEL_PADDING: f32 = 10.;

const PANEL_SPACING: f32 = 8.;

const TITLE_MAX_WIDTH: f32 = 360.;

impl Overlay {
    pub(super) fn panel(
        &self,
        pointer: Pt,
        rect: Option<Rect>,
        scale: f32,
        frame: Rect,
        window: &Window,
        cx: &App,
    ) -> Div {
        let theme = cx.theme();
        let c = &theme.colors;
        let o = &theme.overlay;
        let readout_style = theme.type_scale.readout;
        let title_style = theme.type_scale.toast;
        let hint_style = theme.type_scale.hint;
        let region = self.mode == CaptureMode::Region;
        let picked = self.picked.and_then(|index| self.windows.get(index));

        // In Window the whole window's size, not just the part on this monitor.
        let readout = match (picked, rect) {
            (Some(window), _) if !region => geometry::size_text(window.bounds),
            (_, Some(rect)) => geometry::size_text(rect),
            (_, None) => geometry::position_text(pointer),
        };
        let title = picked
            .filter(|_| !region)
            .map(|window| window.title.clone());
        let hint = match (region, rect.is_some()) {
            (true, true) => "Release or Enter to capture · Esc cancels",
            (true, false) => "Drag to select · Esc cancels",
            (false, true) => "Click or Enter to capture · Esc cancels",
            (false, false) => "Point at a window · Esc cancels",
        };

        let loupe_size = if region {
            LOUPE_CELLS as f32 * LOUPE_ZOOM + f32::from(o.loupe.width) * 2.
        } else {
            0.
        };
        let readout_width = f32::from(readout_style.text_width(&readout, theme, window));
        let title_width = title.as_ref().map_or(0., |title| {
            f32::from(title_style.text_width(title, theme, window)).min(TITLE_MAX_WIDTH)
        });
        let hint_width = f32::from(hint_style.text_width(hint, theme, window));
        let readout_height = f32::from(readout_style.natural_line_height(theme, cx));
        let title_height = title.as_ref().map_or(0., |_| {
            f32::from(title_style.natural_line_height(theme, cx))
        });
        let hint_height = f32::from(hint_style.natural_line_height(theme, cx));
        let border = f32::from(theme.borders.toast.width);
        let panel_width = loupe_size
            .max(readout_width)
            .max(title_width)
            .max(hint_width)
            + 2. * (PANEL_PADDING + border);
        let panel_height = loupe_size
            + if region { PANEL_SPACING } else { 0. }
            + readout_height
            + title_height
            + hint_height
            + 2. * (PANEL_PADDING + border);

        let pointer_logical = (pointer.0 as f32 / scale, pointer.1 as f32 / scale);
        let frame_logical = (frame.width as f32 / scale, frame.height as f32 / scale);
        let (left, top) = geometry::panel_origin(
            pointer_logical,
            (panel_width, panel_height),
            frame_logical,
            PANEL_GAP,
        );

        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(panel_width))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(PANEL_SPACING))
            .p(px(PANEL_PADDING))
            .bg(c.toast_bg)
            .text_color(c.toast_text)
            .border(theme.borders.toast.width)
            .border_color(theme.borders.toast.color)
            .rounded(theme.radii.md)
            .shadow(theme.shadows.pop.clone())
            .when(region, |panel| {
                panel.child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .child(self.loupe(pointer, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .child(
                        div()
                            .type_style(theme, readout_style)
                            .line_height(px(readout_height))
                            .whitespace_nowrap()
                            .child(readout_style.case.apply(&readout)),
                    )
                    .children(title.map(|title| {
                        div()
                            .max_w(px(TITLE_MAX_WIDTH))
                            .type_style(theme, title_style)
                            .line_height(px(title_height))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title_style.case.apply(&title))
                    }))
                    .child(
                        div()
                            .type_style(theme, hint_style)
                            .line_height(px(hint_height))
                            .whitespace_nowrap()
                            .child(hint_style.case.apply(hint)),
                    ),
            )
    }
}
