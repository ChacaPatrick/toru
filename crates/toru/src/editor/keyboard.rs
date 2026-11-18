use gpui::{Context, Focusable, KeyDownEvent, Window};

use super::Editor;
use super::tools::Tool;
use super::typing::Typed;

impl Editor {
    /// The shortcuts of DESIGN.md §5.
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if self.name.focus_handle(cx).is_focused(window) && keystroke.key != "escape" {
            return;
        }
        match self.type_key(keystroke, cx) {
            Typed::Taken => {
                cx.stop_propagation();
                return;
            }
            Typed::Character => return,
            Typed::Not => {}
        }
        if m.alt || m.platform || m.function {
            return;
        }
        if keystroke.key == "escape" && !m.control {
            self.set_tool(Tool::Select, cx);
            cx.stop_propagation();
            return;
        }
        if m.control {
            match keystroke.key.as_str() {
                "c" if !m.shift => self.copy(cx),
                "z" if m.shift => self.redo(cx),
                "z" => self.undo(cx),
                "y" => self.redo(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if matches!(keystroke.key.as_str(), "delete" | "backspace") && self.selected.is_some() {
            self.delete_selected(cx);
            cx.stop_propagation();
            return;
        }
        // On any other control Enter is that control's, so Cancel cancels and Back goes back.
        if keystroke.key == "enter"
            && self.tool == Tool::Crop
            && (self.focus.is_focused(window) || self.crop_button.is_focused(window))
        {
            self.apply_crop(cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.key == "enter"
            && let Some((index, annotation)) = self.selection()
            && annotation.text().is_some()
        {
            self.edit_text(index, cx);
            cx.stop_propagation();
            return;
        }
        if let Some(tool) = Tool::for_key(&keystroke.key) {
            self.set_tool(tool, cx);
            cx.stop_propagation();
        }
    }
}
