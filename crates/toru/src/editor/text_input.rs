//! Characters for the text being typed on the canvas, as the platform composes them: AltGr
//! and accent keys through the keyboard layout, and whole words through an input method. A
//! key's own name, which `Editor::type_key` sees, can't tell these apart. The caret always
//! stands at the end of the text.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, Focusable, Pixels,
    UTF16Selection, Window, point, px, size,
};

use super::{CARET, Editor};

/// While text is typed, the canvas takes the platform's text input for whatever has focus in
/// the editor, except its name field, which takes its own. Called as the canvas paints.
pub(super) fn handle_input(
    editor: &Entity<Editor>,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(focused) = window.focused(cx) else {
        return;
    };
    let this = editor.read(cx);
    let takes = this.typing.is_some()
        && this.focus.contains(&focused, window)
        && !this.name.focus_handle(cx).contains(&focused, window);
    if takes {
        window.handle_input(
            &focused,
            ElementInputHandler::new(bounds, editor.clone()),
            cx,
        );
    }
}

/// Text from the clipboard with each line break as `\n`, whichever program put it there.
pub(super) fn line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Puts `text` in place of the `marked` end of `content`, or after it if nothing is marked,
/// without control characters but with its line breaks. Returns the range it now takes if it
/// is still being composed (`mark`).
fn put(
    content: &mut String,
    marked: Option<Range<usize>>,
    text: &str,
    mark: bool,
) -> Option<Range<usize>> {
    let start = marked.map_or(content.len(), |marked| marked.start.min(content.len()));
    content.truncate(start);
    content.extend(
        line_breaks(text)
            .chars()
            .filter(|c| *c == '\n' || !c.is_control()),
    );
    (mark && content.len() > start).then_some(start..content.len())
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The byte offset in `text` of the UTF-16 offset `at`, or its end.
fn byte_offset(text: &str, at: usize) -> usize {
    let mut units = 0;
    for (offset, c) in text.char_indices() {
        if units >= at {
            return offset;
        }
        units += c.len_utf16();
    }
    text.len()
}

impl Editor {
    fn typed_content(&self) -> Option<&str> {
        Some(self.typing.as_ref()?.text.text()?.content.as_str())
    }

    fn input(&mut self, text: &str, mark: bool, cx: &mut Context<Self>) {
        let Some(typing) = &mut self.typing else {
            return;
        };
        let Some(mut content) = typing.text.text().map(|t| t.content.clone()) else {
            return;
        };
        typing.marked = put(&mut content, typing.marked.take(), text, mark);
        typing.text = typing.text.with_content(content);
        self.preview_changed(cx);
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let content = self.typed_content()?;
        let start = byte_offset(content, range.start);
        let end = byte_offset(content, range.end).max(start);
        *adjusted_range = Some(utf16_len(&content[..start])..utf16_len(&content[..end]));
        Some(content[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = utf16_len(self.typed_content()?);
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let marked = self.typing.as_ref()?.marked.clone()?;
        let content = self.typed_content()?;
        Some(utf16_len(&content[..marked.start])..utf16_len(&content[..marked.end]))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if let Some(typing) = &mut self.typing {
            typing.marked = None;
        }
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input(text, false, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        new_text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input(new_text, true, cx);
    }

    /// The caret, so an input method's window opens beside the text.
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let frame = self.typing_frame()?;
        let at = self.canvas_point(frame.caret);
        let height = frame.caret_height / self.scale * self.viewport.zoom;
        Some(Bounds::new(
            element_bounds.origin + point(px(at.x), px(at.y)),
            size(px(CARET), px(height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.typing.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_line_breaks_become_newlines() {
        assert_eq!(line_breaks("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn composed_text_replaces_itself_until_it_is_committed() {
        let mut content = String::from("Hi ");
        // An input method composes 'に', then 'にほ', then commits '日本'.
        let marked = put(&mut content, None, "に", true);
        assert_eq!(marked, Some(3..6));
        let marked = put(&mut content, marked, "にほ", true);
        assert_eq!((content.as_str(), marked.clone()), ("Hi にほ", Some(3..9)));
        let marked = put(&mut content, marked, "日本", false);
        assert_eq!((content.as_str(), marked), ("Hi 日本", None));
        // A layout's character goes after the text; control characters stay out.
        assert_eq!(put(&mut content, None, "é\u{8}", false), None);
        assert_eq!(content, "Hi 日本é");
        // Composing that is cancelled takes its text back out.
        let marked = put(&mut content, None, "x", true);
        put(&mut content, marked, "", false);
        assert_eq!(content, "Hi 日本é");
    }

    #[test]
    fn utf16_offsets_map_to_bytes() {
        let text = "a😀b";
        assert_eq!(utf16_len(text), 4);
        assert_eq!(byte_offset(text, 1), 1);
        assert_eq!(byte_offset(text, 3), 5);
        assert_eq!(byte_offset(text, 9), 6);
    }
}
