//! The global shortcuts in the window (DESIGN.md §3): keeping the registry in step with the
//! settings and the displays, acting on a press, and the Edit shortcuts dialog. The dialog lists
//! each action, and each connected display, with its shortcut. Pressing a shortcut button
//! records: the next key combination becomes the shortcut, Esc stops without a change, and Clear
//! takes a shortcut away. The registered shortcuts stay registered while recording, so a press
//! of one arrives as a hotkey and is recorded, or refused as another row's, rather than acted on;
//! PrtScn, which Windows would otherwise act on itself, is held in every combination for the
//! duration. Recording ends when the window loses activation or hides, so a press in another
//! app never rebinds a row. Every change is saved and registered at once, and what is wrong
//! with a shortcut, from a bad stored string to a combination another app holds, is said in
//! text under its row.

use std::collections::HashMap;

use gpui::{Context, FocusHandle, KeyDownEvent, div, prelude::*, px};
use toru_core::capture::CaptureMode;

use super::{MainWindow, act, settings::shortcut_text};
use crate::platform;
use crate::shortcuts::{self, Binding, Planned, Problem, Shortcut, problem_text, refusal_text};
use crate::theme::{ActiveTheme, ThemedText};
use crate::ui::{Button, Dialog};

const WIDTH: f32 = 520.;
const ROW_GAP: f32 = 10.;
/// Between a row and the problem text under it.
const NOTE_GAP: f32 = 4.;
/// `.hint`'s `line-height: 1.45`.
const HINT_LEADING: f32 = 1.45;
const BODY: &str = "Press a shortcut, then the new keys; Esc stops. Shortcuts work while Toru is \
                    hidden in the tray, and take those keys from every other app.";
const NOT_WORKING: &str = "Some shortcuts aren't working. Edit shortcuts… says why.";
const DISPLAY_GONE: &str = "That display isn't connected";
const NOT_SET: &str = "Not set";
const RECORDING: &str = "Press keys…";

/// A row's shortcut button and Clear button.
type RowFocus = (FocusHandle, FocusHandle);

/// The Edit shortcuts dialog while it is open.
pub(super) struct ShortcutsDialog {
    /// The row recording a new shortcut, if one is.
    recording: Option<Binding>,
    /// Why the last key press was not taken, shown under its row until the next press or until
    /// recording stops.
    refused: Option<(Binding, String)>,
    /// Each row's buttons, by what the row binds, so a row keeps its focus whatever its place.
    rows: HashMap<Binding, RowFocus>,
    /// The Done button's. Its own, so the quit dialog's first action stays separate should
    /// quitting be asked about while this dialog is open.
    pub(super) done: FocusHandle,
}

impl MainWindow {
    /// Plans the shortcuts from the settings and the displays connected now, and registers them
    /// if that changed anything: a shortcut is only let go and taken again when it must be, so
    /// another app can't slip in between. Called at start, on each change to a shortcut, and
    /// whenever the displays change, the window being hidden in the tray or not.
    pub(super) fn refresh_hotkeys(&mut self, cx: &mut Context<Self>) {
        let (displays, _) = self.displays(cx);
        let entries = shortcuts::entries(&self.settings, |display| {
            displays.iter().any(|d| d.key == display)
        });
        let mut planned = shortcuts::plan(entries);
        if same_plan(&planned, &self.shortcuts) {
            // Nothing is registered anew, so what registration found last time still holds.
            for (row, before) in planned.iter_mut().zip(&self.shortcuts) {
                if row.problem.is_none() {
                    row.problem = before.problem.clone();
                }
            }
        } else {
            self.hotkeys.apply(&mut planned);
        }
        self.shortcuts = planned;
        cx.notify();
    }

    /// Said once, as the app starts, so a shortcut that stopped working since last time isn't
    /// found out by pressing it.
    pub(super) fn announce_shortcut_problems(&mut self, cx: &mut Context<Self>) {
        let broken = self
            .shortcuts
            .iter()
            .any(|row| !matches!(row.problem, None | Some(Problem::Disconnected)));
        if broken {
            self.report_failure(NOT_WORKING, cx);
        }
    }

