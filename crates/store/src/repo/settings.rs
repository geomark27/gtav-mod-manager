//! Key/value application settings.

use rusqlite::{OptionalExtension, params};

use crate::database::Database;
use crate::error::Result;

/// Setting key holding the id of the selected installation.
pub const SETTING_SELECTED_INSTALLATION: &str = "selected_installation";

impl Database {
    /// Returns the value of a setting, if set.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Sets a setting, replacing any previous value.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Removes a setting; removing a missing key is not an error.
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }
}
