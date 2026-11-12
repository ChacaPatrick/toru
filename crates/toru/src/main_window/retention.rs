//! The Delete unsaved after setting (CLAUDE.md): how long unsaved captures stay in history, as a
//! radio group of the preset day counts, Custom with a count of its own under it, and Never.
//! Typing in the count chooses Custom and is checked as it goes; a count of at least 1 reaches the
//! setting on Enter or when focus leaves the group, and anything else is named in the hint's place
//! and never stored. The field's own blur doesn't commit, since clicking another row blurs it, and
//! a dialog raised by that would cover the row and swallow the click. Shortening the period
//! deletes captures for good, so a change that would delete anything right now asks first, saying
//! how many, and Cancel leaves the setting as it was. The captures are counted on the capture
//! queue, after any Save already asked for, and Delete deletes exactly the captures counted, by
//! id, so the dialog is right however long it stays open.
//!
//! The cleanup itself, `Store::delete_unsaved_older_than`, runs on the capture queue off the main
//! thread at launch and at each midnight reset, never with Never set. The gallery drops the cards
//! that went. A failure is reported and the app carries on.

use gpui::{Context, Div, Entity, FocusHandle, Window, div, prelude::*, px};
use gpui_kit::component::input::{InputEvent, InputState};
use toru_core::{
    settings::{RETENTION_PRESET_DAYS, Retention},
    store::{CaptureId, Store},
};

use super::{
    MainWindow, act,
    gallery::plural,
    settings::{PATTERN_HEIGHT, act_with, group, hint},
};
use crate::theme::ActiveTheme;
use crate::ui::{Dialog, RadioGroup, TextField};

const CUSTOM_ROW: usize = RETENTION_PRESET_DAYS.len();
const NEVER_ROW: usize = CUSTOM_ROW + 1;
const DAYS_FIELD_WIDTH: f32 = 72.;
/// Between the count field and "days".
const DAYS_GAP: f32 = 8.;
const HINT: &str = "Saved files are never deleted.";
const CLEANUP_FAILED: &str = "Couldn't delete the older unsaved captures";
const COUNT_FAILED: &str = "Couldn't check what that change would delete";

/// What a row of the group stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Preset(u32),
    Custom,
    Never,
}

pub fn choice_at(row: usize) -> Choice {
    match RETENTION_PRESET_DAYS.get(row) {
        Some(&days) => Choice::Preset(days),
        None if row == CUSTOM_ROW => Choice::Custom,
        None => Choice::Never,
    }
}

/// Which row shows as chosen: Custom while the user has picked it, whatever count it holds, so a
/// typed preset count or an invalid one doesn't jump the choice away; else the setting's own row.
pub fn selected_row(retention: Retention, custom_chosen: bool) -> usize {
    match retention {
        _ if custom_chosen => CUSTOM_ROW,
        Retention::Never => NEVER_ROW,
        Retention::Days(days) => RETENTION_PRESET_DAYS
            .iter()
            .position(|&preset| preset == days)
            .unwrap_or(CUSTOM_ROW),
    }
}

/// The count as typed: a whole number of days, at least 1, or what is wrong with it.
pub fn parse_days(text: &str) -> Result<u32, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Type a number of days.");
    }
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return Err("Type a whole number of days.");
    }
    match text.parse::<u32>() {
        Ok(0) => Err("At least 1 day."),
        Ok(days) => Ok(days),
        Err(_) => Err("Too many days."),
    }
}

/// What the count field shows for a setting: its days, or nothing for Never.
pub fn field_text(retention: Retention) -> String {
    retention
        .days()
        .map(|days| days.to_string())
        .unwrap_or_default()
}

/// The dialog's explanation, for `count` captures older than `days`.
pub fn confirm_prompt(count: usize, days: u32) -> String {
    let (they, are) = if count == 1 {
        ("it", "is")
    } else {
        ("them", "are")
    };
    format!(
        "{} {are} older than {} and would be deleted now. {HINT} Keep {they}, or delete {they}?",
        plural(count, "unsaved capture"),
        plural(days as usize, "day"),
    )
}

