use gpui::{Context, Pixels, Window, prelude::*};

use super::{FETCH_AHEAD, Gallery, Group, GroupKind, RETRY_FETCH_AFTER};
use crate::main_window::history::{self, Next, Page};

impl Gallery {
    /// Called each frame, so days keep coming until the view has filled.
    pub(super) fn fetch_when_near_bottom(
        &mut self,
        content: Pixels,
        scrolled: Pixels,
        viewport: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.show_previous
            || !self.query.is_empty()
            || self.fetch.is_some()
            || self.next == Next::End
        {
            return;
        }
        if !history::near_bottom(content, scrolled, viewport, FETCH_AHEAD) {
            return;
        }
        let (store, session, today, next) =
            (self.store.clone(), self.session, self.today, self.next);
        self.fetch = Some(cx.spawn_in(window, async move |this, cx| {
            let page = cx
                .background_spawn(async move { history::fetch_page(&store, session, today, next) })
                .await;
            match page {
                Ok(page) => {
                    this.update_in(cx, |this, window, cx| this.show_page(page, window, cx))
                        .ok();
                }
                Err(_) => {
                    this.update(cx, |this, cx| {
                        this.fetch_failed = true;
                        this.rebuild_rows(cx);
                        cx.notify();
                    })
                    .ok();
                    cx.background_executor().timer(RETRY_FETCH_AFTER).await;
                    this.update(cx, |this, cx| {
                        this.fetch = None;
                        cx.notify();
                    })
                    .ok();
                }
            }
        }));
    }

    fn show_page(&mut self, page: Option<Page>, window: &mut Window, cx: &mut Context<Self>) {
        self.fetch = None;
        self.fetch_failed = false;
        self.next = page.as_ref().map_or(Next::End, |page| page.next);
        if let Some(page) = page.filter(|page| !page.cards.is_empty()) {
            self.remake_narrow_or_missing_thumbnails(&page.cards, window, cx);
            let cards = page
                .cards
                .into_iter()
                .map(|new| self.new_card(new, window, cx))
                .collect();
            self.groups.push(Group {
                kind: GroupKind::Day(page.day),
                cards,
                collapsed: false,
                drawn: false,
            });
        }
        self.rebuild_rows(cx);
        cx.notify();
    }

    pub fn set_show_previous(&mut self, on: bool, cx: &Context<Self>) {
        if self.show_previous != on {
            self.show_previous = on;
            self.rebuild_rows(cx);
        }
    }

    pub(super) fn toggle_collapsed(&mut self, group: usize, cx: &mut Context<Self>) {
        self.groups[group].collapsed = !self.groups[group].collapsed;
        self.rebuild_rows(cx);
        cx.notify();
    }
}
