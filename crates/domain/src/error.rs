//! Error type of the domain crate.

/// Errors produced by domain value objects and state machines.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A relative path was rejected by validation.
    #[error("invalid relative path {path:?}: {reason}")]
    InvalidRelPath {
        /// The offending input.
        path: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// A SHA-256 string was not 64 hex characters.
    #[error("invalid SHA-256 value {0:?}")]
    InvalidSha256(String),
    /// A persisted enum value was not recognised.
    #[error("invalid {kind} value {value:?}")]
    InvalidEnumValue {
        /// Name of the enum.
        kind: &'static str,
        /// The offending input.
        value: String,
    },
    /// A state machine transition is not allowed.
    #[error("invalid {entity} transition {from} -> {to}")]
    InvalidTransition {
        /// Entity whose state machine rejected the move.
        entity: &'static str,
        /// Current state.
        from: &'static str,
        /// Requested state.
        to: &'static str,
    },
    /// A user-supplied name was rejected.
    #[error("invalid name: {reason}")]
    InvalidName {
        /// Why it was rejected.
        reason: &'static str,
    },
    /// An identifier string was not a valid hyphenated UUID.
    #[error("invalid {kind} id {value:?}")]
    InvalidId {
        /// Name of the id type.
        kind: &'static str,
        /// The offending input.
        value: String,
    },
}

/// Result alias for the domain crate.
pub type Result<T> = std::result::Result<T, Error>;