    /// A global shortcut was pressed: the capture it is bound to, with the saved settings, as
    /// if its toolbar button had been pressed. The window may be hidden in the tray. While a
    /// shortcut is being recorded the keys are what is being recorded instead.
    pub fn hotkey_pressed(&mut self, id: u32, cx: &mut Context<Self>) {
        if self.recording().is_some() {
            if let Some(shortcut) = self.hotkeys.pressed(id) {
                self.record(shortcut, cx);
            }
            return;
        }
        let Some(binding) = self.hotkeys.binding(id).cloned() else {
            return;
        };
        match binding {
            Binding::Region => self.capture(CaptureMode::Region, cx),
            Binding::Window => self.capture(CaptureMode::Window, cx),
            Binding::FullScreen => self.capture(CaptureMode::FullScreen, cx),
            Binding::Delayed => self.start_countdown(cx),
            Binding::Display(key) => {
                let (displays, _) = self.displays(cx);
                match displays.iter().find(|display| display.key == key) {
                    Some(display) => {
                        let id = display.id;
                        self.cancel_countdown(cx);
                        self.capture_full_screen(Some(id), cx);
                    }
                    None => self.report_failure(DISPLAY_GONE, cx),
                }
            }
        }
    }

    /// The row for `binding`, as last planned.
    pub(super) fn planned(&self, binding: &Binding) -> Option<&Planned> {
        self.shortcuts.iter().find(|row| row.binding == *binding)
    }

