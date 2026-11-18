/// Undo and redo over a value: every edit keeps the value before it.
pub struct History<T> {
    pub(super) present: T,
    past: Vec<T>,
    future: Vec<T>,
}

impl<T: Clone> History<T> {
    pub fn new(present: T) -> History<T> {
        History {
            present,
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    pub fn get(&self) -> &T {
        &self.present
    }

    /// Changes the value as one undoable step. Anything redoable is forgotten, as in any editor.
    pub fn edit(&mut self, change: impl FnOnce(&mut T)) {
        self.past.push(self.present.clone());
        self.future.clear();
        change(&mut self.present);
    }

    /// Changes the value as part of the last step, so undo takes both back together: a slider
    /// drag changes the value on every move but is one step.
    pub fn amend(&mut self, change: impl FnOnce(&mut T)) {
        change(&mut self.present);
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Steps back; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.past.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.present, previous);
        self.future.push(current);
        true
    }

    /// Steps forward again; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.present, next);
        self.past.push(current);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_undo_and_redo_in_order() {
        let mut history = History::new(vec![1]);
        history.edit(|v| v.push(2));
        history.edit(|v| v.push(3));
        assert_eq!(history.get(), &[1, 2, 3]);
        assert!(history.undo());
        assert_eq!(history.get(), &[1, 2]);
        assert!(history.undo());
        assert_eq!(history.get(), &[1]);
        assert!(!history.undo());
        assert!(history.redo());
        assert!(history.redo());
        assert_eq!(history.get(), &[1, 2, 3]);
        assert!(!history.redo());
    }

    #[test]
    fn an_edit_after_undo_drops_the_redo_branch() {
        let mut history = History::new(0);
        history.edit(|v| *v = 1);
        history.edit(|v| *v = 2);
        assert!(history.undo());
        history.edit(|v| *v = 9);
        assert!(!history.can_redo());
        assert!(history.undo());
        assert_eq!(*history.get(), 1);
    }
}
