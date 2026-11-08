//! Paints a theme `Fill` over its whole parent. Flat and gradient fills are a background; the
//! checker and grid patterns are drawn cell by cell. A thumbnail shows one until it has an image.

use gpui::{AnyElement, Bounds, Hsla, Pixels, Window, canvas, div, fill as quad, prelude::*, px};

use crate::theme::Fill;

/// `radius` rounds a flat or gradient fill to match its parent's corners. The patterns belong to
/// looks with square corners and are left as they are.
pub fn fill(fill: &Fill, radius: Pixels) -> AnyElement {
    match fill.clone() {
        Fill::Checker { a, b, cell } => pattern(b, move |bounds, window| {
            let columns = (bounds.size.width / cell).ceil() as usize;
            let rows = (bounds.size.height / cell).ceil() as usize;
            for row in 0..rows {
                // `a` starts at the second cell, as a repeating conic gradient's first quarter does.
                for column in ((row + 1) % 2..columns).step_by(2) {
                    let origin =
                        bounds.origin + gpui::point(cell * column as f32, cell * row as f32);
                    let square = Bounds::new(origin, gpui::size(cell, cell));
                    window.paint_quad(quad(square.intersect(&bounds), a));
                }
            }
        }),
        Fill::Grid { bg, line, cell } => pattern(bg, move |bounds, window| {
            // As the mockups' repeating gradients draw it: a 1px line closing each cell.
            let mut x = bounds.origin.x + cell - px(1.);
            while x < bounds.right() {
                let column = Bounds::new(
                    gpui::point(x, bounds.origin.y),
                    gpui::size(px(1.), bounds.size.height),
                );
                window.paint_quad(quad(column, line));
                x += cell;
            }
            let mut y = bounds.origin.y + cell - px(1.);
            while y < bounds.bottom() {
                let row = Bounds::new(
                    gpui::point(bounds.origin.x, y),
                    gpui::size(bounds.size.width, px(1.)),
                );
                window.paint_quad(quad(row, line));
                y += cell;
            }
        }),
        flat => div()
            .size_full()
            .rounded(radius)
            .bg(flat.background())
            .into_any_element(),
    }
}

/// `base` under the cells `paint` draws.
fn pattern(base: Hsla, paint: impl Fn(Bounds<Pixels>, &mut Window) + 'static) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            window.paint_quad(quad(bounds, base));
            paint(bounds, window);
        },
    )
    .size_full()
    .into_any_element()
}
