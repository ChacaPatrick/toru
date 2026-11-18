//! Global shortcuts (DESIGN.md §3 and `docs/spikes.md` §1.1): the text a shortcut is stored as,
//! the key press that records a new one, which shortcuts clash with each other, and the
//! `global-hotkey` registry that makes them work whenever Toru runs, hidden in the tray included.
//!
//! A stored shortcut reads as the user sees it, "Ctrl+Shift+W" or "PrtScn". A bad stored string
//! or a combination another app holds is reported as a `Problem` on its row rather than stopping
//! anything else. While a shortcut is registered no other app gets that key, so the set is kept
//! as small as the settings say, and the registry only ever adds PrtScn, which Windows would act
//! on itself, for the moment a shortcut is being recorded.

mod bindings;
mod hotkeys;
mod keys;
mod shortcut;

pub use bindings::{Binding, Planned, Problem, entries, plan, problem_text, set};
pub use hotkeys::Hotkeys;
pub use keys::keys;
#[cfg(test)]
pub use shortcut::ShortcutError;
pub use shortcut::{Shortcut, refusal_text};