/// The control's state, beside the setting itself.
pub struct RetentionControl {
    /// The Custom count as typed.
    days: Entity<InputState>,
    /// Why the typed count isn't a setting, shown in the hint's place.
    error: Option<&'static str>,
    /// The Custom row is chosen, whatever count it holds.
    custom: bool,
    /// Tracked by the group as a whole, so the typed count commits when focus leaves it.
    group: FocusHandle,
    /// Counts up with each change asked for, so the count of a change since superseded, such as
    /// a count committed by Enter just before another row is clicked, is let go.
    proposal: usize,
    /// The change waiting in the dialog.
    prompt: Option<Prompt>,
}

struct Prompt {
    retention: Retention,
    /// The unsaved captures the dialog says would go, which are what Delete deletes.
    captures: Vec<CaptureId>,
}

impl RetentionControl {
    pub fn new(retention: Retention, window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let days = cx.new(|cx| InputState::new(window, cx).default_value(field_text(retention)));
        cx.subscribe(&days, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Change => this.typed_days(cx),
            InputEvent::PressEnter { .. } => this.commit_days(cx),
            InputEvent::Focus | InputEvent::Blur => {}
        })
        .detach();
        let group = cx.focus_handle();
        cx.on_focus_out(&group, window, |this, _, _, cx| this.commit_days(cx))
            .detach();
        RetentionControl {
            days,
            error: None,
            custom: retention.is_custom(),
            group,
            proposal: 0,
            prompt: None,
        }
    }

    /// Whether the dialog asking to delete older captures is open.
    pub fn asking(&self) -> bool {
        self.prompt.is_some()
    }
}

impl MainWindow {
    /// A row of the group: a preset or Never takes effect, asking first if need be; Custom takes
    /// the count in the field, if it is one.
    fn choose_retention(&mut self, row: usize, cx: &mut Context<Self>) {
        match choice_at(row) {
            Choice::Preset(days) => self.choose_fixed_retention(Retention::Days(days), cx),
            Choice::Never => self.choose_fixed_retention(Retention::Never, cx),
            Choice::Custom => {
                self.retention.custom = true;
                self.commit_days(cx);
            }
        }
        cx.notify();
    }

    fn choose_fixed_retention(&mut self, retention: Retention, cx: &mut Context<Self>) {
        self.retention.custom = false;
        self.retention.error = None;
        self.propose_retention(retention, cx);
    }

    /// Typing in the field chooses Custom and checks the count; it takes effect on Enter or when
    /// focus leaves the group, so a dialog can't come up halfway through a number.
    fn typed_days(&mut self, cx: &mut Context<Self>) {
        self.retention.custom = true;
        self.retention.error = parse_days(&self.retention.days.read(cx).value()).err();
        cx.notify();
    }

    /// Nothing to commit while the dialog is up: focus moving to it leaves the group too.
    fn commit_days(&mut self, cx: &mut Context<Self>) {
        if !self.retention.custom || self.retention.prompt.is_some() {
            return;
        }
        match parse_days(&self.retention.days.read(cx).value()) {
            Ok(days) => {
                self.retention.error = None;
                self.propose_retention(Retention::Days(days), cx);
            }
            Err(message) => self.retention.error = Some(message),
        }
        cx.notify();
    }

    /// Puts the change to the store for what it would delete now, on the capture queue so a Save
    /// asked for already is finished first: nothing, and it takes effect; something, and the
    /// dialog asks first.
    fn propose_retention(&mut self, wanted: Retention, cx: &mut Context<Self>) {
        self.retention.proposal += 1;
        if wanted == self.settings.retention {
            self.retention.prompt = None;
            self.show_retention_field(cx);
            return;
        }
        let Some(days) = wanted.days() else {
            self.apply_retention(wanted, cx);
            return;
        };
        let proposal = self.retention.proposal;
        let (store, today) = (self.store.clone(), self.day);
        self.enqueue(cx, async move |this, cx| {
            let counted = cx
                .background_spawn(async move { store.unsaved_older_than(days, today) })
                .await;
            this.update(cx, |this, cx| {
                if this.retention.proposal != proposal {
                    return;
                }
                match counted {
                    Ok(captures) if captures.is_empty() => this.apply_retention(wanted, cx),
                    Ok(captures) => this.ask_before_deleting(wanted, captures, cx),
                    Err(_) => {
                        this.report_failure(COUNT_FAILED, cx);
                        this.show_stored_retention(cx);
                    }
                }
            })
            .ok();
        });
    }

