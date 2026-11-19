//! Error type for the store crate.

use std::path::PathBuf;

/// Errors produced by `gmm-store`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An error reported by SQLite.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The database was written by a newer version of the application.
    #[error("database schema version {found} is newer than the supported version {supported}")]
    SchemaTooNew {
        /// Version found in the database.
        found: u32,
        /// Highest version this build supports.
        supported: u32,
    },
    /// SQLite did not accept a required pragma value.
    #[error("sqlite rejected pragma {pragma}: got {value}")]
    PragmaRejected {
        /// Pragma name.
        pragma: &'static str,
        /// Value SQLite reported instead.
        value: String,
    },
    /// The directory that should contain the database file does not exist.
    #[error("parent directory of the database does not exist: {path}")]
    ParentDirMissing {
        /// The missing directory.
        path: PathBuf,
    },
    /// A UNIQUE constraint was violated.
    #[error("{what} already exists")]
    Conflict {
        /// What collided (e.g. `installation root`).
        what: &'static str,
    },
    /// A stored value could not be parsed back into its domain type.
    #[error("corrupt value in {table}.{column}: {value:?}")]
    Corrupt {
        /// Table name.
        table: &'static str,
        /// Column name.
        column: &'static str,
        /// The unparsable stored value.
        value: String,
    },
    /// The requested row does not exist.
    #[error("{what} not found")]
    NotFound {
        /// What was looked up.
        what: &'static str,
    },
    /// The stored state does not allow the requested operation (e.g. an illegal transition).
    #[error("invalid state: {what}")]
    InvalidState {
        /// Description of the violated expectation.
        what: String,
    },
}

/// Result alias for the store crate.
pub type Result<T> = std::result::Result<T, Error>;