    /// The toolbar's hint: the Region and Window shortcuts that are working, as the mockups
    /// show them.
    pub(super) fn toolbar_hint(&self) -> String {
        [Binding::Region, Binding::Window]
            .iter()
            .filter_map(|binding| self.planned(binding))
            .filter(|row| row.problem.is_none())
            .filter_map(|row| row.text.as_deref())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// What the dialog calls a row.
    fn binding_name(&self, binding: &Binding, cx: &Context<Self>) -> String {
        match binding {
            Binding::Region => "Region".into(),
            Binding::Window => "Window".into(),
            Binding::FullScreen => "Full screen".into(),
            Binding::Delayed => "Delayed".into(),
            Binding::Display(key) => {
                let (displays, _) = self.displays(cx);
                displays
                    .iter()
                    .find(|display| display.key == *key)
                    .map_or_else(
                        || "Display not connected".to_string(),
                        |display| display.label.to_string(),
                    )
            }
        }
    }

    /// The dialog's rows (`dialog_rows`), with the displays as they are connected now.
    fn rows(&self, cx: &Context<Self>) -> Vec<Planned> {
        let (displays, _) = self.displays(cx);
        let connected: Vec<&str> = displays
            .iter()
            .map(|display| display.key.as_str())
            .collect();
        dialog_rows(&self.shortcuts, &connected)
    }

    pub(super) fn open_shortcuts_dialog(&mut self, cx: &mut Context<Self>) {
        let first = self.rows(cx).into_iter().next().map(|row| row.binding);
        let mut dialog = ShortcutsDialog {
            recording: None,
            refused: None,
            rows: HashMap::new(),
            done: cx.focus_handle().tab_stop(true),
        };
        let first = first.map(|binding| dialog.row_focus(&binding, cx).0.clone());
        self.shortcuts_dialog = Some(dialog);
        if let Some(first) = first {
            self.focus_later(first, cx);
        }
        cx.notify();
    }

    /// Done, or Esc with nothing being recorded. Focus goes back to the Edit shortcuts… button's
    /// column through `on_focus_lost`.
    pub(super) fn close_shortcuts_dialog(&mut self, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.shortcuts_dialog = None;
        cx.notify();
    }

    /// Focuses `focus` once the current update is over, since the window can't be reached from
    /// inside one of its own handlers.
    fn focus_later(&self, focus: FocusHandle, cx: &mut Context<Self>) {
        let window = self.window;
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| focus.focus(window, cx))
                .ok();
        });
    }

    /// The row recording a new shortcut, if one is.
    fn recording(&self) -> Option<Binding> {
        self.shortcuts_dialog
            .as_ref()
            .and_then(|dialog| dialog.recording.clone())
    }

    /// Windows gives no key-down for PrtScn, and acts on it itself, so while recording Toru
    /// holds every combination of it that no row has (`Hotkeys::listen`) and the press arrives
    /// through `hotkey_pressed`. One another app holds goes on reaching that app.
    fn start_recording(&mut self, binding: Binding, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.shortcuts_dialog else {
            return;
        };
        dialog.refused = None;
        dialog.recording = Some(binding);
        self.hotkeys.listen(Shortcut::print_screen_combinations());
        cx.notify();
    }

    /// Ends recording, taken or not, lets PrtScn go, and takes the last refusal's note away.
    /// Also called as the window loses activation or hides, so a press in another app can't
    /// rebind a row.
    pub(super) fn stop_recording(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.shortcuts_dialog else {
            return;
        };
        dialog.refused = None;
        if dialog.recording.take().is_some() {
            self.hotkeys.stop_listening();
        }
        cx.notify();
    }

    /// The keys pressed while recording. Keys another row already has are refused, with a note,
    /// so the user never ends up with two rows wanting the same press. Taken keys are saved and
    /// registered at once.
    fn record(&mut self, shortcut: Shortcut, cx: &mut Context<Self>) {
        let Some(binding) = self.recording() else {
            return;
        };
        let other = self
            .shortcuts
            .iter()
            .find(|row| row.binding != binding && row.shortcut == Some(shortcut))
            .map(|row| row.binding.clone());
        if let Some(other) = other {
            let message = problem_text(&Problem::Duplicate(other), |binding| {
                self.binding_name(binding, cx)
            });
            self.refuse(message, cx);
            return;
        }
        self.stop_recording(cx);
        self.change_settings(cx, |settings| {
            shortcuts::set(settings, &binding, Some(shortcut))
        });
        self.refresh_hotkeys(cx);
    }

    fn refuse(&mut self, message: String, cx: &mut Context<Self>) {
        if let Some(dialog) = &mut self.shortcuts_dialog
            && let Some(binding) = dialog.recording.clone()
        {
            dialog.refused = Some((binding, message));
        }
        cx.notify();
    }

    /// Clear goes with the shortcut, so focus moves to the row's shortcut button rather than
    /// out of the dialog.
    fn clear_shortcut(&mut self, binding: Binding, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.change_settings(cx, |settings| shortcuts::set(settings, &binding, None));
        self.refresh_hotkeys(cx);
        let keys = self
            .shortcuts_dialog
            .as_ref()
            .and_then(|dialog| dialog.rows.get(&binding))
            .map(|(keys, _)| keys.clone());
        if let Some(keys) = keys {
            self.focus_later(keys, cx);
        }
    }

    /// A key pressed in the dialog while recording: Esc on its own stops, anything else is the
    /// new shortcut or the reason it can't be. The platform is asked which key is down, since it
    /// numbers keys the same on every layout where GPUI names them by what they type
    /// (`shortcuts::keys`). Returns whether the key was taken.
    fn recording_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if self.recording().is_none() {
            return false;
        }
        let keystroke = &event.keystroke;
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            self.stop_recording(cx);
            return true;
        }
        let shortcut = match platform::pressed_key(shortcuts::keys()) {
            Some(key) => Shortcut::from_held(key, platform::held_modifiers()),
            None => Shortcut::from_keystroke(keystroke),
        };
        match shortcut {
            Ok(shortcut) => self.record(shortcut, cx),
            Err(error) => self.refuse(refusal_text(&error), cx),
        }
        true
    }

    /// The dialog, for the frame. Rows come and go with the displays, so each row's focus
    /// handles are looked up, or made, as it is drawn.
    pub(super) fn shortcuts_dialog(&mut self, cx: &mut Context<Self>) -> Option<Dialog> {
        self.shortcuts_dialog.as_ref()?;
        let rows = self.rows(cx);
        let dialog = self.shortcuts_dialog.as_mut()?;
        let focus: Vec<RowFocus> = rows
            .iter()
            .map(|row| dialog.row_focus(&row.binding, cx).clone())
            .collect();
        dialog
            .rows
            .retain(|binding, _| rows.iter().any(|row| row.binding == *binding));
        let recording = dialog.recording.clone();
        let refused = dialog.refused.clone();
        let done = dialog.done.clone();

        let theme = cx.theme();
        let c = &theme.colors;
        let hint = theme.type_scale.hint;
        let mut stops = Vec::new();
        let mut list = div().flex().flex_col().gap(px(ROW_GAP));
        for (index, (row, (keys_focus, clear_focus))) in rows.iter().zip(&focus).enumerate() {
            let binding = row.binding.clone();
            let is_recording = recording.as_ref() == Some(&binding);
            let keys = if is_recording {
                RECORDING.to_string()
            } else {
                row.text.as_deref().map_or(NOT_SET.into(), shortcut_text)
            };
            let note = match &refused {
                Some((refused_for, message)) if *refused_for == binding => Some(message.clone()),
                _ => row
                    .problem
                    .as_ref()
                    .map(|problem| problem_text(problem, |other| self.binding_name(other, cx))),
            };
            let name = self.binding_name(&binding, cx);
            stops.push(keys_focus.clone());
            let mut line = div()
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .type_style(theme, theme.type_scale.setting)
                        .child(name.clone()),
                )
                .child(
                    Button::new(("shortcut-keys", index), keys)
                        .named(keys_button_name(
                            &name,
                            row.text.as_deref().map(shortcut_text).as_deref(),
                            is_recording,
                        ))
                        .when_some(note.clone(), Button::description)
                        .small()
                        .when(is_recording, Button::primary)
                        .track_focus(keys_focus)
                        .on_click(act(cx, {
                            let binding = binding.clone();
                            move |this, cx| this.start_recording(binding.clone(), cx)
                        })),
                );
            if row.text.is_some() {
                stops.push(clear_focus.clone());
                line = line.child(
                    Button::new(("shortcut-clear", index), "Clear")
                        .named(format!("Clear {} shortcut", short_binding_name(&name)))
                        .small()
                        .track_focus(clear_focus)
                        .on_click(act(cx, {
                            let binding = binding.clone();
                            move |this, cx| this.clear_shortcut(binding.clone(), cx)
                        })),
                );
            }
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(NOTE_GAP))
                    .child(line)
                    .children(note.map(|note| {
                        // In the text colour, so the words carry it (DESIGN.md §7).
                        div()
                            .type_style(theme, hint)
                            .line_height(hint.size * HINT_LEADING)
                            .text_color(c.text)
                            .child(note)
                    })),
            );
        }

        let view = cx.entity();
        Some(
            Dialog::new(
                "shortcuts-dialog",
                "Edit shortcuts",
                BODY,
                &done,
                act(cx, |this, cx| this.close_shortcuts_dialog(cx)),
            )
            .width(px(WIDTH))
            .content(list, stops)
            .on_key(move |event, _, cx| view.update(cx, |this, cx| this.recording_key(event, cx)))
            .primary_action("Done", act(cx, |this, cx| this.close_shortcuts_dialog(cx))),
        )
    }
}

