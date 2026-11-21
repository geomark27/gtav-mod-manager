//! Single-instance lock backed by an exclusive file lock.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use crate::error::{AppError, ErrorCode};

/// Holds `<data_dir>/gmm.lock` exclusively for as long as it lives.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Takes the lock, failing with `ALREADY_RUNNING` if another process holds it.
    pub fn acquire(data_dir: &Path) -> Result<Self, AppError> {
        let path = data_dir.join("gmm.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| {
                AppError::new(ErrorCode::Io, "Could not open the application lock file.")
                    .with_details(format!("{}: {e}", path.display()))
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(AppError::new(
                ErrorCode::AlreadyRunning,
                "ModSwitch is already running.",
            )
            .with_hint("Close the other window of ModSwitch.")),
            Err(TryLockError::Error(e)) => Err(AppError::new(
                ErrorCode::Io,
                "Could not lock the application lock file.",
            )
            .with_details(format!("{}: {e}", path.display()))),
        }
    }
}
