use global_hotkey::hotkey::Code;

use crate::platform::HeldModifiers;

const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12;
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;
/// The first letter, digit and function key; the rest follow in order.
const VK_A: i32 = 0x41;
const VK_0: i32 = 0x30;
const VK_NUMPAD0: i32 = 0x60;
const VK_F1: i32 = 0x70;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetKeyState(key: i32) -> i16;
}

/// Whether the key with the virtual-key code `key` is down in the key event being handled.
/// `GetKeyState` follows the thread's messages, so inside a key-down handler it says which
/// keys were down as that key went down, whatever the layout names them.
pub(super) fn key_down(key: i32) -> bool {
    // SAFETY: a plain user32 call with a virtual-key code.
    (unsafe { GetKeyState(key) } as u16) & 0x8000 != 0
}

/// The virtual-key code of a key a shortcut can use, by the key's name: the letters, digits,
/// number pad digits and function keys are numbered in order, and the few other keys are
/// named one by one. `None` for a key a shortcut can't use.
pub(super) fn virtual_key(key: Code) -> Option<i32> {
    let name = key.to_string();
    match name.as_bytes() {
        [b'K', b'e', b'y', letter @ b'A'..=b'Z'] => {
            return Some(VK_A + i32::from(letter - b'A'));
        }
        [b'D', b'i', b'g', b'i', b't', digit @ b'0'..=b'9'] => {
            return Some(VK_0 + i32::from(digit - b'0'));
        }
        [b'N', b'u', b'm', b'p', b'a', b'd', digit @ b'0'..=b'9'] => {
            return Some(VK_NUMPAD0 + i32::from(digit - b'0'));
        }
        _ => {}
    }
    if let Some(number) = name.strip_prefix('F')
        && let Ok(number @ 1..=24) = number.parse::<i32>()
    {
        return Some(VK_F1 + number - 1);
    }
    Some(match key {
        Code::PrintScreen => 0x2C,
        Code::Space => 0x20,
        Code::Insert => 0x2D,
        Code::Delete => 0x2E,
        Code::Home => 0x24,
        Code::End => 0x23,
        Code::PageUp => 0x21,
        Code::PageDown => 0x22,
        Code::ArrowUp => 0x26,
        Code::ArrowDown => 0x28,
        Code::ArrowLeft => 0x25,
        Code::ArrowRight => 0x27,
        _ => return None,
    })
}

/// The first of `keys` that `key_down` finds down.
pub(super) fn pressed_among(
    keys: impl Iterator<Item = Code>,
    key_down: impl Fn(i32) -> bool,
) -> Option<Code> {
    let mut keys = keys;
    keys.find(|key| virtual_key(*key).is_some_and(&key_down))
}

/// Which of `keys` is down in the key event being handled (`key_down`), or `None` if none
/// of them is.
pub fn pressed_key(keys: impl Iterator<Item = Code>) -> Option<Code> {
    pressed_among(keys, key_down)
}

/// The modifiers down in the key event being handled (`key_down`).
pub fn held_modifiers() -> HeldModifiers {
    HeldModifiers {
        control: key_down(VK_CONTROL),
        alt: key_down(VK_MENU),
        shift: key_down(VK_SHIFT),
        win: key_down(VK_LWIN) || key_down(VK_RWIN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_key_has_its_own_virtual_key_code() {
        use global_hotkey::hotkey::Code;

        assert_eq!(virtual_key(Code::KeyA), Some(0x41));
        assert_eq!(virtual_key(Code::KeyZ), Some(0x5A));
        assert_eq!(virtual_key(Code::Digit0), Some(0x30));
        assert_eq!(virtual_key(Code::Digit9), Some(0x39));
        assert_eq!(virtual_key(Code::Numpad0), Some(0x60));
        assert_eq!(virtual_key(Code::Numpad9), Some(0x69));
        assert_eq!(virtual_key(Code::F1), Some(0x70));
        assert_eq!(virtual_key(Code::F24), Some(0x87));
        assert_eq!(virtual_key(Code::PrintScreen), Some(0x2C));
        assert_eq!(virtual_key(Code::PageDown), Some(0x22));
        for other in [
            Code::Escape,
            Code::Tab,
            Code::Enter,
            Code::Fn,
            Code::F25,
            Code::Comma,
        ] {
            assert_eq!(virtual_key(other), None, "{other}");
        }
        let codes: Vec<i32> = crate::shortcuts::keys()
            .map(|key| virtual_key(key).expect("every shortcut key has a code"))
            .collect();
        let distinct: std::collections::HashSet<i32> = codes.iter().copied().collect();
        assert_eq!(distinct.len(), codes.len());
    }

    #[test]
    fn the_key_down_is_found_by_its_virtual_key_code_whatever_it_types() {
        use global_hotkey::hotkey::Code;

        let keys = crate::shortcuts::keys;
        // Ctrl+Shift+1: GPUI would say "!" and nothing else is down.
        assert_eq!(pressed_among(keys(), |vk| vk == 0x31), Some(Code::Digit1));
        // Ctrl+Numpad1: GPUI would say "1", as for the top row.
        assert_eq!(pressed_among(keys(), |vk| vk == 0x61), Some(Code::Numpad1));
        assert_eq!(
            pressed_among(keys(), |vk| vk == 0x2C),
            Some(Code::PrintScreen)
        );
        // Esc, Tab and Enter aren't keys a shortcut can use.
        for other in [0x1B, 0x09, 0x0D] {
            assert_eq!(pressed_among(keys(), |vk| vk == other), None);
        }
        assert_eq!(pressed_among(keys(), |_| false), None);
    }

    #[test]
    fn nothing_is_down_in_a_test_with_no_key_event() {
        // `GetKeyState` follows this thread's messages, and a test thread gets none.
        assert!(!key_down(0x41));
        assert_eq!(pressed_key(crate::shortcuts::keys()), None);
        assert_eq!(
            held_modifiers(),
            HeldModifiers {
                control: false,
                alt: false,
                shift: false,
                win: false,
            }
        );
    }
}
