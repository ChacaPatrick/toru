//! `.track`: the editor panel's opacity slider. A 4px track filled in the accent up to the knob.
//! A press on the row sets the value from the pointer and starts a drag that follows the pointer
//! wherever it goes until the button comes up; the arrow keys step the value.

use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, HitboxBehavior, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Role, SharedString, Window, canvas, div, prelude::*, px,
    relative,
};

use super::{MIN_TARGET, focus_marks, keyed_focus};
use crate::theme::ActiveTheme;

const TRACK_HEIGHT: f32 = 4.;
const KNOB: f32 = 14.;
const KNOB_BORDER: f32 = 2.;
/// The mockups' `margin: 8px 6px`: the row the track sits in. It is only 20px tall, so the
/// pressable row grows to §7's `MIN_TARGET`, the extra taken from its margins so the track stays
/// where the mockup puts it.
const MARGIN_Y: f32 = 8.;
const MARGIN_X: f32 = 6.;
const ROW_HEIGHT: f32 = TRACK_HEIGHT + MARGIN_Y * 2.;
/// One arrow key's step.
const STEP: f32 = 0.05;

type ChangeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    label: SharedString,
    /// 0 to 1.
    value: f32,
    on_change: Option<ChangeHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, value: f32) -> Slider {
        Slider {
            id: id.into(),
            label: label.into(),
            value: value.clamp(0., 1.),
            on_change: None,
        }
    }

    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Slider {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

/// The value at `x` on a track spanning `track`, clamped to its ends.
pub fn value_at(x: Pixels, track: Bounds<Pixels>) -> f32 {
    let width = f32::from(track.size.width);
    if width <= 0. {
        return 0.;
    }
    (f32::from(x - track.origin.x) / width).clamp(0., 1.)
}

/// Where an arrow key takes `value`: a step either way, or an end. `None` for other keys.
pub fn key_target(key: &str, value: f32) -> Option<f32> {
    Some(match key {
        "left" | "down" => (value - STEP).max(0.),
        "right" | "up" => (value + STEP).min(1.),
        "home" => 0.,
        "end" => 1.,
        _ => return None,
    })
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = keyed_focus(window, cx, &self.id);
        let id = std::sync::Arc::new(self.id.clone());
        // The track's bounds as last laid out, for turning a press into a value.
        let track_bounds = window.use_keyed_state(
            ElementId::NamedChild(id.clone(), "track".into()),
            cx,
            |_, _| Bounds::<Pixels>::default(),
        );
        // Whether a press that began on the row is still held.
        let dragging =
            window.use_keyed_state(ElementId::NamedChild(id, "dragging".into()), cx, |_, _| {
                false
            });
        let theme = cx.theme();
        let c = &theme.colors;
        let value = self.value;
        let on_change = self.on_change;
        let set_from = {
            let on_change = on_change.clone();
            let track_bounds = track_bounds.clone();
            move |x: Pixels, window: &mut Window, cx: &mut App| {
                if let Some(handler) = &on_change {
                    let track = *track_bounds.read(cx);
                    handler(value_at(x, track), window, cx);
                }
            }
        };
        // The row's hitbox takes the press, then keeps the pointer, so the drag goes on off the
        // row and a press that began elsewhere and crosses the row changes nothing.
        let grip = canvas(
            move |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                {
                    let dragging = dragging.clone();
                    let set_from = set_from.clone();
                    let hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase.bubble()
                            && event.button == MouseButton::Left
                            && hitbox.is_hovered(window)
                        {
                            dragging.update(cx, |held, _| *held = true);
                            window.capture_pointer(hitbox.id);
                            set_from(event.position.x, window, cx);
                        }
                    });
                }
                {
                    let dragging = dragging.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if !phase.bubble() || !*dragging.read(cx) {
                            return;
                        }
                        if event.pressed_button == Some(MouseButton::Left) {
                            set_from(event.position.x, window, cx);
                        } else {
                            dragging.update(cx, |held, _| *held = false);
                        }
                    });
                }
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase.bubble() && *dragging.read(cx) {
                        dragging.update(cx, |held, _| *held = false);
                    }
                });
            },
        )
        .absolute()
        .inset_0();
        let on_key_down = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let Some(next) = key_target(&event.keystroke.key, value) else {
                return;
            };
            cx.stop_propagation();
            if let Some(handler) = &on_change {
                handler(next, window, cx);
            }
        };

        let measure = canvas(
            move |bounds, _, cx| track_bounds.update(cx, |track, _| *track = bounds),
            |_, _, _, _| (),
        )
        .absolute()
        .inset_0();
        let track = div()
            .relative()
            .flex_1()
            .h(px(TRACK_HEIGHT))
            .rounded(px(TRACK_HEIGHT / 2.))
            .bg(c.hair)
            .child(measure)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(value))
                    .rounded(px(TRACK_HEIGHT / 2.))
                    .bg(c.accent),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-(KNOB - TRACK_HEIGHT) / 2.))
                    .left(relative(value))
                    .ml(px(-KNOB / 2.))
                    .size(px(KNOB))
                    .rounded_full()
                    .bg(c.surface)
                    .border(px(KNOB_BORDER))
                    .border_color(c.accent),
            );
        div()
            .id(self.id)
            .role(Role::Slider)
            .aria_label(self.label)
            .track_focus(&focus)
            .relative()
            .flex()
            .items_center()
            .w_full()
            .h(px(MIN_TARGET))
            .my(px((ROW_HEIGHT - MIN_TARGET) / 2.))
            .px(px(MARGIN_X))
            .cursor_pointer()
            .on_key_down(on_key_down)
            .child(grip)
            .child(track)
            .children(focus_marks(
                focus.is_focused(window),
                theme,
                theme.radii.sm,
                px(0.),
                window,
                cx,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn a_press_maps_across_the_track_and_clamps_past_its_ends() {
        let track = Bounds::new(point(px(100.), px(0.)), size(px(200.), px(4.)));
        assert_eq!(value_at(px(100.), track), 0.);
        assert_eq!(value_at(px(150.), track), 0.25);
        assert_eq!(value_at(px(300.), track), 1.);
        assert_eq!(value_at(px(50.), track), 0.);
        assert_eq!(value_at(px(400.), track), 1.);
    }

    #[test]
    fn arrow_keys_step_and_stop_at_the_ends() {
        assert_eq!(key_target("right", 0.5), Some(0.55));
        assert_eq!(key_target("left", 0.02), Some(0.));
        assert_eq!(key_target("up", 0.98), Some(1.));
        assert_eq!(key_target("home", 0.5), Some(0.));
        assert_eq!(key_target("end", 0.5), Some(1.));
        assert_eq!(key_target("a", 0.5), None);
    }
}
