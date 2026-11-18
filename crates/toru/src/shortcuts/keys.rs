use global_hotkey::hotkey::Code;

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

/// The number pad's digits, a key apart from the top row's: Windows tells them apart, so a
/// shortcut recorded on one only fires on that one.
const NUMPAD_DIGITS: [Code; 10] = [
    Code::Numpad0,
    Code::Numpad1,
    Code::Numpad2,
    Code::Numpad3,
    Code::Numpad4,
    Code::Numpad5,
    Code::Numpad6,
    Code::Numpad7,
    Code::Numpad8,
    Code::Numpad9,
];

/// The key for a name as stored, or as GPUI names a pressed key: a letter or digit, "Num 0" to
/// "Num 9" (or "Numpad0"), "F1" to "F24", or one of `NAMED_KEYS`, in any case.
pub(super) fn key_from_name(name: &str) -> Option<Code> {
    let upper = name.to_ascii_uppercase();
    if let [letter @ b'A'..=b'Z'] = upper.as_bytes() {
        return Some(LETTERS[usize::from(letter - b'A')]);
    }
    if let [digit @ b'0'..=b'9'] = upper.as_bytes() {
        return Some(DIGITS[usize::from(digit - b'0')]);
    }
    if let Some(rest) = upper
        .strip_prefix("NUMPAD")
        .or_else(|| upper.strip_prefix("NUM"))
        && let [digit @ b'0'..=b'9'] = rest.trim_start().as_bytes()
    {
        return Some(NUMPAD_DIGITS[usize::from(digit - b'0')]);
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
        .chain(NUMPAD_DIGITS)
        .chain(FUNCTION_KEYS)
        .chain(NAMED_KEYS.iter().map(|(code, _)| *code))
}

pub(super) fn key_name(key: Code) -> String {
    if let Some(index) = LETTERS.iter().position(|code| *code == key) {
        return char::from(b'A' + index as u8).to_string();
    }
    if let Some(index) = DIGITS.iter().position(|code| *code == key) {
        return index.to_string();
    }
    if let Some(index) = NUMPAD_DIGITS.iter().position(|code| *code == key) {
        return format!("Num {index}");
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

/// As PrtScn is by default: the function keys and PrtScn, which type nothing.
pub(super) fn stands_alone(key: Code) -> bool {
    key == Code::PrintScreen || FUNCTION_KEYS.contains(&key)
}
