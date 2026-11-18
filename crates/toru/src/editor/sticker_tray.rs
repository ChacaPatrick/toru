use std::sync::Arc;

use gpui::{Hsla, RenderImage, point};
use tiny_skia::Pixmap;

use super::annotation::{Annotation, Kind, Shape};
use super::sticker::Sticker;
use super::tool_options::style_of;
use super::tools::StrokeOptions;
use super::{Editor, skia, sticker};
use crate::theme::Theme;

/// In logical pixels.
pub const TRAY_ICON_SIZE: f32 = 28.;

/// Keyed on the colours, as the custom palette can change them without changing which palette
/// is shown.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct TrayKey {
    sticker: Hsla,
    line: Hsla,
    scale: f32,
}

impl Editor {
    pub(super) fn refresh_tray(&mut self, theme: &Theme, scale: f32) {
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

    pub fn tray_image(&self, index: usize) -> Option<Arc<RenderImage>> {
        self.tray.get(index).cloned()
    }
}

fn tray_image(sticker: Sticker, theme: &Theme, scale: f32) -> Option<Arc<RenderImage>> {
    let side = (TRAY_ICON_SIZE * scale).round();
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
    Some(skia::render_image(&pixmap))
}

#[cfg(test)]
mod tests {
    use gpui::size;
    use toru_core::settings::Look;

    use super::*;
    use crate::theme::Theme;

    #[test]
    fn the_tray_draws_every_sticker_at_the_scale() {
        let theme = Theme::new(Look::PixelArt, 1);
        for sticker in Sticker::ALL {
            let image = tray_image(sticker, &theme, 1.5).unwrap();
            assert_eq!(image.size(0), size(42.into(), 42.into()));
        }
    }
}
