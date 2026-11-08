//! The Block tool's effects (DESIGN.md §5): Blur, Pixelate and Solid hide part of the picture.
//! Blur and Pixelate are computed from the picture with the blocks under them applied, never
//! from the strokes drawn over it, so a region rendered on its own comes out the same as the
//! whole, a mark under a block is hidden with the picture, and a block over another shows only
//! what the first left: adding a block never reveals more. Both begin by averaging the picture
//! into cells, so only those averages reach the saved file: text under a block can't be read
//! back out of it. Solid paints the chosen colour.

use tiny_skia::{Color, IntRect, IntSize, Pixmap};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockKind {
    Blur,
    Pixelate,
    Solid,
}

impl BlockKind {
    pub const ALL: [BlockKind; 3] = [BlockKind::Blur, BlockKind::Pixelate, BlockKind::Solid];

    pub fn label(self) -> &'static str {
        match self {
            BlockKind::Blur => "Blur",
            BlockKind::Pixelate => "Pixelate",
            BlockKind::Solid => "Solid",
        }
    }
}

/// A cell is a quarter of the block's shorter side, and at least this: a tight block around a
/// line of small text and a tall one around a headline both come out at a cell or two per
/// glyph, which no eye can read. There is no upper limit: one would let large text through.
const CELL_MIN: u32 = 8;
const CELL_FRACTION: u32 = 4;
/// Blur smooths the cell grid this many times with a 3×3 box before scaling it back up.
const BLUR_PASSES: usize = 2;

/// The cell size a block `rect` big is averaged into.
pub fn cell_size(rect: IntRect) -> u32 {
    (rect.width().min(rect.height()) / CELL_FRACTION).max(CELL_MIN)
}

/// The block over `rect` of `base`, the picture with the blocks under this one applied: the
/// part of `rect` within the base, and its pixels. `None` if it misses the base.
pub fn render(
    kind: BlockKind,
    base: &Pixmap,
    rect: IntRect,
    color: Color,
) -> Option<(IntRect, Pixmap)> {
    let rect = rect.intersect(&IntRect::from_xywh(0, 0, base.width(), base.height())?)?;
    let mut data = vec![255u8; (rect.width() * rect.height() * 4) as usize];
    match kind {
        BlockKind::Solid => {
            let c = color.to_color_u8();
            for pixel in data.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[c.red(), c.green(), c.blue(), 255]);
            }
        }
        BlockKind::Pixelate => {
            let cells = Cells::average(base, rect);
            for (i, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (x, y) = (i as u32 % rect.width(), i as u32 / rect.width());
                pixel[..3].copy_from_slice(&cells.at(x / cells.size, y / cells.size));
            }
        }
        BlockKind::Blur => {
            let mut cells = Cells::average(base, rect);
            for _ in 0..BLUR_PASSES {
                cells = cells.smoothed();
            }
            for (i, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (x, y) = (i as u32 % rect.width(), i as u32 / rect.width());
                pixel[..3].copy_from_slice(&cells.sample(x, y));
            }
        }
    }
    let pixmap = Pixmap::from_vec(data, IntSize::from_wh(rect.width(), rect.height())?)?;
    Some((rect, pixmap))
}

/// The block's rectangle averaged into a grid of cells, anchored at its top-left corner; a
/// cell at the right or bottom edge may be cut short and averages what it covers.
struct Cells {
    size: u32,
    columns: u32,
    rows: u32,
    /// Row-major RGB, as `f32` so smoothing doesn't round until the end.
    colors: Vec<[f32; 3]>,
}

impl Cells {
    fn average(base: &Pixmap, rect: IntRect) -> Cells {
        let size = cell_size(rect);
        let columns = rect.width().div_ceil(size);
        let rows = rect.height().div_ceil(size);
        let mut sums = vec![[0f32; 3]; (columns * rows) as usize];
        let mut counts = vec![0u32; (columns * rows) as usize];
        let pixels = base.pixels();
        for y in 0..rect.height() {
            let row = (rect.y() as u32 + y) * base.width() + rect.x() as u32;
            for x in 0..rect.width() {
                let p = pixels[(row + x) as usize];
                let cell = ((y / size) * columns + x / size) as usize;
                sums[cell][0] += p.red() as f32;
                sums[cell][1] += p.green() as f32;
                sums[cell][2] += p.blue() as f32;
                counts[cell] += 1;
            }
        }
        let colors = sums
            .iter()
            .zip(&counts)
            .map(|(sum, count)| sum.map(|v| v / (*count).max(1) as f32))
            .collect();
        Cells {
            size,
            columns,
            rows,
            colors,
        }
    }

    fn color(&self, column: u32, row: u32) -> [f32; 3] {
        self.colors[(row * self.columns + column) as usize]
    }

    /// The cell's colour as bytes.
    fn at(&self, column: u32, row: u32) -> [u8; 3] {
        self.color(column, row).map(|v| v.round() as u8)
    }

