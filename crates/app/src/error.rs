//! Central error model serialized to the UI.

use serde::Serialize;
use ts_rs::TS;

/// Stable error codes shown to (and matched by) the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    /// The game of the installation, or the Rockstar launcher, is running.
    GameRunning,
    /// The installation folder is not writable.
    InstallNotWritable,
    /// Not enough free disk space.
    InsufficientSpace,
    /// Files changed outside the manager.
    DriftDetected,
    /// An interrupted transaction must be recovered first.
    RecoveryRequired,
    /// Ownership of a file is uncertain.
    OwnershipUncertain,
    /// A path escapes its root.
    PathOutsideRoot,
    /// A path already exists or conflicts.
    PathConflict,
    /// A hash did not match.
    HashMismatch,
    /// The requested item does not exist.
    NotFound,
    /// The input is invalid.
    InvalidInput,
    /// Another instance of the app is running.
    AlreadyRunning,
    /// A file-system failure.
    Io,
    /// A database failure.
    Database,
    /// An unexpected internal failure.
    Internal,
    /// Another long-running operation is already in progress.
    OperationInProgress,
    /// The operation was cancelled by the user.
    Cancelled,
    /// A dependency rule blocks the change (a required mod is off, or a dependent is on).
    DependencyBlocked,
}

/// Error returned by every command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[error("{code:?}: {message}")]
#[ts(export)]
pub struct AppError {
    /// Stable machine-readable code.
    pub code: ErrorCode,
    /// User-readable English message.
    pub message: String,
    /// Suggested next step, if any.
    pub hint: Option<String>,
    /// Technical text for logs and support.
    pub details: Option<String>,
}

impl AppError {
    /// Creates an error with a code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            details: None,
        }
    }

    /// Adds a hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds technical details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl From<gmm_store::Error> for AppError {
    fn from(e: gmm_store::Error) -> Self {
        let details = e.to_string();
        match e {
            gmm_store::Error::Conflict { .. } => {
                AppError::new(ErrorCode::InvalidInput, "This item already exists.")
            }
            gmm_store::Error::NotFound { what } => {
                AppError::new(ErrorCode::NotFound, format!("The {what} was not found."))
            }
            gmm_store::Error::InvalidState { what } => {
                AppError::new(ErrorCode::InvalidInput, sentence(&what))
            }
            _ => AppError::new(ErrorCode::Database, "The application database failed."),
        }
        .with_details(details)
    }
}

