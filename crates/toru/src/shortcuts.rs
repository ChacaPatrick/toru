//! Global shortcuts (DESIGN.md §3 and `docs/spikes.md` §1.1): the text a shortcut is stored as,
//! the key press that records a new one, which shortcuts clash with each other, and the
//! `global-hotkey` registry that makes them work whenever Toru runs, hidden in the tray included.
//!
//! A stored shortcut reads as the user sees it, "Ctrl+Shift+W" or "PrtScn". A bad stored string
//! or a combination another app holds is reported as a `Problem` on its row rather than stopping
//! anything else. While a shortcut is registered no other app gets that key, so the set is kept
//! as small as the settings say, and the registry only ever adds PrtScn, which Windows would act
//! on itself, for the moment a shortcut is being recorded.

use std::{
    collections::HashMap,
    fmt,
    sync::mpsc::{SendError, Sender},
};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::Keystroke;
use toru_core::settings::{DisplayShortcut, Settings};

use crate::platform::HeldModifiers;

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

/// Keys named by something other than their own letter or digit.
const NAMED_KEYS: [(Code, &str); 12] = [
    (Code::PrintScreen, "PrtScn"),
    (Code::Space, "Space"),
    (Code::Insert, "Insert"),
    (Code::Delete, "Delete"),
    (Code::Home, "Home"),
    (Code::End, "End"),
    (Code::PageUp, "PageUp"),
    (Code::PageDown, "PageDown"),
    (Code::ArrowUp, "Up"),
    (Code::ArrowDown, "Down"),
    (Code::ArrowLeft, "Left"),
    (Code::ArrowRight, "Right"),
];

const FUNCTION_KEYS: [Code; 24] = [
    Code::F1,
    Code::F2,
    Code::F3,
    Code::F4,
    Code::F5,
    Code::F6,
    Code::F7,
    Code::F8,
    Code::F9,
    Code::F10,
    Code::F11,
    Code::F12,
    Code::F13,
    Code::F14,
    Code::F15,
    Code::F16,
    Code::F17,
    Code::F18,
    Code::F19,
    Code::F20,
    Code::F21,
    Code::F22,
    Code::F23,
    Code::F24,
];

const LETTERS: [Code; 26] = [
    Code::KeyA,
    Code::KeyB,
    Code::KeyC,
    Code::KeyD,
    Code::KeyE,
    Code::KeyF,
    Code::KeyG,
    Code::KeyH,
    Code::KeyI,
    Code::KeyJ,
    Code::KeyK,
    Code::KeyL,
    Code::KeyM,
    Code::KeyN,
    Code::KeyO,
    Code::KeyP,
    Code::KeyQ,
    Code::KeyR,
    Code::KeyS,
    Code::KeyT,
    Code::KeyU,
    Code::KeyV,
    Code::KeyW,
    Code::KeyX,
    Code::KeyY,
    Code::KeyZ,
];

const DIGITS: [Code; 10] = [
    Code::Digit0,
    Code::Digit1,
    Code::Digit2,
    Code::Digit3,
    Code::Digit4,
    Code::Digit5,
    Code::Digit6,
    Code::Digit7,
    Code::Digit8,
    Code::Digit9,
];

/// The key for a name as stored, or as GPUI names a pressed key: a letter or digit, "F1" to
/// "F24", or one of `NAMED_KEYS`, in any case.
fn key_from_name(name: &str) -> Option<Code> {
    let upper = name.to_ascii_uppercase();
    if let [letter @ b'A'..=b'Z'] = upper.as_bytes() {
        return Some(LETTERS[usize::from(letter - b'A')]);
    }
    if let [digit @ b'0'..=b'9'] = upper.as_bytes() {
        return Some(DIGITS[usize::from(digit - b'0')]);
    }
    if let Some(number) = upper.strip_prefix('F')
        && let Ok(number @ 1..=24) = number.parse::<usize>()
    {
        return Some(FUNCTION_KEYS[number - 1]);
    }
    if upper == "PRINTSCREEN" {
        return Some(Code::PrintScreen);
    }
    NAMED_KEYS
        .iter()
        .find(|(_, named)| named.eq_ignore_ascii_case(name))
        .map(|(code, _)| *code)
}

/// Every key a shortcut can use. While a shortcut is being recorded the platform is asked which
/// of these is down (`platform::pressed_key`): Windows numbers keys the same on every layout
/// where GPUI names them by what they type, so Shift+1 isn't "!" and the letters on an AZERTY
/// or Cyrillic layout aren't misnamed.
pub fn keys() -> impl Iterator<Item = Code> {
    LETTERS
        .into_iter()
        .chain(DIGITS)
        .chain(FUNCTION_KEYS)
        .chain(NAMED_KEYS.iter().map(|(code, _)| *code))
}

