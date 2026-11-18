use std::fmt;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use gpui::Keystroke;

use super::keys::{key_from_name, key_name, stands_alone};
use crate::platform::HeldModifiers;

/// A key combination: any of the four modifiers and one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: Code,
}

/// Why a key press or a stored string isn't a shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutError {
    /// Not a key a shortcut can use, named as the user typed or pressed it.
    UnknownKey(String),
    /// Modifiers with no key, or two keys.
    NotOneKey,
    /// A letter, digit or other everyday key alone, or with Shift alone, would take typing from
    /// every app.
    NeedsModifier,
}

impl Shortcut {
    /// Shift on its own isn't enough: Shift+W is how a capital W is typed, in every app.
    fn new(
        control: bool,
        alt: bool,
        shift: bool,
        win: bool,
        key: Code,
    ) -> Result<Shortcut, ShortcutError> {
        if !(control || alt || win) && !stands_alone(key) {
            return Err(ShortcutError::NeedsModifier);
        }
        Ok(Shortcut {
            control,
            alt,
            shift,
            win,
            key,
        })
    }

    /// A stored string: modifiers and one key joined by `+`, in any order and case.
    pub fn parse(text: &str) -> Result<Shortcut, ShortcutError> {
        let (mut control, mut alt, mut shift, mut win) = (false, false, false, false);
        let mut key = None;
        for token in text.split('+').map(str::trim) {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => control = true,
                "alt" => alt = true,
                "shift" => shift = true,
                "win" | "super" | "meta" | "cmd" => win = true,
                _ => {
                    let code = key_from_name(token)
                        .ok_or_else(|| ShortcutError::UnknownKey(token.into()))?;
                    if key.replace(code).is_some() {
                        return Err(ShortcutError::NotOneKey);
                    }
                }
            }
        }
        Shortcut::new(
            control,
            alt,
            shift,
            win,
            key.ok_or(ShortcutError::NotOneKey)?,
        )
    }

    /// The key found down while recording (`platform::pressed_key`) with the modifiers held
    /// along with it.
    pub fn from_held(key: Code, held: HeldModifiers) -> Result<Shortcut, ShortcutError> {
        Shortcut::new(held.control, held.alt, held.shift, held.win, key)
    }

    /// A key pressed while recording, by the name GPUI gives it, where the platform can't say
    /// which key is down. GPUI names a key by what it types, so Shift+1 arrives as "!" and is
    /// refused; a modifier pressed alone arrives as no keystroke at all.
    pub fn from_keystroke(keystroke: &Keystroke) -> Result<Shortcut, ShortcutError> {
        let m = keystroke.modifiers;
        let key = key_from_name(&keystroke.key)
            .ok_or_else(|| ShortcutError::UnknownKey(keystroke.key.clone()))?;
        Shortcut::new(m.control, m.alt, m.shift, m.platform, key)
    }

    /// PrtScn with every combination of the modifiers, held by Toru while a shortcut is being
    /// recorded (`Hotkeys::listen`), so the press reaches the recorder and not Windows, whose
    /// own PrtScn would open the Snipping Tool or overwrite the clipboard.
    pub fn print_screen_combinations() -> impl Iterator<Item = Shortcut> {
        (0u8..16).map(|bits| Shortcut {
            control: bits & 1 != 0,
            alt: bits & 2 != 0,
            shift: bits & 4 != 0,
            win: bits & 8 != 0,
            key: Code::PrintScreen,
        })
    }

    pub fn hotkey(self) -> HotKey {
        let mut mods = Modifiers::empty();
        mods.set(Modifiers::CONTROL, self.control);
        mods.set(Modifiers::ALT, self.alt);
        mods.set(Modifiers::SHIFT, self.shift);
        mods.set(Modifiers::META, self.win);
        HotKey::new(Some(mods), self.key)
    }
}

/// The stored form, and what the settings column shows before spacing: "Ctrl+Shift+W".
impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (held, name) in [
            (self.control, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.win, "Win"),
        ] {
            if held {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", key_name(self.key))
    }
}

