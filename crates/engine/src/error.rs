//! Error type of the engine crate.

use gmm_domain::{RelPath, Sha256, TransactionId};

/// Everything that can go wrong while planning, executing or recovering a transaction.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The target path is already occupied (by an unmanaged file or by another enabled mod).
    #[error("path conflict: {path} is already in use")]
    PathConflict {
        /// The conflicting installation-relative path.
        path: RelPath,
    },
    /// A managed file is missing or differs from the content the engine expects.
    #[error("drift detected: {path} does not match the expected content")]
    Drift {
        /// The drifted path.
        path: RelPath,
    },
    /// A copied file does not hash to the expected digest.
    #[error("hash mismatch for {path}")]
    HashMismatch {
        /// The file whose content was unexpected.
        path: String,
    },
    /// The engine cannot prove it owns the path (original file, reparse point, hard link...).
    #[error("ownership of {path} is uncertain")]
    OwnershipUncertain {
        /// The path that was refused.
        path: RelPath,
    },
    /// A resolved path would leave the installation root (or crosses a reparse point).
    #[error("path {path} escapes the installation root")]
    PathOutsideRoot {
        /// The offending path.
        path: RelPath,
    },
    /// A vault object is missing or its content does not match its name.
    #[error("vault object {sha} is missing or corrupt")]
    VaultCorrupt {
        /// Digest of the object.
        sha: Sha256,
    },
    /// An entity is not in a state that allows the operation.
    #[error("invalid state: {what}")]
    InvalidState {
        /// Human readable description.
        what: String,
    },
    /// The request itself is invalid.
    #[error("invalid input: {what}")]
    InvalidInput {
        /// Human readable description.
        what: String,
    },
    /// A referenced entity does not exist.
    #[error("{what} not found")]
    NotFound {
        /// What was not found.
        what: String,
    },
    /// A mod would be enabled while one of its dependencies stays disabled.
    #[error("mod {mod:?} requires {dependency:?}, which would not be enabled")]
    DependencyDisabled {
        /// Name of the mod being enabled.
        r#mod: String,
        /// Name of the dependency that would be left disabled.
        dependency: String,
    },
    /// A mod would be disabled while an enabled mod still depends on it.
    #[error("mod {mod:?} is required by {dependent:?}, which would stay enabled")]
    DependentEnabled {
        /// Name of the mod being disabled.
        r#mod: String,
        /// Name of the enabled dependent.
        dependent: String,
    },
    /// A rollback could not be proven safe; manual attention is required.
    #[error("transaction {tx} needs attention: {reason}")]
    NeedsAttention {
        /// The blocked transaction.
        tx: TransactionId,
        /// Why the undo stopped.
        reason: String,
    },
    /// File effects are complete but the commit was not finalized; recovery will finish it.
    #[error("transaction {tx} is committing but could not be finalized; recovery will finish it")]
    CommitIncomplete {
        /// The transaction left in `committing`.
        tx: TransactionId,
    },
    /// A simulated power loss: nothing else was done after it.
    #[error("simulated crash")]
    Crashed,
    /// File-system failure (never a simulated crash, see [`Error::Crashed`]).
    #[error("file system error: {0}")]
    Fs(gmm_platform::Error),
    /// Database failure.
    #[error(transparent)]
    Store(#[from] gmm_store::Error),
    /// Domain validation failure.
    #[error(transparent)]
    Domain(#[from] gmm_domain::Error),
}

impl From<gmm_platform::Error> for Error {
    fn from(e: gmm_platform::Error) -> Self {
        if e.is_crash() {
            Error::Crashed
        } else {
            Error::Fs(e)
        }
    }
}

impl Error {
    /// True for a simulated crash: callers must stop immediately.
    pub fn is_crash(&self) -> bool {
        matches!(self, Error::Crashed)
    }
}

/// Result alias of the engine crate.
pub type Result<T> = std::result::Result<T, Error>;
