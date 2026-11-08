//! A value shown for a while, then cleared: the toast and a Copy button's check mark.

use std::time::Duration;

use gpui::Context;

pub struct Flash<T> {
    value: Option<T>,
    /// Counts each `show`, so a timer only clears the value it was started for.
    shown: u64,
}

impl<T: 'static> Flash<T> {
    pub fn new() -> Self {
        Flash {
            value: None,
            shown: 0,
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Shows `value` for `duration`. `flash` finds this `Flash` in the view again when time is up.
    pub fn show<V: 'static>(
        &mut self,
        value: T,
        duration: Duration,
        cx: &mut Context<V>,
        flash: impl Fn(&mut V) -> &mut Flash<T> + 'static,
    ) {
        self.shown += 1;
        let shown = self.shown;
        self.value = Some(value);
        cx.notify();
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(duration).await;
            view.update(cx, |view, cx| {
                let this = flash(view);
                if this.shown == shown {
                    this.value = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}
