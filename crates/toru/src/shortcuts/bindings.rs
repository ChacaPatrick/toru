use std::collections::HashMap;

use toru_core::settings::{DisplayShortcut, Settings};

use super::Shortcut;
use super::shortcut::ShortcutError;

/// What a shortcut does: one of the four capture actions, or Full screen of one display, named
/// by the stable id the platform gives it (`PlatformDisplay::uuid`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Binding {
    Region,
    Window,
    FullScreen,
    Delayed,
    Display(String),
}

/// Why a shortcut isn't working, shown in text on its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The stored string isn't a shortcut.
    Unreadable(ShortcutError),
    /// Another row has the same keys and got them first.
    Duplicate(Binding),
    /// Another app holds the combination (`AlreadyRegistered`).
    TakenByAnotherApp,
    /// Windows refused it for some other reason.
    Failed(String),
    /// The display it captures isn't connected, so it isn't registered for now.
    Disconnected,
}

/// What a shortcut row knows: its action, the stored text, the shortcut that text parses to,
/// and what is wrong, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub binding: Binding,
    pub text: Option<String>,
    pub shortcut: Option<Shortcut>,
    pub problem: Option<Problem>,
}

/// A row before planning: `connected` is false for a display shortcut whose display is away.
pub struct Entry {
    pub binding: Binding,
    pub text: Option<String>,
    pub connected: bool,
}

/// Reads every entry's text and marks the problems that need no registration: unreadable text,
/// a duplicate of an earlier row, and a display that is away. Earlier rows win a clash, so the
/// four actions, listed first, keep their keys over a display shortcut.
pub fn plan(entries: Vec<Entry>) -> Vec<Planned> {
    let mut taken: HashMap<Shortcut, Binding> = HashMap::new();
    entries
        .into_iter()
        .map(|entry| {
            let mut planned = Planned {
                binding: entry.binding,
                text: entry.text,
                shortcut: None,
                problem: None,
            };
            let Some(text) = &planned.text else {
                return planned;
            };
            match Shortcut::parse(text) {
                Err(error) => planned.problem = Some(Problem::Unreadable(error)),
                Ok(shortcut) => {
                    planned.shortcut = Some(shortcut);
                    if let Some(first) = taken.get(&shortcut) {
                        planned.problem = Some(Problem::Duplicate(first.clone()));
                    } else if !entry.connected {
                        planned.problem = Some(Problem::Disconnected);
                    } else {
                        taken.insert(shortcut, planned.binding.clone());
                    }
                }
            }
            planned
        })
        .collect()
}

/// The stored text of each of the four actions and every display shortcut, in that order.
pub fn entries(settings: &Settings, connected: impl Fn(&str) -> bool) -> Vec<Entry> {
    let action = |binding, text: &Option<String>| Entry {
        binding,
        text: text.clone(),
        connected: true,
    };
    let mut entries = vec![
        action(Binding::Region, &settings.region_shortcut),
        action(Binding::Window, &settings.window_shortcut),
        action(Binding::FullScreen, &settings.full_screen_shortcut),
        action(Binding::Delayed, &settings.delayed_shortcut),
    ];
    entries.extend(settings.display_shortcuts.iter().map(|entry| Entry {
        binding: Binding::Display(entry.display.clone()),
        text: Some(entry.shortcut.clone()),
        connected: connected(&entry.display),
    }));
    entries
}

/// Writes a binding's new shortcut, or none, to the settings. A display's keeps its place among
/// the display shortcuts, so the rows don't reorder.
pub fn set(settings: &mut Settings, binding: &Binding, shortcut: Option<Shortcut>) {
    let text = shortcut.map(|shortcut| shortcut.to_string());
    match binding {
        Binding::Region => settings.region_shortcut = text,
        Binding::Window => settings.window_shortcut = text,
        Binding::FullScreen => settings.full_screen_shortcut = text,
        Binding::Delayed => settings.delayed_shortcut = text,
        Binding::Display(display) => {
            let shortcuts = &mut settings.display_shortcuts;
            let at = shortcuts.iter().position(|entry| entry.display == *display);
            match (at, text) {
                (Some(at), Some(shortcut)) => shortcuts[at].shortcut = shortcut,
                (Some(at), None) => {
                    shortcuts.remove(at);
                }
                (None, Some(shortcut)) => shortcuts.push(DisplayShortcut {
                    display: display.clone(),
                    shortcut,
                }),
                (None, None) => {}
            }
        }
    }
}

