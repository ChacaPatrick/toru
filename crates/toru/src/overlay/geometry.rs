//! The overlay's arithmetic, all in physical pixels, where x grows to the right and y downward:
//! a monitor's frame for the drag and the magnifier, and the desktop, where monitors and windows
//! have their places, for picking a window. Nothing here touches GPUI, so every rule has a test.

use xcap::image::{Rgba, RgbaImage, imageops};

/// A frame position: a pixel's column and row, or the edge before them.
pub type Pt = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height
    }

    /// Whether the pixel at `at` is one of this rect's.
    pub fn contains(self, at: Pt) -> bool {
        (self.x..self.right()).contains(&at.0) && (self.y..self.bottom()).contains(&at.1)
    }

    /// The pixels in both rects, if there are any.
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then(|| Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    /// The smallest rect holding both.
    pub fn union(self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect {
            x,
            y,
            width: self.right().max(other.right()) - x,
            height: self.bottom().max(other.bottom()) - y,
        }
    }

    /// This rect in the coordinates of a frame whose origin is at `origin` on the desktop.
    pub fn relative_to(self, origin: Pt) -> Rect {
        Rect {
            x: self.x - origin.0,
            y: self.y - origin.1,
            ..self
        }
    }
}

/// A drag in progress: the edge where the button went down and the edge under the pointer now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pt,
    pub cursor: Pt,
}

impl Selection {
    pub fn new(anchor: Pt) -> Selection {
        Selection {
            anchor,
            cursor: anchor,
        }
    }

    /// The pixels between the two edges, whichever way the drag went: a 400 px drag is 400 wide.
    pub fn rect(self) -> Rect {
        Rect {
            x: self.anchor.0.min(self.cursor.0),
            y: self.anchor.1.min(self.cursor.1),
            width: (self.anchor.0 - self.cursor.0).abs(),
            height: (self.anchor.1 - self.cursor.1).abs(),
        }
    }

    /// A click, or a drag along one line, selects nothing.
    pub fn is_empty(self) -> bool {
        let rect = self.rect();
        rect.width == 0 || rect.height == 0
    }
}