impl ShortcutsDialog {
    /// The row's focus handles, made the first time the row is asked about.
    fn row_focus(&mut self, binding: &Binding, cx: &mut Context<MainWindow>) -> &RowFocus {
        self.rows.entry(binding.clone()).or_insert_with(|| {
            (
                cx.focus_handle().tab_stop(true),
                cx.focus_handle().tab_stop(true),
            )
        })
    }
}

/// The dialog's rows, in an order that holds as shortcuts are set and cleared: the four
/// actions, then each connected display in the order the platform lists them, with its shortcut
/// or none, then any display that isn't connected but keeps a shortcut.
fn dialog_rows(planned: &[Planned], connected: &[&str]) -> Vec<Planned> {
    fn display_key(row: &Planned) -> Option<&str> {
        match &row.binding {
            Binding::Display(key) => Some(key.as_str()),
            _ => None,
        }
    }
    let mut rows: Vec<Planned> = planned
        .iter()
        .filter(|row| display_key(row).is_none())
        .cloned()
        .collect();
    for key in connected {
        let row = planned.iter().find(|row| display_key(row) == Some(key));
        rows.push(row.cloned().unwrap_or_else(|| Planned {
            binding: Binding::Display(key.to_string()),
            text: None,
            shortcut: None,
            problem: None,
        }));
    }
    rows.extend(
        planned
            .iter()
            .filter(|row| display_key(row).is_some_and(|key| !connected.contains(&key)))
            .cloned(),
    );
    rows
}

/// A row's name without the display's size: "Display 2 · 2560×1440" is just "Display 2" here.
fn short_binding_name(name: &str) -> &str {
    name.split(" · ").next().unwrap_or(name)
}

/// What assistive technology calls a row's shortcut button: what it sets and what it is set to,
/// or that it is waiting for the new keys.
fn keys_button_name(binding: &str, keys: Option<&str>, recording: bool) -> String {
    let state = match (recording, keys) {
        (true, _) => "recording, press the new keys",
        (false, Some(keys)) => keys,
        (false, None) => "not set",
    };
    format!("{} shortcut, {state}", short_binding_name(binding))
}

/// Whether two plans want the same registrations: the same rows with the same keys, and the
/// same ones held back before registration. Registration results aren't compared, since they
/// are what applying finds out.
fn same_plan(a: &[Planned], b: &[Planned]) -> bool {
    let key = |row: &Planned| {
        (
            row.binding.clone(),
            row.shortcut,
            matches!(
                row.problem,
                Some(Problem::Unreadable(_) | Problem::Duplicate(_) | Problem::Disconnected)
            ),
        )
    };
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| key(a) == key(b))
}