    /// Every cell averaged with its neighbours, the grid's edges counting their own cells again.
    fn smoothed(&self) -> Cells {
        let mut colors = Vec::with_capacity(self.colors.len());
        for row in 0..self.rows as i64 {
            for column in 0..self.columns as i64 {
                let mut sum = [0f32; 3];
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let c = (column + dx).clamp(0, self.columns as i64 - 1) as u32;
                        let r = (row + dy).clamp(0, self.rows as i64 - 1) as u32;
                        let color = self.color(c, r);
                        for (s, v) in sum.iter_mut().zip(color) {
                            *s += v;
                        }
                    }
                }
                colors.push(sum.map(|v| v / 9.));
            }
        }
        Cells { colors, ..*self }
    }

    /// The colour at pixel `(x, y)` of the block, blended between the four nearest cell
    /// centres, so the grid scales back up without steps.
    fn sample(&self, x: u32, y: u32) -> [u8; 3] {
        let position = |v: u32, count: u32| {
            let at = ((v as f32 + 0.5) / self.size as f32 - 0.5).clamp(0., (count - 1) as f32);
            let low = at.floor();
            (low as u32, (low as u32 + 1).min(count - 1), at - low)
        };
        let (c0, c1, tx) = position(x, self.columns);
        let (r0, r1, ty) = position(y, self.rows);
        let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        let top = mix(self.color(c0, r0), self.color(c1, r0), tx);
        let bottom = mix(self.color(c0, r1), self.color(c1, r1), tx);
        mix(top, bottom, ty).map(|v| v.round() as u8)
    }
}

