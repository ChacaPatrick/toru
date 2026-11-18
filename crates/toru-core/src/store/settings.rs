use rusqlite::params;

use super::{Result, Store};
use crate::settings::Settings;

impl Store {
    pub fn load_settings(&self) -> Result<Settings> {
        let db = self.db();
        let mut statement = db.prepare("SELECT key, value FROM settings")?;
        let mut rows = statement.query([])?;
        let mut settings = Settings::default();
        while let Some(row) = rows.next()? {
            // A value that isn't valid text is one more malformed value, not a reason to fail.
            if let (Ok(key), Ok(value)) = (row.get_ref(0)?.as_str(), row.get_ref(1)?.as_str()) {
                settings.apply_stored(key, value);
            }
        }
        Ok(settings)
    }

    /// All or nothing.
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let values = settings.stored_values()?;
        let mut db = self.db();
        let transaction = db.transaction()?;
        for (key, value) in values {
            transaction.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::tests::every_field_changed;
    use crate::store::StoreError;
    use crate::store::test_support::*;

    #[test]
    fn a_new_store_loads_the_default_settings() {
        let dir = TestDir::new();
        assert_eq!(dir.open().load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn a_database_from_before_settings_gets_the_table_on_open() {
        let dir = TestDir::new();
        let store = dir.open();
        store.db().execute_batch("DROP TABLE settings").unwrap();
        drop(store);
        assert_eq!(dir.open().load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn settings_survive_reopening() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        drop(store);
        assert_eq!(dir.open().load_settings().unwrap(), every_field_changed());
    }

    #[test]
    fn saving_settings_again_replaces_them() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        let cleared = Settings {
            region_shortcut: None,
            save_folder: None,
            ..Settings::default()
        };
        store.save_settings(&cleared).unwrap();
        assert_eq!(store.load_settings().unwrap(), cleared);
    }

    #[test]
    fn a_missing_setting_takes_its_default_and_the_others_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute("DELETE FROM settings WHERE key = 'look'", [])
            .unwrap();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                look: Settings::default().look,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn a_malformed_setting_takes_its_default_and_the_others_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute("UPDATE settings SET value = 'huh' WHERE key = 'format'", [])
            .unwrap();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                format: Settings::default().format,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn stored_values_that_are_not_text_do_not_fail_the_load() {
        let dir = TestDir::new();
        let store = dir.open();
        store.save_settings(&every_field_changed()).unwrap();
        store
            .db()
            .execute_batch(
                "UPDATE settings SET value = x'00ff' WHERE key = 'on_close';
                 UPDATE settings SET value = CAST(x'ff' AS TEXT) WHERE key = 'look';
                 INSERT INTO settings (key, value) VALUES (x'ff', 'x');",
            )
            .unwrap();
        let defaults = Settings::default();
        assert_eq!(
            store.load_settings().unwrap(),
            Settings {
                on_close: defaults.on_close,
                look: defaults.look,
                ..every_field_changed()
            }
        );
    }

    #[test]
    fn a_failed_settings_save_changes_nothing() {
        let dir = TestDir::new();
        let store = dir.open();
        store
            .db()
            .execute_batch(
                "CREATE TRIGGER refuse BEFORE INSERT ON settings
                    WHEN NEW.key = 'settings_collapsed'
                    BEGIN SELECT RAISE(ABORT, 'refused'); END",
            )
            .unwrap();

        let result = store.save_settings(&every_field_changed());
        assert!(matches!(result, Err(StoreError::Database(_))));
        assert_eq!(store.load_settings().unwrap(), Settings::default());
    }
}