/// Why a key press was not recorded.
pub fn refusal_text(error: &ShortcutError) -> String {
    match error {
        ShortcutError::UnknownKey(key) => format!("“{key}” can't be a shortcut key."),
        ShortcutError::NotOneKey => "Press one key with its modifiers.".into(),
        ShortcutError::NeedsModifier => "Hold Ctrl, Alt or Win with that key.".into(),
    }
}

#[cfg(test)]
pub(super) mod tests {

    use gpui::Modifiers as GpuiModifiers;

    use super::*;
    use crate::shortcuts::keys;

    pub(crate) fn shortcut(text: &str) -> Shortcut {
        Shortcut::parse(text).unwrap()
    }

    fn keystroke(key: &str, control: bool, alt: bool, shift: bool, platform: bool) -> Keystroke {
        Keystroke {
            modifiers: GpuiModifiers {
                control,
                alt,
                shift,
                platform,
                function: false,
            },
            key: key.to_string(),
            key_char: None,
        }
    }

    #[test]
    fn the_defaults_parse_and_print_back_the_same() {
        for text in ["PrtScn", "Ctrl+Shift+W", "Ctrl+Shift+F"] {
            assert_eq!(shortcut(text).to_string(), text);
        }
        assert_eq!(
            shortcut("PrtScn"),
            Shortcut {
                control: false,
                alt: false,
                shift: false,
                win: false,
                key: Code::PrintScreen,
            }
        );
        assert_eq!(shortcut("Ctrl+Shift+W").key, Code::KeyW);
    }

    #[test]
    fn modifiers_come_out_in_one_order_whatever_the_case_or_order_in() {
        assert_eq!(
            shortcut("shift + WIN + control+alt+f12").to_string(),
            "Ctrl+Alt+Shift+Win+F12"
        );
        assert_eq!(shortcut("Super+PageUp").to_string(), "Win+PageUp");
        assert_eq!(shortcut("Alt+printscreen").to_string(), "Alt+PrtScn");
        assert_eq!(shortcut("ctrl+alt+7").to_string(), "Ctrl+Alt+7");
    }

    #[test]
    fn every_key_name_round_trips() {
        let keys: Vec<Code> = keys().collect();
        assert_eq!(keys.len(), 26 + 10 + 10 + 24 + 12);
        for key in keys {
            let name = key_name(key);
            assert_eq!(key_from_name(&name), Some(key), "{name}");
            assert_eq!(key_from_name(&name.to_lowercase()), Some(key), "{name}");
        }
    }

    #[test]
    fn bad_text_says_what_is_wrong() {
        assert_eq!(
            Shortcut::parse("Ctrl+Hyper"),
            Err(ShortcutError::UnknownKey("Hyper".into()))
        );
        assert_eq!(
            Shortcut::parse("F25"),
            Err(ShortcutError::UnknownKey("F25".into()))
        );
        assert_eq!(Shortcut::parse("Ctrl+Shift"), Err(ShortcutError::NotOneKey));
        assert_eq!(Shortcut::parse("Ctrl+A+B"), Err(ShortcutError::NotOneKey));
        assert_eq!(
            Shortcut::parse(""),
            Err(ShortcutError::UnknownKey("".into()))
        );
        assert_eq!(Shortcut::parse("W"), Err(ShortcutError::NeedsModifier));
        assert_eq!(Shortcut::parse("Space"), Err(ShortcutError::NeedsModifier));
        assert_eq!(
            Shortcut::parse("Shift+W"),
            Err(ShortcutError::NeedsModifier),
            "Shift+W types a capital W"
        );
        assert_eq!(
            Shortcut::parse("Shift+Space"),
            Err(ShortcutError::NeedsModifier)
        );
    }

    #[test]
    fn function_keys_and_prtscn_stand_alone_and_other_keys_need_ctrl_alt_or_win() {
        assert_eq!(shortcut("F9").to_string(), "F9");
        assert_eq!(shortcut("Shift+F9").to_string(), "Shift+F9");
        assert_eq!(shortcut("Shift+PrtScn").to_string(), "Shift+PrtScn");
        assert_eq!(Shortcut::parse("5"), Err(ShortcutError::NeedsModifier));
        assert_eq!(Shortcut::parse("Home"), Err(ShortcutError::NeedsModifier));
        assert_eq!(
            Shortcut::parse("Shift+Home"),
            Err(ShortcutError::NeedsModifier)
        );
        assert_eq!(shortcut("Ctrl+Home").to_string(), "Ctrl+Home");
        assert_eq!(shortcut("Alt+Shift+5").to_string(), "Alt+Shift+5");
        assert_eq!(shortcut("Win+Space").to_string(), "Win+Space");
    }

