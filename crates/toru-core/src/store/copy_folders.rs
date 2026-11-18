use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::{Result, Store};

impl Store {
    /// A new empty folder for one Copy all's files. It lasts until a later set replaces it, even
    /// across restarts, since a paste needs its files long after the copy.
    pub fn new_copy_folder(&self) -> Result<PathBuf> {
        let number = self.next_copy.fetch_add(1, Ordering::Relaxed);
        let folder = self.copies_dir.join(number.to_string());
        fs::create_dir_all(&folder)?;
        Ok(folder)
    }

    /// Removes every Copy all folder but `keep`, once `keep`'s set has replaced them on the
    /// clipboard. Anything that can't be removed goes with the next set.
    pub fn remove_other_copy_folders(&self, keep: &Path) {
        remove_copy_folders_except(&self.copies_dir, keep);
    }
}

/// The highest number among the Copy all folders, which is the set made last.
pub(super) fn latest_copy_folder(copies_dir: &Path) -> Option<u64> {
    fs::read_dir(copies_dir)
        .ok()?
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .max()
}

// Failures are ignored: the next sweep tries again.
pub(super) fn remove_copy_folders_except(copies_dir: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(copies_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path() != keep {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;

    #[test]
    fn open_keeps_only_the_latest_copy_folder_for_a_paste_after_a_restart() {
        let dir = TestDir::new();
        let store = dir.open();
        let older = store.new_copy_folder().unwrap();
        let latest = store.new_copy_folder().unwrap();
        fs::write(latest.join("shot.png"), b"copied").unwrap();
        drop(store);

        let store = dir.open();
        assert!(!older.exists());
        assert_eq!(fs::read(latest.join("shot.png")).unwrap(), b"copied");

        let next = store.new_copy_folder().unwrap();
        assert_ne!(next, latest, "a new set must not land in the kept one");
    }

    #[test]
    fn a_later_copy_folder_replaces_the_one_kept_from_the_last_run() {
        let dir = TestDir::new();
        let store = dir.open();
        let kept = store.new_copy_folder().unwrap();
        drop(store);

        let store = dir.open();
        let next = store.new_copy_folder().unwrap();
        store.remove_other_copy_folders(&next);
        assert!(!kept.exists());
        assert!(next.is_dir());
    }

    #[test]
    fn open_without_copy_folders_starts_numbering_them_at_zero() {
        let dir = TestDir::new();
        let store = dir.open();
        assert!(store.new_copy_folder().unwrap().ends_with("0"));
    }
}
