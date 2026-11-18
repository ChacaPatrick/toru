//! Global shortcuts (DESIGN.md §3 and `docs/spikes.md` §1.1).

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
