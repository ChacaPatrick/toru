use gpui::{App, Entity, Global, WeakEntity};

#[derive(Default)]
struct OpenMenus(Vec<WeakEntity<bool>>);

impl Global for OpenMenus {}

pub(super) fn menu_opened(open: &Entity<bool>, cx: &mut App) {
    let mut menus = std::mem::take(&mut cx.default_global::<OpenMenus>().0);
    // Menus closed by their own means leave their entry behind until now.
    menus.retain(|menu| menu.upgrade().is_some_and(|menu| *menu.read(cx)));
    menus.push(open.downgrade());
    cx.default_global::<OpenMenus>().0 = menus;
}

pub fn menus_open(cx: &App) -> bool {
    cx.try_global::<OpenMenus>().is_some_and(|menus| {
        menus
            .0
            .iter()
            .any(|menu| menu.upgrade().is_some_and(|menu| *menu.read(cx)))
    })
}

/// Without moving focus. Call it between frames, not during a draw, so the menus are redrawn
/// closed.
pub fn close_menus(cx: &mut App) {
    let menus = std::mem::take(&mut cx.default_global::<OpenMenus>().0);
    for menu in menus.iter().filter_map(WeakEntity::upgrade) {
        menu.update(cx, |open, cx| {
            if *open {
                *open = false;
                cx.notify();
            }
        });
    }
}