impl Clone for Cells {
    fn clone(&self) -> Cells {
        Cells {
            colors: self.colors.clone(),
            ..*self
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A picture of text-like detail: 2px strokes in 8×16 glyph boxes on white, with a rule
    /// under each line, so every cell holds both ink and paper.
    pub fn text_like(width: u32, height: u32) -> Pixmap {
        text_like_scaled(width, height, 1)
    }

    /// `text_like` with every glyph `scale` times as big: a headline rather than body text.
    pub fn text_like_scaled(width: u32, height: u32, scale: u32) -> Pixmap {
        let s = scale as f32;
        let mut picture = Pixmap::new(width, height).unwrap();
        picture.fill(Color::WHITE);
        let ink = tiny_skia::Paint {
            shader: tiny_skia::Shader::SolidColor(Color::BLACK),
            ..Default::default()
        };
        let mut fill = |x: f32, y: f32, w: f32, h: f32| {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                picture.fill_rect(rect, &ink, tiny_skia::Transform::identity(), None);
            }
        };
        for line in 0..height / (16 * scale) {
            let top = (line * 16 * scale) as f32 + 2. * s;
            for glyph in 0..width / (8 * scale) {
                let left = (glyph * 8 * scale) as f32 + s;
                // A stem, a bar and, on every other glyph, a bowl.
                fill(left, top, 2. * s, 10. * s);
                fill(left, top + (glyph % 3) as f32 * 3. * s, 5. * s, 2. * s);
                if glyph % 2 == 0 {
                    fill(left + 4. * s, top + 2. * s, 2. * s, 6. * s);
                }
            }
            fill(0., top + 12. * s, width as f32, s);
        }
        picture
    }

    /// How much neighbouring pixels differ, summed over the picture: the detail left in it.
    pub fn high_frequency(picture: &Pixmap) -> f64 {
        let (w, h) = (picture.width(), picture.height());
        let luma = |x: u32, y: u32| {
            let p = picture.pixel(x, y).unwrap();
            p.red() as f64 + p.green() as f64 + p.blue() as f64
        };
        let mut total = 0.;
        for y in 0..h {
            for x in 0..w {
                if x + 1 < w {
                    total += (luma(x + 1, y) - luma(x, y)).abs();
                }
                if y + 1 < h {
                    total += (luma(x, y + 1) - luma(x, y)).abs();
                }
            }
        }
        total
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> IntRect {
        IntRect::from_xywh(x, y, w, h).unwrap()
    }

    #[test]
    fn cells_are_a_quarter_of_the_short_side_and_never_tiny() {
        assert_eq!(cell_size(rect(0, 0, 400, 20)), CELL_MIN);
        assert_eq!(cell_size(rect(0, 0, 400, 60)), 15);
        assert_eq!(cell_size(rect(0, 0, 400, 300)), 75);
        assert_eq!(cell_size(rect(0, 0, 3840, 2160)), 540);
    }

    #[test]
    fn large_text_is_hidden_as_well_as_small() {
        // A headline, its glyphs 160×320 as on a 4K capture, with a block tight around the line.
        let scale = 20;
        let base = text_like_scaled(1280, 320, scale);
        let block = rect(0, 0, 1280, 320);
        let size = cell_size(block);
        // No more than two cells across a glyph and four down it, as for small text.
        assert!(size * 2 >= 8 * scale, "a cell of {size}");
        assert!(size * 4 >= 16 * scale);
        // Pixelate leaves nothing finer than a cell, and Blur no step steeper than a cell's
        // slope: the 40px stems, each a step of 255, are gone from both.
        let (_, pixelated) = render(BlockKind::Pixelate, &base, block, Color::WHITE).unwrap();
        for y in 0..pixelated.height() {
            for x in 0..pixelated.width() {
                let corner = pixelated.pixel(x / size * size, y / size * size).unwrap();
                assert_eq!(pixelated.pixel(x, y).unwrap(), corner, "pixel ({x}, {y})");
            }
        }
        let (_, blurred) = render(BlockKind::Blur, &base, block, Color::WHITE).unwrap();
        let steps = (0..blurred.height())
            .flat_map(|y| (1..blurred.width()).map(move |x| (x, y)))
            .map(|(x, y)| {
                let a = blurred.pixel(x - 1, y).unwrap().red() as i32;
                let b = blurred.pixel(x, y).unwrap().red() as i32;
                (a - b).abs()
            })
            .max()
            .unwrap();
        assert!(steps <= 255 / size as i32, "a step of {steps}");
    }

    #[test]
    fn pixelate_and_blur_leave_no_detail_finer_than_a_cell() {
        let base = text_like(160, 64);
        let block = rect(5, 3, 150, 58);
        let before = {
            let (_, part) = render(BlockKind::Solid, &base, block, Color::WHITE).unwrap();
            let mut original = Pixmap::new(part.width(), part.height()).unwrap();
            original.draw_pixmap(
                -block.x(),
                -block.y(),
                base.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
            high_frequency(&original)
        };
        assert!(before > 0.);
        let (_, pixelated) = render(BlockKind::Pixelate, &base, block, Color::WHITE).unwrap();
        // Every pixel of a cell is the same: nothing finer than a cell survives.
        let size = cell_size(block);
        for y in 0..pixelated.height() {
            for x in 0..pixelated.width() {
                let corner = pixelated.pixel(x / size * size, y / size * size).unwrap();
                assert_eq!(pixelated.pixel(x, y).unwrap(), corner, "pixel ({x}, {y})");
            }
        }
        // The strokes were 2px apart; what is left changes only at cell edges.
        assert!(high_frequency(&pixelated) < before * 0.1);
        let (_, blurred) = render(BlockKind::Blur, &base, block, Color::WHITE).unwrap();
        assert!(high_frequency(&blurred) < before * 0.05);
        // No step between neighbours is more than a cell's slope could make.
        let steps = (0..blurred.height())
            .flat_map(|y| (1..blurred.width()).map(move |x| (x, y)))
            .map(|(x, y)| {
                let a = blurred.pixel(x - 1, y).unwrap().red() as i32;
                let b = blurred.pixel(x, y).unwrap().red() as i32;
                (a - b).abs()
            })
            .max()
            .unwrap();
        assert!(steps <= 255 / size as i32, "a step of {steps}");
        // Neither is white or black anywhere: ink and paper have been mixed.
        for part in [&pixelated, &blurred] {
            for p in part.pixels() {
                assert!(p.red() > 40 && p.red() < 240);
                assert_eq!(p.alpha(), 255);
            }
        }
    }

    #[test]
    fn solid_paints_the_colour_and_a_flat_picture_stays_flat() {
        let mut base = Pixmap::new(30, 20).unwrap();
        base.fill(Color::from_rgba8(10, 200, 30, 255));
        let (area, solid) = render(
            BlockKind::Solid,
            &base,
            rect(-5, 5, 20, 40),
            Color::from_rgba8(1, 2, 3, 255),
        )
        .unwrap();
        assert_eq!(area, rect(0, 5, 15, 15));
        assert!(solid.pixels().iter().all(|p| p.red() == 1 && p.blue() == 3));
        for kind in [BlockKind::Blur, BlockKind::Pixelate] {
            let (_, part) = render(kind, &base, rect(0, 0, 30, 20), Color::WHITE).unwrap();
            assert!(
                part.pixels()
                    .iter()
                    .all(|p| p.red() == 10 && p.green() == 200 && p.blue() == 30)
            );
        }
        assert!(render(BlockKind::Blur, &base, rect(40, 0, 5, 5), Color::WHITE).is_none());
    }

    #[test]
    fn a_partial_cell_averages_what_it_covers() {
        // 12 wide: one full cell of 8 and one of 4; the right 4 columns are black.
        let mut base = Pixmap::new(12, 8).unwrap();
        base.fill(Color::WHITE);
        base.fill_rect(
            tiny_skia::Rect::from_xywh(8., 0., 4., 8.).unwrap(),
            &tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(Color::BLACK),
                ..Default::default()
            },
            tiny_skia::Transform::identity(),
            None,
        );
        let (_, part) =
            render(BlockKind::Pixelate, &base, rect(0, 0, 12, 8), Color::WHITE).unwrap();
        assert_eq!(part.pixel(2, 2).unwrap().red(), 255);
        assert_eq!(part.pixel(10, 2).unwrap().red(), 0);
    }
}