/// The user's sentence for a problem. `name_of` gives a binding's name, as the row labels it.
pub fn problem_text(problem: &Problem, name_of: impl Fn(&Binding) -> String) -> String {
    match problem {
        Problem::Unreadable(ShortcutError::UnknownKey(key)) => {
            format!("Can't use the key “{key}”. Record it again.")
        }
        Problem::Unreadable(ShortcutError::NotOneKey) => {
            "Needs exactly one key with its modifiers. Record it again.".into()
        }
        Problem::Unreadable(ShortcutError::NeedsModifier) => {
            "Needs Ctrl, Alt or Win with it. Record it again.".into()
        }
        Problem::Duplicate(first) => format!("Already used for {}.", name_of(first)),
        Problem::TakenByAnotherApp => "Already taken by another app.".into(),
        Problem::Failed(why) => format!("Couldn't register it: {why}"),
        Problem::Disconnected => "That display isn't connected. The shortcut is kept.".into(),
    }
}

#[cfg(test)]
pub(super) mod tests {

    use super::*;
    use crate::shortcuts::refusal_text;
    use crate::shortcuts::shortcut::ShortcutError;
    use crate::shortcuts::shortcut::tests::shortcut;

    pub(crate) fn entry(binding: Binding, text: &str) -> Entry {
        Entry {
            binding,
            text: (!text.is_empty()).then(|| text.to_string()),
            connected: true,
        }
    }

    #[test]
    fn planning_reads_each_row_and_keeps_unset_rows_empty() {
        let planned = plan(vec![
            entry(Binding::Region, "PrtScn"),
            entry(Binding::Delayed, ""),
        ]);
        assert_eq!(planned[0].shortcut, Some(shortcut("PrtScn")));
        assert_eq!(planned[0].problem, None);
        assert_eq!(planned[1].text, None);
        assert_eq!(planned[1].shortcut, None);
        assert_eq!(planned[1].problem, None);
    }

    #[test]
    fn the_later_of_two_rows_with_the_same_keys_is_the_duplicate() {
        let planned = plan(vec![
            entry(Binding::Window, "Ctrl+Shift+W"),
            entry(Binding::FullScreen, "shift+ctrl+w"),
            entry(Binding::Delayed, "Ctrl+Shift+W"),
        ]);
        assert_eq!(planned[0].problem, None);
        assert_eq!(
            planned[1].problem,
            Some(Problem::Duplicate(Binding::Window))
        );
        assert_eq!(
            planned[2].problem,
            Some(Problem::Duplicate(Binding::Window))
        );
    }

    #[test]
    fn unreadable_text_is_a_problem_and_takes_no_keys() {
        let planned = plan(vec![
            entry(Binding::Region, "Ctrl+Hyper"),
            entry(Binding::Window, "Ctrl+Shift+W"),
        ]);
        assert_eq!(
            planned[0].problem,
            Some(Problem::Unreadable(ShortcutError::UnknownKey(
                "Hyper".into()
            )))
        );
        assert_eq!(planned[0].shortcut, None);
        assert_eq!(planned[1].problem, None);
    }

    #[test]
    fn an_absent_display_keeps_its_shortcut_but_frees_the_keys() {
        let planned = plan(vec![
            Entry {
                binding: Binding::Display("away".into()),
                text: Some("Ctrl+Alt+2".into()),
                connected: false,
            },
            entry(Binding::Delayed, "Ctrl+Alt+2"),
        ]);
        assert_eq!(planned[0].problem, Some(Problem::Disconnected));
        assert_eq!(planned[0].shortcut, Some(shortcut("Ctrl+Alt+2")));
        assert_eq!(planned[1].problem, None, "the keys weren't taken");
    }