/// The settings column's note under the Shortcuts group, when a shortcut isn't working.
pub(super) fn column_note(rows: &[Planned]) -> Option<&'static str> {
    rows.iter()
        .any(|row| !matches!(row.problem, None | Some(Problem::Disconnected)))
        .then_some(NOT_WORKING)
}

#[cfg(test)]
mod tests {
    use crate::shortcuts::ShortcutError;

    use super::*;

    fn row(binding: Binding, text: &str, problem: Option<Problem>) -> Planned {
        let shortcut = Shortcut::parse(text).ok();
        Planned {
            binding,
            text: Some(text.to_string()),
            shortcut,
            problem,
        }
    }

    #[test]
    fn shortcut_buttons_are_named_by_what_they_set() {
        assert_eq!(
            keys_button_name("Region", Some("PrtScn"), false),
            "Region shortcut, PrtScn"
        );
        assert_eq!(
            keys_button_name("Display 2 · 2560×1440", None, false),
            "Display 2 shortcut, not set"
        );
        assert_eq!(
            keys_button_name("Full screen", Some("F9"), true),
            "Full screen shortcut, recording, press the new keys"
        );
    }

    #[test]
    fn a_plan_is_the_same_whatever_registration_found() {
        let before = vec![row(Binding::Region, "PrtScn", None)];
        let taken = vec![row(
            Binding::Region,
            "PrtScn",
            Some(Problem::TakenByAnotherApp),
        )];
        assert!(same_plan(&before, &taken));
    }

    #[test]
    fn a_plan_changes_with_the_keys_the_rows_or_what_is_held_back() {
        let base = vec![row(Binding::Region, "PrtScn", None)];
        assert!(!same_plan(
            &base,
            &[row(Binding::Region, "Alt+PrtScn", None)]
        ));
        assert!(!same_plan(&base, &[row(Binding::Window, "PrtScn", None)]));
        assert!(!same_plan(
            &base,
            &[row(
                Binding::Region,
                "PrtScn",
                Some(Problem::Duplicate(Binding::Window))
            )]
        ));
        assert!(!same_plan(&base, &[]));
    }

    #[test]
    fn the_rows_keep_the_displays_order_whether_or_not_they_have_shortcuts() {
        let planned = vec![
            row(Binding::Region, "PrtScn", None),
            row(Binding::Delayed, "Ctrl+Alt+D", None),
            row(
                Binding::Display("away".into()),
                "Ctrl+Alt+9",
                Some(Problem::Disconnected),
            ),
            row(Binding::Display("second".into()), "Ctrl+Alt+2", None),
        ];
        let rows = dialog_rows(&planned, &["first", "second"]);
        let bindings: Vec<&Binding> = rows.iter().map(|row| &row.binding).collect();
        assert_eq!(
            bindings,
            [
                &Binding::Region,
                &Binding::Delayed,
                &Binding::Display("first".into()),
                &Binding::Display("second".into()),
                &Binding::Display("away".into()),
            ]
        );
        assert_eq!(rows[2].text, None, "no shortcut yet, so one can be added");
        assert_eq!(rows[3].text.as_deref(), Some("Ctrl+Alt+2"));
        assert_eq!(rows[4].problem, Some(Problem::Disconnected));

        // Setting the first display's shortcut, which lists it after the second's in the
        // settings, moves no row.
        let mut with_first = planned.clone();
        with_first.push(row(Binding::Display("first".into()), "Ctrl+Alt+1", None));
        let after = dialog_rows(&with_first, &["first", "second"]);
        let after: Vec<&Binding> = after.iter().map(|row| &row.binding).collect();
        assert_eq!(after, bindings);
    }

    #[test]
    fn the_column_notes_broken_shortcuts_but_not_an_absent_display() {
        let fine = [row(Binding::Region, "PrtScn", None)];
        assert_eq!(column_note(&fine), None);
        let away = [row(
            Binding::Display("away".into()),
            "Ctrl+Alt+2",
            Some(Problem::Disconnected),
        )];
        assert_eq!(column_note(&away), None);
        for problem in [
            Problem::TakenByAnotherApp,
            Problem::Duplicate(Binding::Region),
            Problem::Unreadable(ShortcutError::NotOneKey),
            Problem::Failed("no".into()),
        ] {
            let broken = [row(Binding::Window, "Ctrl+Shift+W", Some(problem))];
            assert_eq!(column_note(&broken), Some(NOT_WORKING));
        }
    }
}
