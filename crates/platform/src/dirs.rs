//! Application directories (data, vault, logs, database).

use crate::error::{Error, Result};
use crate::fs::Fs;
use std::path::{Path, PathBuf};

/// Locations used by the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// Root data directory.
    pub data_dir: PathBuf,
    /// Backup vault directory.
    pub vault_dir: PathBuf,
    /// Log directory.
    pub logs_dir: PathBuf,
    /// SQLite database file.
    pub db_path: PathBuf,
}

impl AppDirs {
    /// Directories under the per-user local data dir (Windows: `%LOCALAPPDATA%\GtavModManager\data`).
    pub fn from_system() -> Result<Self> {
        let pd = directories::ProjectDirs::from("", "", "GtavModManager")
            .ok_or(Error::DataDirUnavailable)?;
        Ok(Self::at(pd.data_local_dir()))
    }

    /// Directories rooted at `root` (for tests).
    pub fn at(root: &Path) -> Self {
        Self {
            data_dir: root.to_path_buf(),
            vault_dir: root.join("vault"),
            logs_dir: root.join("logs"),
            db_path: root.join("gmm.sqlite3"),
        }
    }

    /// Creates the data, vault and logs directories if missing.
    pub fn ensure_created(&self, fs: &dyn Fs) -> Result<()> {
        fs.create_dir_all(&self.data_dir)?;
        fs.create_dir_all(&self.vault_dir)?;
        fs.create_dir_all(&self.logs_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::std_fs::StdFs;

    #[test]
    fn at_builds_paths() {
        let d = AppDirs::at(Path::new("/r"));
        assert_eq!(d.data_dir, Path::new("/r"));
        assert_eq!(d.vault_dir, Path::new("/r/vault"));
        assert_eq!(d.logs_dir, Path::new("/r/logs"));
        assert_eq!(d.db_path, Path::new("/r/gmm.sqlite3"));
    }

    #[test]
    fn ensure_created_makes_dirs_and_is_idempotent() {
        let t = tempfile::TempDir::new().unwrap();
        let d = AppDirs::at(&t.path().join("app"));
        d.ensure_created(&StdFs).unwrap();
        d.ensure_created(&StdFs).unwrap();
        assert!(d.vault_dir.is_dir() && d.logs_dir.is_dir());
    }

    #[test]
    fn from_system_ends_with_app_name() {
        if let Ok(d) = AppDirs::from_system() {
            assert!(
                d.data_dir
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("gtavmodmanager")
            );
        }
    }
}