/// The device pixel a window position is. GPUI divided a whole device pixel by the scale in
/// `f32`, so multiplying back and rounding undoes it exactly, where flooring would land one too
/// low whenever the division came out just under.
fn device_pixel(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

/// The frame pixel under a window position, for the magnifier, kept inside the frame.
pub fn pixel_at(position: (f32, f32), scale: f32, frame: (u32, u32)) -> Pt {
    let at = |value: f32, len: u32| device_pixel(value, scale).clamp(0, len as i32 - 1);
    (at(position.0, frame.0), at(position.1, frame.1))
}

/// The pixel edge a drag's corner is on: the near edge of the pixel under the pointer, except on
/// the frame's last column or row, which the pointer cannot go past, where it is the far edge so
/// the frame's edge can be selected. A position outside the frame, which a drag that has crossed
/// onto a neighbouring monitor reports, is clamped to the edge it went past.
pub fn edge_at(position: (f32, f32), scale: f32, frame: (u32, u32)) -> Pt {
    let at = |value: f32, len: u32| {
        let pixel = device_pixel(value, scale);
        let len = len as i32;
        if pixel >= len - 1 { len } else { pixel.max(0) }
    };
    (at(position.0, frame.0), at(position.1, frame.1))
}

/// The parts of a `width` by `height` frame outside `rect`: above, below, left and right of it,
/// skipping any that would be empty. Drawn dimmed, they leave the selection untouched.
pub fn around(rect: Rect, width: i32, height: i32) -> Vec<Rect> {
    let bottom = rect.y + rect.height;
    let right = rect.x + rect.width;
    [
        Rect {
            x: 0,
            y: 0,
            width,
            height: rect.y,
        },
        Rect {
            x: 0,
            y: bottom,
            width,
            height: height - bottom,
        },
        Rect {
            x: 0,
            y: rect.y,
            width: rect.x,
            height: rect.height,
        },
        Rect {
            x: right,
            y: rect.y,
            width: width - right,
            height: rect.height,
        },
    ]
    .into_iter()
    .filter(|part| part.width > 0 && part.height > 0)
    .collect()
}

/// Where a `panel` sits beside the pointer at `cursor`: below and to the right, `gap` away, unless
/// that would leave the `frame`, in which case it flips to the other side on that axis. Whatever
/// is left is kept inside the frame. All in the same units, logical or physical.
pub fn panel_origin(
    cursor: (f32, f32),
    panel: (f32, f32),
    frame: (f32, f32),
    gap: f32,
) -> (f32, f32) {
    let place = |at: f32, size: f32, len: f32| {
        let after = at + gap;
        let origin = if after + size <= len {
            after
        } else {
            at - gap - size
        };
        origin.clamp(0., (len - size).max(0.))
    };
    (
        place(cursor.0, panel.0, frame.0),
        place(cursor.1, panel.1, frame.1),
    )
}

/// The `cells` by `cells` pixels of `frame` centred on `centre`, row by row, with `None` for any
/// that lie outside the frame. `cells` is odd, so the centre pixel is the middle one.
pub fn loupe_pixels(frame: &RgbaImage, centre: Pt, cells: u32) -> Vec<Option<Rgba<u8>>> {
    let reach = cells as i32 / 2;
    let mut pixels = Vec::with_capacity((cells * cells) as usize);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (x, y) = (centre.0 + dx, centre.1 + dy);
            let pixel = u32::try_from(x)
                .ok()
                .zip(u32::try_from(y).ok())
                .and_then(|(x, y)| frame.get_pixel_checked(x, y))
                .copied();
            pixels.push(pixel);
        }
    }
    pixels
}

/// The topmost of `windows`, which are listed topmost first, under the desktop pixel `at`.
pub fn window_at(windows: impl IntoIterator<Item = Rect>, at: Pt) -> Option<usize> {
    windows.into_iter().position(|window| window.contains(at))
}

/// The pixels of `window` as the frozen monitors show them: each monitor's frame is copied
/// where it overlaps the window, and the result is the window cut down to what is on some
/// monitor, so a window hanging off the desktop's edge loses the part no screen shows. A gap
/// between monitors inside the window, which no screen shows either, stays transparent. `None`
/// if no monitor shows any of it.
pub fn compose(window: Rect, monitors: &[(Rect, &RgbaImage)]) -> Option<RgbaImage> {
    let shown = monitors
        .iter()
        .filter_map(|(bounds, _)| window.intersect(*bounds))
        .reduce(Rect::union)?;
    let mut image = RgbaImage::new(shown.width as u32, shown.height as u32);
    for (bounds, frame) in monitors {
        let Some(part) = shown.intersect(*bounds) else {
            continue;
        };
        let from = part.relative_to((bounds.x, bounds.y));
        let to = part.relative_to((shown.x, shown.y));
        let pixels = imageops::crop_imm(
            *frame,
            from.x as u32,
            from.y as u32,
            part.width as u32,
            part.height as u32,
        );
        imageops::replace(&mut image, &*pixels, i64::from(to.x), i64::from(to.y));
    }
    Some(image)
}

/// "400 × 300 px", the readout while dragging.
pub fn size_text(rect: Rect) -> String {
    format!("{} × {} px", rect.width, rect.height)
}

