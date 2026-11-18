use gpui::{AnyElement, Context, div, prelude::*, px};
use toru_core::time::LocalDate;

use super::{
    EARLIER_MARGIN, EARLIER_PADDING, Gallery, GroupKind, HEADER_HEIGHT, SEARCH_WIDTH, SELECT_HINT,
    SELECT_HINT_GAP,
};
use crate::main_window::{HINT_LEADING, act};
use crate::theme::{ActiveTheme, DayDate, ThemedText};
use crate::ui::{Button, Readable, TextField};

impl Gallery {
    /// The session header with the search field and Select at its right, and select mode's
    /// hint on a line beneath while the mode is on, or an earlier group's header with its rule
    /// above and Collapse at its right.
    pub(super) fn header(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let c = &theme.colors;
        let group = &self.groups[index];
        let strings = &theme.strings;
        let count = group.cards.len();
        let saved = group
            .cards
            .iter()
            .filter(|card| card.capture.is_saved())
            .count();
        let section = theme.type_scale.section;
        // Collapse and Expand name their group by this, without the look's "Earlier — ".
        let short_title = match group.kind {
            GroupKind::Session => strings.session.to_string(),
            GroupKind::Day(date) if date == self.today => strings.earlier_today.to_string(),
            GroupKind::Day(date) => day_date(date, strings.day_date),
        };
        let (title, hint, color) = match group.kind {
            GroupKind::Session => (
                short_title.clone(),
                format!(
                    "{} · {saved} saved · nothing saved to your folder until you save",
                    plural(count, "capture")
                ),
                c.text,
            ),
            GroupKind::Day(date) => (
                if date == self.today {
                    short_title.clone()
                } else {
                    format!("{}{short_title}", strings.earlier)
                },
                format!(
                    "{} · {} unsaved · shown because “Show previous” is on",
                    plural(count, "capture"),
                    count - saved
                ),
                theme.window_muted(),
            ),
        };
        let titles = div()
            .id(("group-heading", index))
            .read_as_heading(format!("{title} · {hint}"))
            .flex()
            .items_baseline()
            .gap(px(10.))
            .min_w_0()
            .child(
                div()
                    .flex_none()
                    .type_style(theme, section)
                    .text_color(color)
                    .whitespace_nowrap()
                    .child(section.case.apply(&title)),
            )
            .child(
                div()
                    .min_w_0()
                    .type_style(theme, theme.type_scale.hint)
                    .text_color(theme.window_muted())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(hint),
            );
        let row = div()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(titles);
        if index == 0 {
            // The search field stays through select mode (CLAUDE.md), so a search that is still
            // filtering the cards is on view; Select all and Done take Select's place.
            let controls = div().flex_none().flex().items_center().gap(px(8.)).child(
                TextField::new(&self.search, "Search names")
                    .height(px(HEADER_HEIGHT))
                    .w(px(SEARCH_WIDTH)),
            );
            let controls = if self.select_mode {
                controls
                    .child(
                        Button::new("select-all", "Select all")
                            .small()
                            .on_click(act(cx, |this, cx| this.select_all(cx))),
                    )
                    .child(
                        Button::new("done", "Done")
                            .small()
                            .primary()
                            .on_click(act(cx, |this, cx| this.leave_select_mode(cx))),
                    )
            } else {
                controls.child(
                    Button::new("select", "Select")
                        .small()
                        .on_click(act(cx, |this, cx| this.enter_select_mode(cx))),
                )
            };
            let row = row.child(controls);
            if !self.select_mode {
                return row.into_any_element();
            }
            // The session line stays through select mode (CLAUDE.md), so the mode's hint goes
            // on a line of its own beneath the row, where it has the gallery's whole width.
            let hint_style = theme.type_scale.hint;
            return div()
                .w_full()
                .flex()
                .flex_col()
                .child(row)
                .child(
                    div()
                        .id("select-hint")
                        .read_as_text(SELECT_HINT)
                        .mt(px(SELECT_HINT_GAP))
                        .type_style(theme, hint_style)
                        .line_height(hint_style.size * HINT_LEADING)
                        .text_color(theme.window_muted())
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(SELECT_HINT),
                )
                .into_any_element();
        }
        let rule = theme.borders.earlier_rule;
        div()
            .w_full()
            .mt(px(EARLIER_MARGIN))
            .pt(px(EARLIER_PADDING))
            .border_t(rule.width)
            .border_color(rule.color)
            .when(rule.dashed, |rule| rule.border_dashed())
            .child(
                row.child(
                    Button::new(
                        ("collapse", index),
                        if group.collapsed {
                            "Expand ▾"
                        } else {
                            "Collapse ▴"
                        },
                    )
                    .named(collapse_name(group.collapsed, &short_title))
                    .small()
                    .on_click(act(cx, move |this, cx| this.toggle_collapsed(index, cx))),
                ),
            )
            .into_any_element()
    }
}

pub fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// What assistive technology calls a group's Collapse or Expand button.
fn collapse_name(collapsed: bool, group: &str) -> String {
    let action = if collapsed { "Expand" } else { "Collapse" };
    format!("{action} {group}")
}

/// A day as the look writes it: "Tue, Nov 4", "NOV 4" or "TUE NOV 4".
pub(super) fn day_date(date: LocalDate, style: DayDate) -> String {
    let (weekday, month, day) = (date.weekday_short(), date.month_short(), date.day);
    match style {
        DayDate::Long => format!("{weekday}, {month} {day}"),
        DayDate::ShortUpper => format!("{month} {day}").to_uppercase(),
        DayDate::LongUpper => format!("{weekday} {month} {day}").to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOV_4: LocalDate = LocalDate {
        year: 2026,
        month: 11,
        day: 4,
    };
    #[test]
    fn day_dates_follow_the_look() {
        assert_eq!(day_date(NOV_4, DayDate::Long), "Wed, Nov 4");
        assert_eq!(day_date(NOV_4, DayDate::ShortUpper), "NOV 4");
        assert_eq!(day_date(NOV_4, DayDate::LongUpper), "WED NOV 4");
    }

    #[test]
    fn collapse_buttons_name_their_group_without_the_glyph() {
        assert_eq!(
            collapse_name(false, "Earlier today"),
            "Collapse Earlier today"
        );
        assert_eq!(collapse_name(true, "Sun, Nov 8"), "Expand Sun, Nov 8");
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(plural(1, "capture"), "1 capture");
        assert_eq!(plural(5, "capture"), "5 captures");
        assert_eq!(plural(0, "capture"), "0 captures");
    }
}