fn key_name(key: Code) -> String {
    if let Some(index) = LETTERS.iter().position(|code| *code == key) {
        return char::from(b'A' + index as u8).to_string();
    }
    if let Some(index) = DIGITS.iter().position(|code| *code == key) {
        return index.to_string();
    }
    if let Some(index) = FUNCTION_KEYS.iter().position(|code| *code == key) {
        return format!("F{}", index + 1);
    }
    NAMED_KEYS
        .iter()
        .find(|(code, _)| *code == key)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| format!("{key:?}"))
}

/// Whether the key can be a shortcut on its own, as PrtScn is by default: the function keys
/// and PrtScn, which type nothing.
fn stands_alone(key: Code) -> bool {
    key == Code::PrintScreen || FUNCTION_KEYS.contains(&key)
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

/// The registry. Creating it opens the hidden window that receives the presses, on GPUI's main
/// thread, whose message loop serves it (`docs/spikes.md` §1.1). Each press is sent through
/// `sender` as the pressed shortcut's id, since the handler runs where GPUI can't be touched.
pub struct Hotkeys {
    manager: Option<GlobalHotKeyManager>,
    /// What is registered right now, by the id a press carries.
    registered: HashMap<u32, (Shortcut, Binding)>,
    /// Held only while a shortcut is being recorded (`listen`), by the same id.
    listening: HashMap<u32, Shortcut>,
}

impl Hotkeys {
    pub fn new<T: Send + 'static>(
        sender: Sender<T>,
        pressed: impl Fn(u32) -> T + Send + Sync + 'static,
    ) -> Hotkeys {
        let manager = match GlobalHotKeyManager::new() {
            Ok(manager) => Some(manager),
            Err(error) => {
                eprintln!("Toru couldn't set up global shortcuts: {error}");
                None
            }
        };
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() == HotKeyState::Pressed {
                let _: Result<(), SendError<T>> = sender.send(pressed(event.id()));
            }
        }));
        Hotkeys {
            manager,
            registered: HashMap::new(),
            listening: HashMap::new(),
        }
    }

    /// Registers every planned shortcut that has no problem yet, in place of whatever was
    /// registered before, and notes on each row what Windows refused.
    pub fn apply(&mut self, planned: &mut [Planned]) {
        self.suspend();
        let Some(manager) = &self.manager else {
            for row in planned.iter_mut().filter(|row| row.shortcut.is_some()) {
                row.problem
                    .get_or_insert(Problem::Failed("Shortcuts aren't available.".into()));
            }
            return;
        };
        for row in planned.iter_mut() {
            let Some(shortcut) = row.shortcut.filter(|_| row.problem.is_none()) else {
                continue;
            };
            let hotkey = shortcut.hotkey();
            // A display plugged in while recording may want keys held for the recording.
            if self.listening.remove(&hotkey.id()).is_some() {
                manager.unregister(hotkey).ok();
            }
            match manager.register(hotkey) {
                Ok(()) => {
                    self.registered
                        .insert(hotkey.id(), (shortcut, row.binding.clone()));
                }
                Err(error) => row.problem = Some(registration_problem(error)),
            }
        }
    }

    /// Lets every shortcut go, so the keys reach other apps. `apply` brings them back.
    pub fn suspend(&mut self) {
        if let Some(manager) = &self.manager {
            for (shortcut, _) in self.registered.values() {
                manager.unregister(shortcut.hotkey()).ok();
            }
        }
        self.registered.clear();
    }

    /// Holds `shortcuts` too, until `stop_listening`, so that while a shortcut is being recorded
    /// their presses reach `listening` rather than Windows. One a row already has, or another
    /// app holds, is left as it is.
    pub fn listen(&mut self, shortcuts: impl Iterator<Item = Shortcut>) {
        self.stop_listening();
        let Some(manager) = &self.manager else {
            return;
        };
        for shortcut in shortcuts {
            let hotkey = shortcut.hotkey();
            if !self.registered.contains_key(&hotkey.id()) && manager.register(hotkey).is_ok() {
                self.listening.insert(hotkey.id(), shortcut);
            }
        }
    }

    pub fn stop_listening(&mut self) {
        if let Some(manager) = &self.manager {
            for shortcut in self.listening.values() {
                manager.unregister(shortcut.hotkey()).ok();
            }
        }
        self.listening.clear();
    }

    /// What the shortcut with `id` does, if it is one of Toru's.
    pub fn binding(&self, id: u32) -> Option<&Binding> {
        self.registered.get(&id).map(|(_, binding)| binding)
    }

    /// The keys pressed, whether a row's or one held for recording (`listen`), so that while a
    /// shortcut is being recorded a press of any of them is recorded rather than acted on.
    pub fn pressed(&self, id: u32) -> Option<Shortcut> {
        self.listening
            .get(&id)
            .copied()
            .or_else(|| self.registered.get(&id).map(|(shortcut, _)| *shortcut))
    }
}

