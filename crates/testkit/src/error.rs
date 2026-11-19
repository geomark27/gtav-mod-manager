//! Error type of the testkit crate.

use std::path::PathBuf;

/// Errors produced while building fake installations or taking snapshots.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O operation failed.
    #[error("I/O error during {op} on {path}: {source}")]
    Io {
        /// Operation name.
        op: &'static str,
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A relative path was rejected by `RelPath::parse`.
    #[error("invalid relative path: {0}")]
    InvalidPath(#[from] gmm_domain::Error),
    /// A symbolic link was found while taking a snapshot.
    #[error("symbolic link found in snapshot: {0}")]
    SymlinkInSnapshot(String),
}

impl Error {
    pub(crate) fn io(op: &'static str, path: &std::path::Path, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}

/// Result alias of this crate.
pub type Result<T> = std::result::Result<T, Error>;
