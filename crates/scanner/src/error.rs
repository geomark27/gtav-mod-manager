//! Error type of the scanner crate.

/// Errors produced by catalog loading and file parsing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A catalog file is malformed or inconsistent.
    #[error("invalid catalog: {0}")]
    Catalog(String),
    /// A Valve KeyValues (VDF) document is malformed.
    #[error("invalid VDF at line {line}: {message}")]
    Vdf {
        /// 1-based line of the problem.
        line: usize,
        /// What went wrong.
        message: String,
    },
    /// A domain value (for example a relative path) was rejected.
    #[error(transparent)]
    Domain(#[from] gmm_domain::Error),
    /// A file-system or hashing operation failed.
    #[error(transparent)]
    Fs(gmm_platform::Error),
    /// A catalog glob pattern is invalid.
    #[error("invalid glob {pattern:?}: {reason}")]
    Glob {
        /// Offending pattern.
        pattern: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// The operation was cancelled through its progress callback.
    #[error("operation cancelled")]
    Cancelled,
}

impl From<gmm_platform::Error> for Error {
    fn from(e: gmm_platform::Error) -> Self {
        match e {
            gmm_platform::Error::Cancelled => Error::Cancelled,
            other => Error::Fs(other),
        }
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
