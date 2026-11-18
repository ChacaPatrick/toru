use gpui::{App, Div, Hsla, Rgba, div, prelude::*, px};

use super::geometry::Pt;
use super::{Overlay, geometry};
use crate::theme::ActiveTheme;

/// The magnifier shows this many frame pixels across, each `LOUPE_ZOOM` logical pixels wide.
pub(super) const LOUPE_CELLS: u32 = 15;

pub(super) const LOUPE_ZOOM: f32 = 8.;

impl Overlay {
    /// The magnifier: the frame pixels around the pointer, blown up, with the centre cell
    /// outlined and one-pixel hairlines crossing through it.
    pub(super) fn loupe(&self, pointer: Pt, cx: &App) -> Div {
        let theme = cx.theme();
        let c = &theme.colors;
        let o = &theme.overlay;
        let pixels = geometry::loupe_pixels(&self.frame, pointer, LOUPE_CELLS);
        let cell = px(LOUPE_ZOOM);
        let rows = pixels.chunks(LOUPE_CELLS as usize).map(|row| {
            div().flex().children(row.iter().map(|pixel| {
                div()
                    .w(cell)
                    .h(cell)
                    .bg(pixel.map(pixel_color).unwrap_or(c.toast_bg))
            }))
        });
        let centre = px((LOUPE_CELLS / 2) as f32 * LOUPE_ZOOM);
        let middle = centre + cell / 2. - px(0.5);
        let hairline = Hsla {
            a: 0.5,
            ..o.crosshair
        };
        let inner = px(LOUPE_CELLS as f32 * LOUPE_ZOOM);
        div()
            .relative()
            .flex()
            .flex_col()
            .border(o.loupe.width)
            .border_color(o.loupe.color)
            .children(rows)
            .child(
                div()
                    .absolute()
                    .left(middle)
                    .top_0()
                    .w_px()
                    .h(inner)
                    .bg(hairline),
            )
            .child(
                div()
                    .absolute()
                    .top(middle)
                    .left_0()
                    .h_px()
                    .w(inner)
                    .bg(hairline),
            )
            .child(
                div()
                    .absolute()
                    .left(centre)
                    .top(centre)
                    .w(cell)
                    .h(cell)
                    .border_1()
                    .border_color(o.crosshair),
            )
    }
}

fn pixel_color(pixel: xcap::image::Rgba<u8>) -> Hsla {
    Rgba {
        r: f32::from(pixel[0]) / 255.,
        g: f32::from(pixel[1]) / 255.,
        b: f32::from(pixel[2]) / 255.,
        a: 1.,
    }
    .into()
}
