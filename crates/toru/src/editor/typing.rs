use gpui::{Bounds, Context, Keystroke, Point};

use super::annotation::Annotation;
use super::{Editor, text_input};

/// `index` is the document's text being edited, `None` for a new one. `marked` is the end of the
/// content an input method is still composing, in bytes.
pub(super) struct Typing {
    pub(super) index: Option<usize>,
    pub(super) text: Annotation,
    pub(super) marked: Option<std::ops::Range<usize>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Typed {
    /// Not a typing key, so the editor's shortcuts may have it.
    Not,
    Taken,
    /// Reaches the text through `text_input`. The shortcuts don't have it, but it propagates:
    /// Windows makes the character only from a key-down nothing handled.
    Character,
}

/// In picture pixels.
pub(super) struct TypingFrame {
    pub(super) bounds: Bounds<f32>,
    pub(super) caret: Point<f32>,
    pub(super) caret_height: f32,
}

impl Editor {
    pub(super) fn start_typing(
        &mut self,
        index: Option<usize>,
        text: Annotation,
        cx: &mut Context<Self>,
    ) {
        self.finish_typing(cx);
        self.typing = Some(Typing {
            index,
            text,
            marked: None,
        });
        self.document_changed(cx);
    }

    pub(super) fn edit_text(&mut self, index: usize, cx: &mut Context<Self>) {
        let text = self.document.annotations()[index].clone();
        self.start_typing(Some(index), text, cx);
    }

    pub(super) fn finish_typing(&mut self, cx: &mut Context<Self>) {
        let Some(Typing { index, text, .. }) = self.typing.take() else {
            return;
        };
        let empty = text.text().is_none_or(|t| t.content.trim().is_empty());
        match (index, empty) {
            (Some(index), true) => {
                self.document.remove(index, false);
                self.select(None, cx);
            }
            (Some(index), false) => {
                self.document.replace(index, text);
            }
            (None, true) => {}
            (None, false) => {
                self.document.add(text);
            }
        }
        self.document_changed(cx);
    }

    /// Characters come through `text_input`, so AltGr and accent keys type what they show; here
    /// their keys are only kept from the shortcuts. Ctrl+Z and Ctrl+Y are left to `key_down`,
    /// whose undo and redo put the text on the document first.
    pub(super) fn type_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> Typed {
        let Some(typing) = &self.typing else {
            return Typed::Not;
        };
        let m = keystroke.modifiers;
        // AltGr is Ctrl+Alt on Windows; Ctrl alone is a shortcut.
        let ctrl = m.control && !m.alt;
        if keystroke.key == "escape" {
            self.finish_typing(cx);
            return Typed::Taken;
        }
        let Some(mut content) = typing.text.text().map(|t| t.content.clone()) else {
            return Typed::Not;
        };
        match keystroke.key.as_str() {
            "enter" if !m.control => content.push('\n'),
            "backspace" => {
                content.pop();
            }
            "v" if ctrl => {
                if let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    content.push_str(&text_input::normalize_line_breaks(&pasted));
                }
            }
            "z" | "y" if ctrl => return Typed::Not,
            "tab" => return Typed::Not,
            _ if types_character(keystroke) => return Typed::Character,
            _ => return Typed::Taken,
        }
        if let Some(typing) = &mut self.typing {
            typing.text = typing.text.with_content(content);
            typing.marked = None;
        }
        self.preview_changed(cx);
        Typed::Taken
    }

    pub(super) fn typing_frame(&self) -> Option<TypingFrame> {
        let text = &self.typing.as_ref()?.text;
        let (caret, caret_height) = text.caret()?;
        Some(TypingFrame {
            bounds: text.bounds(),
            caret,
            caret_height,
        })
    }
}

/// Windows reports AltGr as Ctrl+Alt; Ctrl or Alt alone makes a shortcut. A dead key has its
/// accent as its character.
fn types_character(keystroke: &Keystroke) -> bool {
    let m = keystroke.modifiers;
    keystroke.key_char.is_some() && m.control == m.alt && !m.platform && !m.function
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_keys_are_told_from_shortcuts() {
        let key = |key: &str, key_char: Option<&str>, modifiers: gpui::Modifiers| Keystroke {
            modifiers,
            key: key.into(),
            key_char: key_char.map(Into::into),
        };
        let none = gpui::Modifiers::default();
        let shift = gpui::Modifiers::shift();
        let alt_gr = gpui::Modifiers {
            control: true,
            alt: true,
            ..none
        };
        assert!(types_character(&key("a", Some("a"), none)));
        assert!(types_character(&key("a", Some("A"), shift)));
        assert!(types_character(&key("space", Some(" "), none)));
        assert!(types_character(&key("e", Some("€"), alt_gr)));
        // A dead key, whose accent Windows composes with the next key.
        assert!(types_character(&key("´", Some("´"), none)));
        assert!(!types_character(&key(
            "a",
            None,
            gpui::Modifiers::control()
        )));
        assert!(!types_character(&key(
            "e",
            Some("e"),
            gpui::Modifiers::alt()
        )));
        assert!(!types_character(&key("left", None, none)));
        assert!(!types_character(&key("enter", None, none)));
    }
}