    #[test]
    fn the_entries_follow_the_settings_with_the_actions_first() {
        let mut settings = Settings::default();
        settings.display_shortcuts.push(DisplayShortcut {
            display: "here".into(),
            shortcut: "Ctrl+Alt+1".into(),
        });
        settings.display_shortcuts.push(DisplayShortcut {
            display: "away".into(),
            shortcut: "Ctrl+Alt+2".into(),
        });
        let entries = entries(&settings, |display| display == "here");
        let bindings: Vec<&Binding> = entries.iter().map(|entry| &entry.binding).collect();
        assert_eq!(
            bindings,
            [
                &Binding::Region,
                &Binding::Window,
                &Binding::FullScreen,
                &Binding::Delayed,
                &Binding::Display("here".into()),
                &Binding::Display("away".into()),
            ]
        );
        assert_eq!(entries[0].text.as_deref(), Some("PrtScn"));
        assert_eq!(entries[3].text, None);
        assert!(entries[4].connected);
        assert!(!entries[5].connected);
    }

    #[test]
    fn setting_a_shortcut_writes_its_canonical_text_and_clearing_removes_it() {
        let mut settings = Settings::default();
        set(
            &mut settings,
            &Binding::Window,
            Some(shortcut("shift+ctrl+q")),
        );
        assert_eq!(settings.window_shortcut.as_deref(), Some("Ctrl+Shift+Q"));
        set(&mut settings, &Binding::Region, None);
        assert_eq!(settings.region_shortcut, None);

        let display = Binding::Display("one".into());
        set(&mut settings, &display, Some(shortcut("Ctrl+Alt+1")));
        set(&mut settings, &display, Some(shortcut("Ctrl+Alt+9")));
        assert_eq!(
            settings.display_shortcuts,
            vec![DisplayShortcut {
                display: "one".into(),
                shortcut: "Ctrl+Alt+9".into(),
            }]
        );
        set(&mut settings, &display, None);
        assert!(settings.display_shortcuts.is_empty());
        set(&mut settings, &display, None);
        assert!(settings.display_shortcuts.is_empty());
    }

    #[test]
    fn a_display_shortcut_set_again_keeps_its_place() {
        let mut settings = Settings::default();
        let (one, two) = (
            Binding::Display("one".into()),
            Binding::Display("two".into()),
        );
        set(&mut settings, &one, Some(shortcut("Ctrl+Alt+1")));
        set(&mut settings, &two, Some(shortcut("Ctrl+Alt+2")));
        set(&mut settings, &one, Some(shortcut("Ctrl+Alt+3")));
        let order: Vec<(&str, &str)> = settings
            .display_shortcuts
            .iter()
            .map(|entry| (entry.display.as_str(), entry.shortcut.as_str()))
            .collect();
        assert_eq!(order, [("one", "Ctrl+Alt+3"), ("two", "Ctrl+Alt+2")]);
    }

    #[test]
    fn problem_texts_name_the_other_row() {
        let name = |binding: &Binding| match binding {
            Binding::Window => "Window".to_string(),
            _ => "other".to_string(),
        };
        assert_eq!(
            problem_text(&Problem::Duplicate(Binding::Window), name),
            "Already used for Window."
        );
        assert_eq!(
            problem_text(&Problem::TakenByAnotherApp, name),
            "Already taken by another app."
        );
        assert_eq!(
            problem_text(
                &Problem::Unreadable(ShortcutError::UnknownKey("Hyper".into())),
                name
            ),
            "Can't use the key “Hyper”. Record it again."
        );
        assert_eq!(
            refusal_text(&ShortcutError::NeedsModifier),
            "Hold Ctrl, Alt or Win with that key."
        );
    }
}
