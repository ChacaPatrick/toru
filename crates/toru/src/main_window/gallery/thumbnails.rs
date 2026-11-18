use gpui::{Context, Pixels, Resource, Window, prelude::*, px};
use toru_core::store::CaptureId;

use super::{Card, Gallery, MAX_CARD_WIDTH, NewCard};
use crate::main_window::card;

impl Gallery {
    pub fn thumbnail_width(&self, size: (u32, u32)) -> u32 {
        thumbnail_width_for(size, self.scale)
    }

    pub(super) fn release_thumbnail(
        &self,
        card: &Card,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = Resource::from(card.files.thumbnail.clone());
        self.thumbnails
            .update(cx, |thumbnails, cx| thumbnails.remove(&source, window, cx));
    }

    /// Thumbnails hold GPU memory, so a group with none of its cards near the view lets go of
    /// them.
    pub(super) fn release_far_thumbnails(
        &mut self,
        near: &[bool],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (index, near) in near.iter().enumerate() {
            if self.groups[index].drawn && !near {
                self.groups[index].drawn = false;
                for card in &self.groups[index].cards {
                    self.release_thumbnail(card, window, cx);
                }
            }
        }
    }

    /// Thumbnails stored before the cards could grow, or at a lower display scale, are narrower
    /// than a card needs, so the card would stretch them.
    pub(super) fn remake_narrow_or_missing_thumbnails(
        &self,
        cards: &[NewCard],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let wanted: Vec<_> = cards
            .iter()
            .map(|card| {
                let size = (card.capture.width, card.capture.height);
                (card.files.id, self.thumbnail_width(size))
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let remade: Vec<_> = cx
                .background_spawn(async move {
                    wanted
                        .into_iter()
                        .filter(|&(id, width)| {
                            store
                                .remake_narrow_or_missing_thumbnail(id, width)
                                .unwrap_or(false)
                        })
                        .map(|(id, _)| id)
                        .collect()
                })
                .await;
            if remade.is_empty() {
                return;
            }
            this.update_in(cx, |this, window, cx| {
                for capture in remade {
                    this.reload_thumbnail(capture, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn reload_thumbnail(
        &mut self,
        capture: CaptureId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        let source = Resource::from(card.files.thumbnail.clone());
        self.thumbnails
            .update(cx, |thumbnails, cx| thumbnails.remove(&source, window, cx));
        cx.notify();
    }

    /// A saved card's `bytes` are its file's, which going Back doesn't touch.
    pub fn picture_changed(
        &mut self,
        capture: CaptureId,
        (width, height): (u32, u32),
        bytes: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.stored_card(capture) else {
            return;
        };
        card.capture.width = width;
        card.capture.height = height;
        if !card.capture.is_saved() {
            card.bytes = bytes;
        }
        self.reload_thumbnail(capture, window, cx);
    }
}

fn physical_width(width: Pixels, scale: f32) -> u32 {
    (f32::from(width) * scale).ceil().max(1.) as u32
}

/// No wider than it must be to stay sharp, since GPU memory grows quickly with it
/// (`docs/spikes.md` §1.3): the widest card in physical pixels, or, for a picture so wide that
/// it would then be shorter than a card's thumbnail, which it fills, wide enough to be that
/// tall. The store never makes a thumbnail wider than its picture.
pub(super) fn thumbnail_width_for((width, height): (u32, u32), scale: f32) -> u32 {
    let widest_card = physical_width(px(MAX_CARD_WIDTH), scale);
    let thumb_height = u64::from(physical_width(px(card::THUMB_HEIGHT), scale));
    let fills_height = (thumb_height * u64::from(width)).div_ceil(u64::from(height.max(1)));
    widest_card.max(u32::try_from(fills_height).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_widths_are_physical_pixels() {
        assert_eq!(physical_width(px(304.), 1.), 304);
        assert_eq!(physical_width(px(304.5), 2.), 609);
        assert_eq!(physical_width(px(0.), 1.), 1);
    }

    #[test]
    fn a_wide_picture_gets_a_thumbnail_tall_enough_to_fill_the_card() {
        assert_eq!(thumbnail_width_for((1920, 1080), 1.), 410);
        assert_eq!(thumbnail_width_for((800, 1000), 1.5), 615);
        // A strip 12 times as wide as it is tall fills the 140px thumbnail at 1680 wide.
        assert_eq!(thumbnail_width_for((1200, 100), 1.), 1680);
        assert_eq!(thumbnail_width_for((1200, 100), 2.), 3360);
        // A picture with no height can't divide by it.
        assert_eq!(thumbnail_width_for((10, 0), 1.), 1400);
    }
}
