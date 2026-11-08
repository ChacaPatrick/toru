//! Spike for `docs/spikes.md` section 1.5: the kinds of controls Toru needs, each with a role and a
//! label, so that what Windows UI Automation sees can be queried from another process.
//!
//! Every state change and keyboard focus change is printed to stdout. `TORU_SPIKE_AUTOCLOSE_MS`
//! quits after that long.

use std::{
    env,
    fmt::Display,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gpui::{
    App, Bounds, Context, Div, Entity, FocusHandle, Role, SharedString, Stateful, Subscription,
    TitlebarOptions, Toggled, Window, WindowBounds, WindowOptions, accesskit::Live, div,
    prelude::*, px, size, text,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_platform::application;

/// Prints with Unix-time milliseconds, to compare against what another process receives.
fn log(line: impl Display) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    println!("{} {line}", now.as_millis());
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

/// A focusable tab stop with a role and a label. The visible text is a plain string, which GPUI
/// doesn't expose, so assistive technology gets the label once and not a duplicate text child.
fn control(
    id: &'static str,
    role: Role,
    label: &'static str,
    focus: &FocusHandle,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(role)
        .aria_label(label)
        .track_focus(focus)
        .p_2()
        .border_1()
        .focus(|style| style.border_4())
        .child(label)
}

struct Controls {
    /// One per control below, in tab order, with the name printed when it gains focus.
    focus: [(&'static str, FocusHandle); 4],
    last_focused: Option<&'static str>,
    copies: usize,
    keep_unsaved: bool,
    selected: bool,
    toast: Option<SharedString>,
    name: Entity<InputState>,
    _name_events: Subscription,
}

impl Controls {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value("Capture 1"));
        let name_events = cx.subscribe(&name, |_, name, event: &InputEvent, cx| match event {
            InputEvent::Change => log(format!("name changed to {:?}", name.read(cx).value())),
            InputEvent::Focus => log("focus: name field"),
            InputEvent::Blur => log("blur: name field"),
            _ => {}
        });
        let focus = ["copy", "dismiss", "keep unsaved", "select"]
            .map(|name| (name, cx.focus_handle().tab_stop(true)));
        Self {
            focus,
            last_focused: None,
            copies: 0,
            keep_unsaved: false,
            selected: false,
            toast: None,
            name,
            _name_events: name_events,
        }
    }

    fn print_focus_change(&mut self, window: &Window) {
        let focused = self
            .focus
            .iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(name, _)| *name);
        if focused != self.last_focused {
            self.last_focused = focused;
            if let Some(name) = focused {
                log(format!("focus: {name}"));
            }
        }
    }
}

impl Render for Controls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.print_focus_change(window);
        let [copy, dismiss, keep_unsaved, select] = self.focus.clone().map(|(_, handle)| handle);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .child(text!("This session"))
            .child(
                control("copy", Role::Button, "Copy all", &copy).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copies += 1;
                        log(format!("copy pressed, {} so far", this.copies));
                        this.toast = Some(format!("Copied {} times", this.copies).into());
                        cx.notify();
                    },
                )),
            )
            .child(
                control("dismiss", Role::Button, "Dismiss toast", &dismiss).on_click(cx.listener(
                    |this, _, _, cx| {
                        log("toast dismissed");
                        this.toast = None;
                        cx.notify();
                    },
                )),
            )
            .child(
                control("keep-unsaved", Role::Switch, "Keep unsaved", &keep_unsaved)
                    .aria_toggled(toggled(self.keep_unsaved))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.keep_unsaved = !this.keep_unsaved;
                        log(format!("keep unsaved is now {}", this.keep_unsaved));
                        cx.notify();
                    })),
            )
            .child(Input::new(&self.name).aria_label("Capture name"))
            .child(
                control("select", Role::CheckBox, "Select capture 1", &select)
                    .aria_toggled(toggled(self.selected))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected = !this.selected;
                        log(format!("selected is now {}", this.selected));
                        cx.notify();
                    })),
            )
            .when_some(self.toast.clone(), |this, message| {
                // GPUI has no setter for the live property, but this hook gets the AccessKit node.
                this.child(
                    div()
                        .id("toast")
                        .role(Role::Status)
                        .aria_label(message.clone())
                        .a11y_synthetic_children(|builder| {
                            builder.parent_node().set_live(Live::Polite);
                        })
                        .child(message),
                )
            })
    }
}

fn main() {
    let autoclose = env::var("TORU_SPIKE_AUTOCLOSE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis);

    application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(420.), px(420.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Toru accessibility spike".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Controls::new(window, cx))
        })
        .expect("failed to open the window");

        if let Some(delay) = autoclose {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(delay).await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}