/// `AlreadyRegistered` is the one failure the user can do something about.
fn registration_problem(error: global_hotkey::Error) -> Problem {
    match error {
        global_hotkey::Error::AlreadyRegistered(_) => Problem::TakenByAnotherApp,
        other => Problem::Failed(other.to_string()),
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

/// Why a key press was not recorded.
pub fn refusal_text(error: &ShortcutError) -> String {
    match error {
        ShortcutError::UnknownKey(key) => format!("“{key}” can't be a shortcut key."),
        ShortcutError::NotOneKey => "Press one key with its modifiers.".into(),
        ShortcutError::NeedsModifier => "Hold Ctrl, Alt or Win with that key.".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use gpui::Modifiers as GpuiModifiers;

    use super::*;

    fn shortcut(text: &str) -> Shortcut {
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
        assert_eq!(keys.len(), 26 + 10 + 24 + 12);
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

    fn entry(binding: Binding, text: &str) -> Entry {
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

    /// Registers a combination nobody uses, for the moment the test takes: Ctrl+Alt+Shift+Win+F24.
    /// A second registry wanting the same keys is told another app has them, and once the
    /// first lets go the second gets them.
    #[test]
    #[cfg(windows)]
    fn a_combination_another_registry_holds_is_taken_by_another_app() {
        let unusual = shortcut("Ctrl+Alt+Shift+Win+F24");
        let (sender, _receiver) = mpsc::channel::<u32>();
        let mut first = Hotkeys::new(sender.clone(), |id| id);
        let mut second = Hotkeys::new(sender, |id| id);
        assert!(first.manager.is_some() && second.manager.is_some());
        let mut rows = plan(vec![entry(Binding::Delayed, "Ctrl+Alt+Shift+Win+F24")]);

        first.apply(&mut rows);
        assert_eq!(rows[0].problem, None);
        assert_eq!(
            first.binding(unusual.hotkey().id()),
            Some(&Binding::Delayed)
        );

        let mut again = rows.clone();
        second.apply(&mut again);
        assert_eq!(again[0].problem, Some(Problem::TakenByAnotherApp));

        first.suspend();
        assert_eq!(first.binding(unusual.hotkey().id()), None);
        let mut after = rows.clone();
        second.apply(&mut after);
        assert_eq!(after[0].problem, None);
        second.suspend();
    }

    /// Listening holds keys for recording without making them a row's, skips a row's own, and
    /// lets them go again. Unusual combinations stand in for PrtScn so the owner's key isn't
    /// taken while the test runs.
    #[test]
    #[cfg(windows)]
    fn listened_keys_are_recorded_not_acted_on_and_let_go_afterwards() {
        let own = shortcut("Ctrl+Alt+Shift+Win+F22");
        let extra = shortcut("Ctrl+Alt+Shift+Win+F23");
        let (sender, _receiver) = mpsc::channel::<u32>();
        let mut hotkeys = Hotkeys::new(sender.clone(), |id| id);
        let mut other = Hotkeys::new(sender, |id| id);
        let mut rows = plan(vec![entry(Binding::Delayed, "Ctrl+Alt+Shift+Win+F22")]);
        hotkeys.apply(&mut rows);
        assert_eq!(rows[0].problem, None);

        hotkeys.listen([own, extra].into_iter());
        assert_eq!(hotkeys.pressed(own.hotkey().id()), Some(own));
        assert_eq!(hotkeys.pressed(extra.hotkey().id()), Some(extra));
        assert_eq!(hotkeys.binding(extra.hotkey().id()), None);
        assert_eq!(
            hotkeys.listening.len(),
            1,
            "the row's own keys aren't held twice"
        );
        let mut taken = plan(vec![entry(Binding::Region, "Ctrl+Alt+Shift+Win+F23")]);
        other.apply(&mut taken);
        assert_eq!(taken[0].problem, Some(Problem::TakenByAnotherApp));

        hotkeys.stop_listening();
        assert_eq!(hotkeys.pressed(extra.hotkey().id()), None);
        assert_eq!(hotkeys.pressed(own.hotkey().id()), Some(own));
        let mut free = plan(vec![entry(Binding::Region, "Ctrl+Alt+Shift+Win+F23")]);
        other.apply(&mut free);
        assert_eq!(free[0].problem, None);
        other.suspend();
        hotkeys.suspend();
    }
}
