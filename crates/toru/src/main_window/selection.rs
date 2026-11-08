//! Select mode's bookkeeping (DESIGN.md §4), kept apart from the gallery so it can be tested on
//! its own: which cards are selected, by id, and the anchor a Shift-click ranges from. The
//! gallery hands in the listed cards in reading order; a range is everything listed between the
//! anchor and the pressed card, so it never picks up a card folded away by Collapse or dropped
//! by a search.

use std::collections::HashSet;
use std::path::Path;

use gpui::Modifiers;

/// Whether a key press of `a` with these modifiers is select mode's Ctrl+A. AltGr reaches the
/// window as Ctrl+Alt, and AltGr+A types a letter on many layouts, so Alt rules it out; so does
/// Shift, which Ctrl+Shift+A is not.
pub fn selects_all(modifiers: &Modifiers) -> bool {
    modifiers.control && !modifiers.alt && !modifiers.shift
}

/// Whether a press on a card outside select mode enters it: Ctrl held, without Alt, for the
/// same reason as `selects_all`.
pub fn enters_select_mode(modifiers: &Modifiers) -> bool {
    modifiers.control && !modifiers.alt
}

#[derive(Default)]
pub struct Selection {
    ids: HashSet<usize>,
    /// The last card pressed on its own, which the next Shift-click ranges from.
    anchor: Option<usize>,
}

impl Selection {
    pub fn contains(&self, id: usize) -> bool {
        self.ids.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// A plain press: the card flips, and becomes the anchor.
    pub fn toggle(&mut self, id: usize) {
        if !self.ids.remove(&id) {
            self.ids.insert(id);
        }
        self.anchor = Some(id);
    }

    /// A Shift-press: selects every card listed from the anchor to `id`, in `listed` order. With
    /// no anchor listed it is a plain press. The anchor stays, so a second Shift-press ranges
    /// from the same place.
    pub fn extend_to(&mut self, id: usize, listed: &[usize]) {
        let anchor = self
            .anchor
            .and_then(|a| listed.iter().position(|&c| c == a));
        let target = listed.iter().position(|&c| c == id);
        match (anchor, target) {
            (Some(anchor), Some(target)) => {
                let (from, to) = (anchor.min(target), anchor.max(target));
                self.ids.extend(&listed[from..=to]);
            }
            _ => self.toggle(id),
        }
    }

    /// Select all: every listed card.
    pub fn set_all(&mut self, listed: &[usize]) {
        self.ids.extend(listed);
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }

    /// Drops any card no longer listed: one a search filtered out, one in a group that was
    /// collapsed, or one in an earlier group hidden by Show previous. What the bulk bar counts is
    /// then always what the checkboxes show.
    pub fn retain_listed(&mut self, listed: &[usize]) {
        self.ids.retain(|id| listed.contains(id));
        self.anchor = self.anchor.filter(|id| listed.contains(id));
    }
}

/// The path hint under a saved card's chip: the file in its folder, with the rest of the path
/// elided, as the mockups show ("…\Screenshots\login-bug-repro.png").
pub fn path_hint(path: &Path) -> String {
    let file = path.file_name().map(|f| f.to_string_lossy());
    let folder = path
        .parent()
        .and_then(Path::file_name)
        .map(|f| f.to_string_lossy());
    match (folder, file) {
        (Some(folder), Some(file)) => format!("…\\{folder}\\{file}"),
        (None, Some(file)) => file.into_owned(),
        _ => path.display().to_string(),
    }
}

/// "3 selected" and "2 saved · 1 unsaved" for the bulk bar.
pub struct Counts {
    pub selected: usize,
    pub saved: usize,
}

impl Counts {
    pub fn unsaved(&self) -> usize {
        self.selected - self.saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const LISTED: [usize; 6] = [10, 11, 12, 20, 21, 22];

    fn selected(selection: &Selection) -> Vec<usize> {
        let mut ids: Vec<usize> = LISTED
            .iter()
            .copied()
            .filter(|&id| selection.contains(id))
            .collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn a_press_toggles_and_sets_the_anchor() {
        let mut s = Selection::default();
        s.toggle(11);
        assert!(s.contains(11) && s.len() == 1);
        s.toggle(11);
        assert!(s.is_empty());
        assert_eq!(s.anchor, Some(11));
    }

    #[test]
    fn a_shift_press_selects_the_range_either_way_round() {
        let mut s = Selection::default();
        s.toggle(21);
        s.extend_to(11, &LISTED);
        assert_eq!(selected(&s), [11, 12, 20, 21]);
        // The anchor stays, so the next Shift-press ranges from it again.
        s.extend_to(22, &LISTED);
        assert_eq!(selected(&s), [11, 12, 20, 21, 22]);
        assert_eq!(s.anchor, Some(21));
    }

    #[test]
    fn a_shift_press_with_no_anchor_listed_is_a_plain_press() {
        let mut s = Selection::default();
        s.extend_to(12, &LISTED);
        assert_eq!(selected(&s), [12]);
        // An anchor that was filtered out of the list doesn't range.
        s.toggle(99);
        s.extend_to(20, &LISTED);
        assert_eq!(selected(&s), [12, 20]);
    }

    #[test]
    fn select_all_and_clear() {
        let mut s = Selection::default();
        s.toggle(10);
        s.set_all(&LISTED);
        assert_eq!(selected(&s), LISTED);
        s.clear();
        assert!(s.is_empty() && s.anchor.is_none());
    }

    #[test]
    fn unlisted_cards_leave_the_selection() {
        let mut s = Selection::default();
        s.set_all(&LISTED);
        s.toggle(22);
        s.toggle(22);
        s.retain_listed(&[10, 12, 21]);
        assert_eq!(selected(&s), [10, 12, 21]);
        // The anchor went with its card.
        assert_eq!(s.anchor, None);
    }

    #[test]
    fn ctrl_a_needs_ctrl_alone_and_a_ctrl_press_needs_no_alt() {
        let ctrl = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        let alt_gr = Modifiers { alt: true, ..ctrl };
        let ctrl_shift = Modifiers {
            shift: true,
            ..ctrl
        };
        assert!(selects_all(&ctrl));
        assert!(!selects_all(&alt_gr));
        assert!(!selects_all(&ctrl_shift));
        assert!(!selects_all(&Modifiers::default()));
        assert!(enters_select_mode(&ctrl));
        assert!(enters_select_mode(&ctrl_shift));
        assert!(!enters_select_mode(&alt_gr));
        assert!(!enters_select_mode(&Modifiers::default()));
    }

    #[test]
    fn path_hints_keep_the_folder_and_file() {
        assert_eq!(
            path_hint(&PathBuf::from(
                r"C:\Users\Pat\Pictures\Screenshots\login-bug-repro.png"
            )),
            r"…\Screenshots\login-bug-repro.png"
        );
        assert_eq!(path_hint(&PathBuf::from("shot.png")), "shot.png");
    }

    #[test]
    fn counts_split_saved_and_unsaved() {
        let counts = Counts {
            selected: 3,
            saved: 2,
        };
        assert_eq!(counts.unsaved(), 1);
    }
}