    fn ask_before_deleting(
        &mut self,
        wanted: Retention,
        captures: Vec<CaptureId>,
        cx: &mut Context<Self>,
    ) {
        self.retention.prompt = Some(Prompt {
            retention: wanted,
            captures,
        });
        self.focus_dialog(cx);
        cx.notify();
    }

    /// A change that deletes nothing right now. The midnight reset's cleanup takes the new
    /// period from there.
    fn apply_retention(&mut self, retention: Retention, cx: &mut Context<Self>) {
        self.retention.prompt = None;
        self.change_settings(cx, |settings| settings.retention = retention);
        self.show_retention_field(cx);
    }

    /// The dialog's Delete: the change takes effect and exactly the captures it named go, by id,
    /// however long it stayed open. One saved in the meantime is left alone.
    fn delete_counted(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.retention.prompt.take() else {
            return;
        };
        self.change_settings(cx, |settings| settings.retention = prompt.retention);
        self.show_retention_field(cx);
        let captures = prompt.captures;
        self.delete_from_history(cx, move |store| store.delete_unsaved(&captures));
    }

    /// Cancel, or a failed count: the control goes back to showing the stored setting.
    fn show_stored_retention(&mut self, cx: &mut Context<Self>) {
        self.retention.prompt = None;
        self.retention.custom = self.settings.retention.is_custom();
        self.retention.error = None;
        self.show_retention_field(cx);
        cx.notify();
    }

    /// Puts the stored setting's count in the field, so Custom starts from it. Deferred, since
    /// the field's window can't be reached from inside one of its own handlers.
    fn show_retention_field(&mut self, cx: &mut Context<Self>) {
        let text = field_text(self.settings.retention);
        let (window, field) = (self.window, self.retention.days.clone());
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    field.update(cx, |field, cx| {
                        if field.value() != text {
                            field.set_value(text, window, cx);
                        }
                    })
                })
                .ok();
        });
    }

    /// Deletes the unsaved captures past the retention period. Nothing happens with Never.
    pub fn clean_up_history(&mut self, cx: &mut Context<Self>) {
        let Some(days) = self.settings.retention.days() else {
            return;
        };
        let today = self.day;
        self.delete_from_history(cx, move |store| {
            store.delete_unsaved_older_than(days, today)
        });
    }

    /// Runs `delete` on the store and takes the cards of what went out of the gallery. On the
    /// queue, so a capture still being stored or a Save asked for already is finished first.
    fn delete_from_history(
        &mut self,
        cx: &mut Context<Self>,
        delete: impl FnOnce(&Store) -> toru_core::store::Result<Vec<CaptureId>> + Send + 'static,
    ) {
        let store = self.store.clone();
        self.enqueue(cx, async move |this, cx| {
            let deleted = cx.background_spawn(async move { delete(&store) }).await;
            this.update(cx, |this, cx| match deleted {
                Ok(deleted) if deleted.is_empty() => {}
                Ok(deleted) => {
                    this.gallery
                        .update(cx, |gallery, cx| gallery.remove_captures(&deleted, cx));
                }
                Err(_) => this.report_failure(CLEANUP_FAILED, cx),
            })
            .ok();
        });
    }

    /// The dialog for a change that would delete captures now. Cancel is first, focused and
    /// primary, so Enter and Esc both leave everything as it was.
    pub(super) fn retention_dialog(&self, cx: &Context<Self>) -> Option<Dialog> {
        let prompt = self.retention.prompt.as_ref()?;
        let count = prompt.captures.len();
        let body = confirm_prompt(count, prompt.retention.days()?);
        Some(
            Dialog::new(
                "retention-dialog",
                "Delete older captures?",
                body,
                &self.dialog_focus,
                act(cx, |this, cx| this.show_stored_retention(cx)),
            )
            .primary_action("Cancel", act(cx, |this, cx| this.show_stored_retention(cx)))
            .action(
                format!("Delete {}", plural(count, "capture")),
                act(cx, |this, cx| this.delete_counted(cx)),
            ),
        )
    }
}

