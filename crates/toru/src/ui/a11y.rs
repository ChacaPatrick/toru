use gpui::{Role, prelude::*};

/// Every heading is a section of a window with no outline above it; AccessKit adds one for UI
/// Automation, so this reads as level 2.
///
/// Narrator's heading navigation skips these: accesskit_windows (0.34 in gpui-pre 0.3.11, and
/// 0.35) never sets `HeadingLevel`. Proposed upstream in
/// https://github.com/AccessKit/accesskit/issues/840; once released, these report `HeadingLevel2`
/// with no change here.
const HEADING_LEVEL: usize = 1;

/// The visible separators read as a pause rather than a symbol.
pub fn spoken(text: &str) -> String {
    text.replace(" · ", ", ")
}

/// GPUI only reports elements with both an id and a role, so plain text needs an `id` and one of
/// these calls to reach a screen reader.
pub trait Readable: StatefulInteractiveElement + Sized {
    /// Standalone text, such as a status line or a path. AccessKit names a `Role::Label` node from
    /// its value, ignoring any label.
    fn read_as_text(self, text: impl AsRef<str>) -> Self {
        self.role(Role::Label).aria_value(spoken(text.as_ref()))
    }

    fn read_as_heading(self, name: impl AsRef<str>) -> Self {
        self.role(Role::Heading)
            .aria_level(HEADING_LEVEL)
            .aria_label(spoken(name.as_ref()))
    }

    /// Assistive technology is told the rest of the window is out of reach.
    fn modal_dialog(self) -> Self {
        self.a11y_synthetic_children(|builder| builder.parent_node().set_modal())
    }

    /// It only has a node while hidden, so the tree is otherwise unchanged.
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
