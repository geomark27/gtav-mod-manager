//! Error type of the platform crate.

use crate::fs::FsOp;
use std::path::PathBuf;

/// Errors produced by file-system and hashing operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An OS-level I/O failure.
    #[error("{op} failed for {}: {source}", path.display())]
    Io {
        /// Operation name.
        op: &'static str,
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The destination already exists.
    #[error("already exists: {}", path.display())]
    AlreadyExists {
        /// Existing path.
        path: PathBuf,
    },
    /// A simulated process crash (fault injection).
    #[error("simulated crash during {op:?} on {}", path.display())]
    SimulatedCrash {
        /// Operation that "crashed".
        op: FsOp,
        /// Path involved.
        path: PathBuf,
    },
    /// A simulated operation failure (fault injection).
    #[error("simulated failure during {op:?} on {}", path.display())]
    SimulatedFailure {
        /// Operation that failed.
        op: FsOp,
        /// Path involved.
        path: PathBuf,
    },
    /// The operation was cancelled by the caller.
    #[error("operation cancelled")]
    Cancelled,
    /// The per-user data directory could not be determined.
    #[error("the application data directory is unavailable")]
    DataDirUnavailable,
}

impl Error {
    /// Returns true for a simulated crash.
    pub fn is_crash(&self) -> bool {
        matches!(self, Error::SimulatedCrash { .. })
    }

    pub(crate) fn io(op: &'static str, path: &std::path::Path, source: std::io::Error) -> Self {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            return Error::AlreadyExists {
                path: path.to_path_buf(),
            };
        }
        Error::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