/// Capitalizes the first letter and ends the text with a period.
pub(crate) fn sentence(text: &str) -> String {
    let mut chars = text.trim().chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

impl From<gmm_engine::Error> for AppError {
    fn from(e: gmm_engine::Error) -> Self {
        use gmm_engine::Error as E;
        let details = e.to_string();
        let err = match e {
            E::PathConflict { path } => AppError::new(
                ErrorCode::PathConflict,
                format!("{path} is already used by another file or mod."),
            )
            .with_hint("Disable the other mod or move the file, then try again."),
            E::Drift { path } => AppError::new(
                ErrorCode::DriftDetected,
                format!("{path} was changed or removed outside the manager."),
            )
            .with_hint("Restore the mod from its stored copy, re-adopt its files or forget it."),
            E::HashMismatch { .. } => AppError::new(
                ErrorCode::HashMismatch,
                "A file does not match its expected content.",
            )
            .with_hint("Nothing was changed. Scan again and retry."),
            E::VaultCorrupt { .. } => AppError::new(
                ErrorCode::HashMismatch,
                "A stored copy of a mod file is missing or damaged.",
            )
            .with_hint(
                "Nothing was changed. Restore the file from the game folder if it is still there.",
            ),
            E::OwnershipUncertain { path } => AppError::new(
                ErrorCode::OwnershipUncertain,
                format!("The app cannot prove that it owns {path}, so it will not touch it."),
            ),
            E::PathOutsideRoot { path } => AppError::new(
                ErrorCode::PathOutsideRoot,
                format!("{path} resolves outside the game folder."),
            ),
            E::NeedsAttention { .. } => AppError::new(
                ErrorCode::RecoveryRequired,
                "An earlier operation could not be undone safely.",
            )
            .with_hint("Check the files listed in the details, then restart the app."),
            E::CommitIncomplete { .. } => AppError::new(
                ErrorCode::RecoveryRequired,
                "The file changes finished but the operation was not finalized.",
            )
            .with_hint("Restart the app to finish the operation."),
            E::InvalidState { what } | E::InvalidInput { what } => {
                AppError::new(ErrorCode::InvalidInput, sentence(&what))
            }
            E::NotFound { what } => AppError::new(
                ErrorCode::NotFound,
                sentence(&format!("{what} was not found")),
            ),
            E::DependencyDisabled { r#mod, dependency } => AppError::new(
                ErrorCode::DependencyBlocked,
                format!(
                    "\"{}\" requires \"{}\", which would not be enabled.",
                    r#mod, dependency
                ),
            )
            .with_hint("Enable the required mods too, or include dependencies."),
            E::DependentEnabled { r#mod, dependent } => AppError::new(
                ErrorCode::DependencyBlocked,
                format!(
                    "\"{}\" is required by \"{}\", which would stay enabled.",
                    r#mod, dependent
                ),
            )
            .with_hint("Disable the mods that depend on it too, or include dependencies."),
            E::Crashed => AppError::new(ErrorCode::Internal, "The operation was interrupted."),
            E::Fs(_) => AppError::new(ErrorCode::Io, "A file operation failed."),
            E::Store(inner) => return AppError::from(inner),
            E::Domain(inner) => return AppError::from(inner),
        };
        err.with_details(details)
    }
}

impl From<gmm_scanner::Error> for AppError {
    fn from(e: gmm_scanner::Error) -> Self {
        if matches!(e, gmm_scanner::Error::Cancelled) {
            return AppError::new(ErrorCode::Cancelled, "The operation was cancelled.");
        }
        AppError::new(ErrorCode::Internal, "Game detection failed unexpectedly.")
            .with_details(e.to_string())
    }
}

impl From<gmm_archive::Error> for AppError {
    fn from(e: gmm_archive::Error) -> Self {
        use gmm_archive::Error as A;
        let details = e.to_string();
        match e {
            A::Cancelled => AppError::new(ErrorCode::Cancelled, "The operation was cancelled."),
            A::Platform(inner) => return AppError::from(inner),
            A::Io(_) => AppError::new(ErrorCode::Io, "The archive could not be read."),
            A::Zip(_) => AppError::new(
                ErrorCode::InvalidInput,
                "The file is not a valid ZIP archive.",
            ),
            A::UnsafePath { .. } | A::UnsafeEntry { .. } => AppError::new(
                ErrorCode::InvalidInput,
                "The archive contains an unsafe path or file type and was not extracted.",
            ),
            A::Encrypted { .. } => AppError::new(
                ErrorCode::InvalidInput,
                "The archive is password-protected, which is not supported.",
            ),
            A::DuplicateEntry { .. } => AppError::new(
                ErrorCode::InvalidInput,
                "The archive contains files with conflicting names.",
            ),
            A::TooLarge { .. } => AppError::new(
                ErrorCode::InvalidInput,
                "The archive is larger than the allowed limits.",
            ),
            A::SuspiciousCompression { .. } => AppError::new(
                ErrorCode::InvalidInput,
                "The archive has an implausible compression ratio and was rejected.",
            ),
        }
        .with_details(details)
    }
}

impl From<gmm_platform::Error> for AppError {
    fn from(e: gmm_platform::Error) -> Self {
        match e {
            gmm_platform::Error::AlreadyExists { .. } => {
                AppError::new(ErrorCode::PathConflict, "The destination already exists.")
            }
            _ => AppError::new(ErrorCode::Io, "A file operation failed."),
        }
        .with_details(e.to_string())
    }
}

impl From<gmm_domain::Error> for AppError {
    fn from(e: gmm_domain::Error) -> Self {
        AppError::new(ErrorCode::InvalidInput, "The provided value is not valid.")
            .with_details(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn store_error_maps_to_database() {
        let e: AppError = gmm_store::Error::SchemaTooNew {
            found: 9,
            supported: 1,
        }
        .into();
        assert_eq!(e.code, ErrorCode::Database);
        assert!(e.details.unwrap().contains("newer"));
    }

    #[test]
    fn store_conflict_not_found_and_corrupt_map() {
        let e: AppError = gmm_store::Error::Conflict { what: "x" }.into();
        assert_eq!(e.code, ErrorCode::InvalidInput);
        let e: AppError = gmm_store::Error::NotFound {
            what: "installation",
        }
        .into();
        assert_eq!(e.code, ErrorCode::NotFound);
        assert_eq!(e.message, "The installation was not found.");
        let e: AppError = gmm_store::Error::Corrupt {
            table: "t",
            column: "c",
            value: "v".into(),
        }
        .into();
        assert_eq!(e.code, ErrorCode::Database);
    }

    #[test]
    fn scanner_error_maps_to_internal_with_details() {
        let e: AppError = gmm_scanner::Error::Catalog("bad".into()).into();
        assert_eq!(e.code, ErrorCode::Internal);
        assert!(e.details.unwrap().contains("bad"));
    }

    #[test]
    fn scanner_cancellation_maps_to_cancelled() {
        let e: AppError = gmm_scanner::Error::Cancelled.into();
        assert_eq!(e.code, ErrorCode::Cancelled);
        let v = serde_json::to_string(&ErrorCode::OperationInProgress).unwrap();
        assert_eq!(v, "\"OPERATION_IN_PROGRESS\"");
    }

    #[test]
    fn platform_errors_map() {
        let e: AppError = gmm_platform::Error::AlreadyExists {
            path: PathBuf::from("x"),
        }
        .into();
        assert_eq!(e.code, ErrorCode::PathConflict);
        let e: AppError = gmm_platform::Error::Cancelled.into();
        assert_eq!(e.code, ErrorCode::Io);
        assert!(e.details.is_some());
    }

    #[test]
    fn domain_error_maps_to_invalid_input() {
        let e: AppError = gmm_domain::Error::InvalidSha256("zz".into()).into();
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert!(e.details.unwrap().contains("zz"));
    }

    #[test]
    fn engine_errors_map_to_codes() {
        use gmm_engine::Error as E;
        let path = gmm_domain::RelPath::parse("scripts/a.asi").unwrap();
        let tx = gmm_domain::TransactionId::new();
        let sha = gmm_domain::Sha256::parse(&"a".repeat(64)).unwrap();
        let code = |e: E| AppError::from(e).code;
        assert_eq!(
            code(E::PathConflict { path: path.clone() }),
            ErrorCode::PathConflict
        );
        assert_eq!(
            code(E::Drift { path: path.clone() }),
            ErrorCode::DriftDetected
        );
        assert_eq!(
            code(E::HashMismatch { path: "x".into() }),
            ErrorCode::HashMismatch
        );
        assert_eq!(code(E::VaultCorrupt { sha }), ErrorCode::HashMismatch);
        assert_eq!(
            code(E::OwnershipUncertain { path: path.clone() }),
            ErrorCode::OwnershipUncertain
        );
        assert_eq!(
            code(E::PathOutsideRoot { path: path.clone() }),
            ErrorCode::PathOutsideRoot
        );
        assert_eq!(
            code(E::NeedsAttention {
                tx,
                reason: "r".into()
            }),
            ErrorCode::RecoveryRequired
        );
        let incomplete = AppError::from(E::CommitIncomplete { tx });
        assert_eq!(incomplete.code, ErrorCode::RecoveryRequired);
        assert_eq!(
            incomplete.hint.as_deref(),
            Some("Restart the app to finish the operation.")
        );
        assert_eq!(
            code(E::InvalidState { what: "x".into() }),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            code(E::InvalidInput { what: "x".into() }),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            code(E::NotFound { what: "mod".into() }),
            ErrorCode::NotFound
        );
        assert_eq!(code(E::Crashed), ErrorCode::Internal);
        let blocked = AppError::from(E::DependencyDisabled {
            r#mod: "A".into(),
            dependency: "B".into(),
        });
        assert_eq!(blocked.code, ErrorCode::DependencyBlocked);
        assert!(blocked.message.contains("\"A\"") && blocked.message.contains("\"B\""));
        let blocked = AppError::from(E::DependentEnabled {
            r#mod: "B".into(),
            dependent: "A".into(),
        });
        assert_eq!(blocked.code, ErrorCode::DependencyBlocked);
        assert!(blocked.message.contains("\"A\""));
        let conflict = AppError::from(E::PathConflict { path });
        assert!(conflict.message.contains("scripts/a.asi"));
        assert!(conflict.details.is_some());
    }

    #[test]
    fn store_invalid_state_maps_to_invalid_input() {
        let e: AppError = gmm_store::Error::InvalidState {
            what: "mod is gone".into(),
        }
        .into();
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert_eq!(e.message, "Mod is gone.");
    }

    #[test]
    fn archive_errors_map() {
        use gmm_archive::Error as A;
        let code = |e: A| AppError::from(e).code;
        assert_eq!(code(A::Cancelled), ErrorCode::Cancelled);
        assert_eq!(code(A::Zip("x".into())), ErrorCode::InvalidInput);
        assert_eq!(
            code(A::UnsafePath { name: "..".into() }),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            code(A::TooLarge {
                what: "w",
                limit: 1
            }),
            ErrorCode::InvalidInput
        );
        assert_eq!(code(A::Io(std::io::Error::other("x"))), ErrorCode::Io);
    }

    #[test]
    fn sentence_helper() {
        assert_eq!(sentence("a mod exists"), "A mod exists.");
        assert_eq!(sentence("done."), "Done.");
        assert_eq!(sentence(""), "");
    }

    #[test]
    fn json_shape_is_stable() {
        let e = AppError::new(ErrorCode::PathConflict, "m").with_details("d");
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"code":"PATH_CONFLICT","message":"m","hint":null,"details":"d"})
        );
    }

    #[test]
    fn codes_serialize_screaming_snake() {
        let v = serde_json::to_string(&ErrorCode::InstallNotWritable).unwrap();
        assert_eq!(v, "\"INSTALL_NOT_WRITABLE\"");
        let v = serde_json::to_string(&ErrorCode::AlreadyRunning).unwrap();
        assert_eq!(v, "\"ALREADY_RUNNING\"");
    }
}
