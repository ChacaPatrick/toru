//! What assistive technology reads of anything that isn't a control. GPUI only reports elements
//! with both an id and a role, so plain text is invisible to a screen reader, and AccessKit names
//! a `Role::Label` node from its value, ignoring any label, while every other role is named by its
//! label. This is the one place that knows that: give an element an `id` and one of these calls.

use gpui::{Role, prelude::*};

/// Every heading in the app is a section of a window with no outline above it. AccessKit adds one
/// for UI Automation, so this reads as level 2.
const HEADING_LEVEL: usize = 1;

/// `text` as it should be spoken: the visible separators read as a pause rather than a symbol.
pub fn spoken(text: &str) -> String {
    text.replace(" · ", ", ")
}

pub trait Readable: StatefulInteractiveElement + Sized {
    /// Standalone text, such as a status line or a path.
    fn read_as_text(self, text: impl AsRef<str>) -> Self {
        self.role(Role::Label).aria_value(spoken(text.as_ref()))
    }

    /// A section heading, with anything read along with it in `name`.
    fn read_as_heading(self, name: impl AsRef<str>) -> Self {
        self.role(Role::Heading)
            .aria_level(HEADING_LEVEL)
            .aria_label(spoken(name.as_ref()))
    }

    /// A dialog's panel: assistive technology is told the rest of the window is out of reach.
    fn modal_dialog(self) -> Self {
        self.a11y_synthetic_children(|builder| builder.parent_node().set_modal())
    }

    /// While `hidden`, takes this element and everything in it out of the accessibility tree, as
    /// behind a modal dialog. It only has a node then, so the tree is otherwise unchanged.
    fn hidden_from_readers(self, hidden: bool) -> Self {
        if !hidden {
            return self;
        }
        self.role(Role::Group)
            .a11y_synthetic_children(|builder| builder.parent_node().set_hidden())
    }
}

impl<E: StatefulInteractiveElement> Readable for E {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_are_spoken_as_pauses() {
        assert_eq!(
            spoken("800×1000 · Window · 9:44 AM"),
            "800×1000, Window, 9:44 AM"
        );
        assert_eq!(spoken("No separators"), "No separators");
    }
}
