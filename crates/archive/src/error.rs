//! Error type of the archive crate.

/// Errors produced while staging an archive.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An entry name is not a safe relative path (zip-slip, absolute, drive letter, ...).
    #[error("unsafe path in archive: {name:?}")]
    UnsafePath {
        /// Offending entry name.
        name: String,
    },
    /// An entry is a symlink or another non-regular file.
    #[error("unsafe entry {name:?}: {reason}")]
    UnsafeEntry {
        /// Offending entry name.
        name: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// An entry is encrypted.
    #[error("encrypted entry not supported: {name:?}")]
    Encrypted {
        /// Offending entry name.
        name: String,
    },
    /// Two entries collide (same path ignoring case, or a file used as a folder).
    #[error("duplicate or conflicting entry: {name:?}")]
    DuplicateEntry {
        /// Offending entry name.
        name: String,
    },
    /// A size or count limit was exceeded.
    #[error("archive exceeds limit: {what} (limit {limit})")]
    TooLarge {
        /// Which limit.
        what: &'static str,
        /// The limit value.
        limit: u64,
    },
    /// The compression ratio of an entry is implausibly high.
    #[error("suspicious compression ratio in entry {name:?}")]
    SuspiciousCompression {
        /// Offending entry name.
        name: String,
    },
    /// The operation was cancelled by the caller.
    #[error("operation cancelled")]
    Cancelled,
    /// The ZIP container is malformed or unreadable.
    #[error("invalid zip archive: {0}")]
    Zip(String),
    /// A file-system failure.
    #[error(transparent)]
    Platform(#[from] gmm_platform::Error),
    /// A raw I/O failure.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias of the archive crate.
pub type Result<T> = std::result::Result<T, Error>;