/// "1532, 884", the pointer's pixel on the monitor.
pub fn position_text(pixel: Pt) -> String {
    format!("{}, {}", pixel.0, pixel.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: (u32, u32) = (2560, 1440);

    #[test]
    fn a_drag_covers_the_pixels_between_its_edges_either_way() {
        let down = Selection {
            anchor: (100, 80),
            cursor: (500, 380),
        };
        let up = Selection {
            anchor: (500, 380),
            cursor: (100, 80),
        };
        let expected = Rect {
            x: 100,
            y: 80,
            width: 400,
            height: 300,
        };
        assert_eq!(down.rect(), expected);
        assert_eq!(up.rect(), expected);
        assert!(!down.is_empty());
    }

    #[test]
    fn a_click_or_a_line_is_empty() {
        assert!(Selection::new((10, 10)).is_empty());
        let line = Selection {
            anchor: (10, 10),
            cursor: (300, 10),
        };
        assert!(line.is_empty());
    }

    #[test]
    fn pixel_at_stays_inside_the_frame() {
        assert_eq!(pixel_at((100., 80.), 1., FRAME), (100, 80));
        assert_eq!(pixel_at((-3., 5000.), 1., FRAME), (0, 1439));
    }

    #[test]
    fn pixel_at_scales_logical_positions_to_physical() {
        assert_eq!(pixel_at((100., 80.), 1.5, FRAME), (150, 120));
        assert_eq!(pixel_at((100.5, 80.5), 2., FRAME), (201, 161));
    }

    /// GPUI hands over `pixel / scale` in `f32`; every device pixel must come back as itself.
    #[test]
    fn every_device_pixel_survives_the_round_trip_at_175_percent() {
        let scale = 1.75;
        let frame = (7680, 4320);
        for pixel in 0..7680 {
            let logical = pixel as f32 / scale;
            assert_eq!(
                pixel_at((logical, 0.), scale, frame).0,
                pixel,
                "pixel {pixel}"
            );
            let edge = edge_at((logical, 0.), scale, frame).0;
            let expected = if pixel == 7679 { 7680 } else { pixel };
            assert_eq!(edge, expected, "edge {pixel}");
        }
    }

    #[test]
    fn edge_at_is_the_near_edge_except_on_the_last_column_and_row() {
        assert_eq!(edge_at((100., 80.), 1., FRAME), (100, 80));
        assert_eq!(edge_at((2559., 1439.), 1., FRAME), (2560, 1440));
        assert_eq!(edge_at((2558., 1438.), 1., FRAME), (2558, 1438));
        assert_eq!(edge_at((-1., -1.), 1., FRAME), (0, 0));
        // At scale 2 the last logical pixel is the frame's last physical column too.
        assert_eq!(edge_at((1279.5, 719.5), 2., FRAME), (2560, 1440));
    }

    /// A drag that has crossed onto the next monitor keeps reporting positions, now outside this
    /// frame; the selection stops at the edge it went past.
    #[test]
    fn an_overshoot_past_the_frame_clamps_to_that_edge() {
        assert_eq!(edge_at((2700., 700.), 1., FRAME), (2560, 700));
        assert_eq!(edge_at((-140., 700.), 1., FRAME), (0, 700));
        assert_eq!(edge_at((700., -30.), 1., FRAME), (700, 0));
        assert_eq!(edge_at((700., 1500.), 1., FRAME), (700, 1440));
        assert_eq!(edge_at((-70., 2000.), 2., FRAME), (0, 1440));
        let selection = Selection {
            anchor: edge_at((2000., 1000.), 1., FRAME),
            cursor: edge_at((2900., 1600.), 1., FRAME),
        };
        assert_eq!(
            selection.rect(),
            Rect {
                x: 2000,
                y: 1000,
                width: 560,
                height: 440
            }
        );
    }

    #[test]
    fn a_drag_to_the_corner_selects_the_whole_frame() {
        let selection = Selection {
            anchor: edge_at((0., 0.), 1., FRAME),
            cursor: edge_at((2559., 1439.), 1., FRAME),
        };
        assert_eq!(
            selection.rect(),
            Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440
            }
        );
    }

    #[test]
    fn around_gives_four_parts_that_tile_the_rest_of_the_frame() {
        let rect = Rect {
            x: 100,
            y: 80,
            width: 400,
            height: 300,
        };
        let parts = around(rect, 2560, 1440);
        assert_eq!(parts.len(), 4);
        let area: i32 = parts.iter().map(|p| p.width * p.height).sum();
        assert_eq!(area, 2560 * 1440 - 400 * 300);
        assert!(
            parts
                .iter()
                .all(|p| p.x >= 0 && p.y >= 0 && p.x + p.width <= 2560 && p.y + p.height <= 1440)
        );
        assert!(parts.contains(&Rect {
            x: 500,
            y: 80,
            width: 2060,
            height: 300
        }));
    }

    #[test]
    fn around_drops_the_parts_a_selection_on_the_edge_leaves_empty() {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        };
        assert!(around(rect, 2560, 1440).is_empty());
        let corner = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        assert_eq!(around(corner, 2560, 1440).len(), 2);
    }

    #[test]
    fn panel_sits_below_right_of_the_pointer_with_room_to_spare() {
        assert_eq!(
            panel_origin((500., 400.), (160., 200.), (2560., 1440.), 20.),
            (520., 420.)
        );
    }

    #[test]
    fn panel_flips_to_the_other_side_at_the_frame_edges() {
        // Too far right: flip left. Too far down: flip up.
        assert_eq!(
            panel_origin((2500., 1400.), (160., 200.), (2560., 1440.), 20.),
            (2320., 1180.)
        );
        // Exactly fitting still goes below right.
        assert_eq!(
            panel_origin((2380., 1220.), (160., 200.), (2560., 1440.), 20.),
            (2400., 1240.)
        );
    }

    #[test]
    fn panel_stays_inside_a_frame_too_small_for_either_side() {
        // Flipping left would go negative, so it is clamped to the edge.
        assert_eq!(
            panel_origin((150., 100.), (160., 200.), (300., 250.), 20.),
            (0., 0.)
        );
    }

    #[test]
    fn loupe_pixels_are_row_major_with_none_outside_the_frame() {
        let mut frame = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        frame.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        frame.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
        let pixels = loupe_pixels(&frame, (0, 0), 3);
        assert_eq!(pixels.len(), 9);
        // The row above the frame, then the frame's first row from x = -1.
        assert_eq!(&pixels[..3], &[None, None, None]);
        assert_eq!(pixels[3], None);
        assert_eq!(pixels[4], Some(Rgba([255, 0, 0, 255])));
        assert_eq!(pixels[5], Some(Rgba([0, 255, 0, 255])));
        // The centre is the middle entry.
        let pixels = loupe_pixels(&frame, (1, 0), 3);
        assert_eq!(pixels[4], Some(Rgba([0, 255, 0, 255])));
        assert_eq!(pixels[3], Some(Rgba([255, 0, 0, 255])));
    }

    #[test]
    fn a_rect_contains_its_pixels_but_not_its_far_edges() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        assert!(rect.contains((10, 20)));
        assert!(rect.contains((109, 69)));
        assert!(!rect.contains((110, 20)));
        assert!(!rect.contains((10, 70)));
        assert!(!rect.contains((9, 20)));
    }

    #[test]
    fn rects_intersect_where_they_overlap_and_not_when_they_touch() {
        let a = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let b = Rect {
            x: 50,
            y: -20,
            width: 100,
            height: 50,
        };
        assert_eq!(
            a.intersect(b),
            Some(Rect {
                x: 50,
                y: 0,
                width: 50,
                height: 30
            })
        );
        assert_eq!(
            a.union(b),
            Rect {
                x: 0,
                y: -20,
                width: 150,
                height: 120
            }
        );
        let beside = Rect {
            x: 100,
            y: 0,
            width: 10,
            height: 10,
        };
        assert_eq!(a.intersect(beside), None);
        assert_eq!(
            b.relative_to((50, -20)),
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 50
            }
        );
    }

    /// The owner's second monitor sits above the primary at (8, -1440), so desktop positions
    /// can be negative and windows can hang across the two.
    #[test]
    fn window_at_picks_the_topmost_window_under_the_pointer_on_any_monitor() {
        let windows = [
            Rect {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
            Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
            },
            Rect {
                x: 500,
                y: -1000,
                width: 1200,
                height: 1800,
            },
        ];
        // Inside the first two: the topmost wins.
        assert_eq!(window_at(windows, (150, 150)), Some(0));
        // The maximized window behind it.
        assert_eq!(window_at(windows, (50, 50)), Some(1));
        // On the second monitor, in the window spanning both.
        assert_eq!(window_at(windows, (600, -500)), Some(2));
        // Where the spanning window reaches down over the primary, the maximized one is on top.
        assert_eq!(window_at(windows, (1000, 50)), Some(1));
        // Desktop with nothing on it.
        assert_eq!(window_at(windows, (100, -500)), None);
        assert_eq!(window_at(Vec::new(), (0, 0)), None);
    }

    #[test]
    fn compose_stitches_a_window_from_the_monitors_it_spans() {
        // Two 4x4 monitors, the second above the first and 1 px to the right, like the owner's.
        let lower = (
            Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
            RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])),
        );
        let upper = (
            Rect {
                x: 1,
                y: -4,
                width: 4,
                height: 4,
            },
            RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])),
        );
        let monitors = [(lower.0, &lower.1), (upper.0, &upper.1)];
        // A window from (2, -2) to (4, 2): the top half on the upper monitor, the bottom half on
        // the lower one.
        let window = Rect {
            x: 2,
            y: -2,
            width: 2,
            height: 4,
        };
        let image = compose(window, &monitors).unwrap();
        assert_eq!(image.dimensions(), (2, 4));
        assert_eq!(*image.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
        assert_eq!(*image.get_pixel(1, 1), Rgba([0, 0, 255, 255]));
        assert_eq!(*image.get_pixel(0, 2), Rgba([255, 0, 0, 255]));
        assert_eq!(*image.get_pixel(1, 3), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn compose_cuts_a_window_down_to_what_some_monitor_shows() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let frame = RgbaImage::from_pixel(4, 4, Rgba([9, 9, 9, 255]));
        let monitors = [(monitor, &frame)];
        // Hanging off the left and bottom edges.
        let window = Rect {
            x: -2,
            y: 2,
            width: 4,
            height: 4,
        };
        let image = compose(window, &monitors).unwrap();
        assert_eq!(image.dimensions(), (2, 2));
        assert!(image.pixels().all(|p| *p == Rgba([9, 9, 9, 255])));
        // Entirely off screen.
        let gone = Rect {
            x: 10,
            y: 10,
            width: 4,
            height: 4,
        };
        assert!(compose(gone, &monitors).is_none());
    }

    /// Where two monitors with a gap between them both show part of a window, the gap no screen
    /// shows stays transparent rather than borrowing pixels from either.
    #[test]
    fn compose_leaves_a_gap_between_monitors_transparent() {
        let left = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        let right = Rect {
            x: 3,
            y: 0,
            width: 2,
            height: 2,
        };
        let frame = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let monitors = [(left, &frame), (right, &frame)];
        let window = Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 2,
        };
        let image = compose(window, &monitors).unwrap();
        assert_eq!(image.dimensions(), (5, 2));
        assert_eq!(*image.get_pixel(1, 1), Rgba([1, 2, 3, 255]));
        assert_eq!(*image.get_pixel(2, 1), Rgba([0, 0, 0, 0]));
        assert_eq!(*image.get_pixel(3, 1), Rgba([1, 2, 3, 255]));
    }

    #[test]
    fn texts_read_as_the_design_writes_sizes() {
        let rect = Rect {
            x: 1,
            y: 2,
            width: 400,
            height: 300,
        };
        assert_eq!(size_text(rect), "400 × 300 px");
        assert_eq!(position_text((1532, 884)), "1532, 884");
    }
}