/// The group: the radio rows with the count field under Custom, then the hint, or, while the
/// typed count isn't a setting, what is wrong with it, in the text colour so the words carry it
/// (DESIGN.md §7).
pub(super) fn retention_group(this: &MainWindow, cx: &Context<MainWindow>) -> Div {
    let theme = cx.theme();
    let control = &this.retention;
    let options = RETENTION_PRESET_DAYS
        .iter()
        .map(|&days| format!("{days} days"))
        .chain(["Custom".to_string(), "Never".to_string()]);
    let (text, color) = match control.error {
        Some(message) => (message, theme.colors.text),
        None => (HINT, theme.colors.muted),
    };
    group(theme, "Delete unsaved after")
        .child(
            div().track_focus(&control.group).child(
                RadioGroup::new(
                    "retention",
                    "Delete unsaved after",
                    options,
                    selected_row(this.settings.retention, control.custom),
                )
                .description(text)
                .detail(CUSTOM_ROW, days_field(&control.days).into_any_element())
                .on_change(act_with(cx, |this, cx, row| this.choose_retention(row, cx))),
            ),
        )
        .child(hint(theme, text).text_color(color).mt(px(-4.)))
}

/// Under the Custom row: the count field and "days".
fn days_field(days: &Entity<InputState>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(DAYS_GAP))
        .child(
            TextField::new(days, "Custom number of days")
                .height(px(PATTERN_HEIGHT))
                .w(px(DAYS_FIELD_WIDTH)),
        )
        .child("days")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_presets_then_custom_then_never() {
        for (row, days) in RETENTION_PRESET_DAYS.into_iter().enumerate() {
            assert_eq!(choice_at(row), Choice::Preset(days));
        }
        assert_eq!(choice_at(CUSTOM_ROW), Choice::Custom);
        assert_eq!(choice_at(NEVER_ROW), Choice::Never);
    }

    #[test]
    fn the_setting_picks_its_row_unless_custom_is_chosen() {
        assert_eq!(selected_row(Retention::Days(30), false), 2);
        assert_eq!(selected_row(Retention::Days(45), false), CUSTOM_ROW);
        assert_eq!(selected_row(Retention::Never, false), NEVER_ROW);
        assert_eq!(selected_row(Retention::Days(30), true), CUSTOM_ROW);
        assert_eq!(selected_row(Retention::Never, true), CUSTOM_ROW);
    }

    #[test]
    fn a_count_is_a_whole_number_of_at_least_one_day() {
        assert_eq!(parse_days("1"), Ok(1));
        assert_eq!(parse_days(" 45 "), Ok(45));
        assert_eq!(parse_days("007"), Ok(7));
        assert_eq!(parse_days(""), Err("Type a number of days."));
        assert_eq!(parse_days("  "), Err("Type a number of days."));
        assert_eq!(parse_days("0"), Err("At least 1 day."));
        assert_eq!(parse_days("00"), Err("At least 1 day."));
        assert_eq!(parse_days("-1"), Err("Type a whole number of days."));
        assert_eq!(parse_days("1.5"), Err("Type a whole number of days."));
        assert_eq!(parse_days("ten"), Err("Type a whole number of days."));
        assert_eq!(parse_days("4294967296"), Err("Too many days."));
    }

    #[test]
    fn the_field_shows_the_days_and_nothing_for_never() {
        assert_eq!(field_text(Retention::Days(30)), "30");
        assert_eq!(field_text(Retention::Days(1)), "1");
        assert_eq!(field_text(Retention::Never), "");
    }

    #[test]
    fn the_prompt_counts_the_captures_and_the_days() {
        assert_eq!(
            confirm_prompt(12, 14),
            "12 unsaved captures are older than 14 days and would be deleted now. Saved files \
             are never deleted. Keep them, or delete them?"
        );
        assert_eq!(
            confirm_prompt(1, 1),
            "1 unsaved capture is older than 1 day and would be deleted now. Saved files are \
             never deleted. Keep it, or delete it?"
        );
    }
}