    #[test]
    fn a_key_press_becomes_a_shortcut_as_gpui_names_it() {
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("w", true, false, true, false)),
            Ok(shortcut("Ctrl+Shift+W"))
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("f12", false, false, false, false)),
            Ok(shortcut("F12"))
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("s", false, false, true, true)),
            Ok(shortcut("Shift+Win+S"))
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("!", true, false, false, false)),
            Err(ShortcutError::UnknownKey("!".into())),
            "Shift+1 reaches GPUI as the character, which is why the virtual key is asked for"
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("w", false, false, true, false)),
            Err(ShortcutError::NeedsModifier)
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("pageup", false, true, false, false)),
            Ok(shortcut("Alt+PageUp"))
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("a", false, false, false, false)),
            Err(ShortcutError::NeedsModifier)
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("escape", true, false, false, false)),
            Err(ShortcutError::UnknownKey("escape".into()))
        );
        assert_eq!(
            Shortcut::from_keystroke(&keystroke("tab", true, false, false, false)),
            Err(ShortcutError::UnknownKey("tab".into()))
        );
    }

    #[test]
    fn a_held_key_takes_the_modifiers_held_with_it() {
        let held = |control, alt, shift, win| HeldModifiers {
            control,
            alt,
            shift,
            win,
        };
        assert_eq!(
            Shortcut::from_held(Code::Digit1, held(true, false, true, false)),
            Ok(shortcut("Ctrl+Shift+1"))
        );
        assert_eq!(
            Shortcut::from_held(Code::PrintScreen, held(false, true, false, false)),
            Ok(shortcut("Alt+PrtScn"))
        );
        assert_eq!(
            Shortcut::from_held(Code::KeyW, held(false, false, true, false)),
            Err(ShortcutError::NeedsModifier)
        );
    }

    #[test]
    fn a_number_pad_digit_stays_a_number_pad_digit() {
        let held = HeldModifiers {
            control: true,
            alt: false,
            shift: false,
            win: false,
        };
        let recorded = Shortcut::from_held(Code::Numpad1, held).unwrap();
        assert_eq!(recorded.to_string(), "Ctrl+Num 1");
        assert_eq!(shortcut("Ctrl+Num 1"), recorded);
        assert_eq!(shortcut("ctrl+numpad1"), recorded);
        assert_ne!(recorded, shortcut("Ctrl+1"));
        assert_eq!(recorded.hotkey().key, Code::Numpad1);
        assert_ne!(recorded.hotkey().id(), shortcut("Ctrl+1").hotkey().id());
    }

    #[test]
    fn every_prtscn_combination_is_listened_for_once() {
        let combinations: Vec<Shortcut> = Shortcut::print_screen_combinations().collect();
        assert_eq!(combinations.len(), 16);
        let ids: std::collections::HashSet<u32> =
            combinations.iter().map(|s| s.hotkey().id()).collect();
        assert_eq!(ids.len(), 16);
        assert!(combinations.iter().all(|s| s.key == Code::PrintScreen));
        assert!(combinations.contains(&shortcut("PrtScn")));
        assert!(combinations.contains(&shortcut("Ctrl+Alt+Shift+Win+PrtScn")));
    }

    #[test]
    fn the_hotkey_carries_the_same_modifiers_and_key() {
        let hotkey = shortcut("Ctrl+Alt+Shift+Win+K").hotkey();
        assert_eq!(hotkey.key, Code::KeyK);
        assert!(hotkey.mods.contains(Modifiers::CONTROL));
        assert!(hotkey.mods.contains(Modifiers::ALT));
        assert!(hotkey.mods.contains(Modifiers::SHIFT));
        assert!(hotkey.mods.contains(Modifiers::META));
        assert_eq!(shortcut("PrtScn").hotkey().mods, Modifiers::empty());
        assert_ne!(
            shortcut("Ctrl+W").hotkey().id(),
            shortcut("Ctrl+F").hotkey().id()
        );
    }
}
