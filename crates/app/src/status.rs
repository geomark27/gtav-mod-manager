//! System status use case (`get_app_status`).

use chrono::SecondsFormat;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::AppError;

/// Snapshot of the application's runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppStatusDto {
    /// Application version.
    pub app_version: String,
    /// Data directory.
    pub data_dir: String,
    /// Backup vault directory.
    pub vault_dir: String,
    /// Log directory.
    pub logs_dir: String,
    /// Database file.
    pub db_path: String,
    /// Schema version (`user_version`).
    pub schema_version: u32,
    /// SQLite journal mode.
    pub journal_mode: String,
    /// SQLite synchronous level: `off`, `normal`, `full` or `extra`.
    pub synchronous: String,
    /// Whether foreign keys are enforced.
    pub foreign_keys: bool,
    /// Start time, RFC 3339 UTC.
    pub started_at: String,
    /// What startup recovery did with interrupted transactions.
    pub recovery: RecoveryDto,
}

/// Counts of the transactions startup recovery handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryDto {
    /// Interrupted transactions that were undone.
    pub rolled_back: u32,
    /// Interrupted transactions that were finished.
    pub committed: u32,
    /// Transactions that could not be undone safely; the installation stays blocked.
    pub needs_attention: u32,
    /// Transactions left untouched (installation folder unavailable).
    pub skipped: u32,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn synchronous_name(level: i64) -> String {
    match level {
        0 => "off",
        1 => "normal",
        2 => "full",
        3 => "extra",
        _ => "unknown",
    }
    .to_owned()
}

impl App {
    /// Reports paths, database pragmas and the schema version.
    pub fn status(&self) -> Result<AppStatusDto, AppError> {
        let (schema_version, pragmas) = {
            let db = self.lock_db()?;
            (db.schema_version()?, db.pragma_snapshot()?)
        };
        Ok(AppStatusDto {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            data_dir: self.dirs.data_dir.to_string_lossy().into_owned(),
            vault_dir: self.dirs.vault_dir.to_string_lossy().into_owned(),
            logs_dir: self.dirs.logs_dir.to_string_lossy().into_owned(),
            db_path: self.dirs.db_path.to_string_lossy().into_owned(),
            schema_version,
            journal_mode: pragmas.journal_mode,
            synchronous: synchronous_name(pragmas.synchronous),
            foreign_keys: pragmas.foreign_keys,
            started_at: self.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            recovery: RecoveryDto {
                rolled_back: count(self.recovery.rolled_back.len()),
                committed: count(self.recovery.committed.len()),
                needs_attention: count(self.recovery.needs_attention.len()),
                skipped: count(self.recovery.skipped.len()),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use gmm_platform::AppDirs;
    use tempfile::TempDir;

    #[test]
    fn init_creates_dirs_and_database() {
        let t = TempDir::new().unwrap();
        let dirs = AppDirs::at(&t.path().join("data"));
        let _app = App::init(dirs.clone()).unwrap();
        assert!(dirs.vault_dir.is_dir() && dirs.logs_dir.is_dir());
        assert!(dirs.db_path.is_file());
    }

    #[test]
    fn status_reports_real_values() {
        let t = TempDir::new().unwrap();
        let dirs = AppDirs::at(t.path());
        let app = App::init(dirs.clone()).unwrap();
        let s = app.status().unwrap();
        assert_eq!(s.schema_version, gmm_store::LATEST_SCHEMA_VERSION);
        assert_eq!(s.journal_mode, "wal");
        assert_eq!(s.synchronous, "full");
        assert!(s.foreign_keys);
        assert_eq!(s.data_dir, dirs.data_dir.to_string_lossy());
        assert_eq!(s.vault_dir, dirs.vault_dir.to_string_lossy());
        assert_eq!(s.logs_dir, dirs.logs_dir.to_string_lossy());
        assert_eq!(s.db_path, dirs.db_path.to_string_lossy());
        assert_eq!(s.app_version, env!("CARGO_PKG_VERSION"));
        assert!(s.started_at.ends_with('Z'));
        assert_eq!(
            s.recovery,
            RecoveryDto {
                rolled_back: 0,
                committed: 0,
                needs_attention: 0,
                skipped: 0
            }
        );
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("schemaVersion").is_some());
    }

    #[test]
    fn second_instance_is_rejected_until_first_is_dropped() {
        let t = TempDir::new().unwrap();
        let dirs = AppDirs::at(t.path());
        let first = App::init(dirs.clone()).unwrap();
        let err = App::init(dirs.clone()).unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyRunning);
        assert_eq!(
            err.hint.as_deref(),
            Some("Close the other window of GTA V Mod Manager.")
        );
        drop(first);
        assert!(App::init(dirs).is_ok());
    }

    #[test]
    fn synchronous_names() {
        assert_eq!(synchronous_name(0), "off");
        assert_eq!(synchronous_name(1), "normal");
        assert_eq!(synchronous_name(3), "extra");
        assert_eq!(synchronous_name(9), "unknown");
    }
}
