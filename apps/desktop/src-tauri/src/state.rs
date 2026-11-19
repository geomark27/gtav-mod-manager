//! Managed application state.

use std::sync::Arc;

use gmm_app::{App, AppError, LogGuard};

/// State shared with every command. Holds the init result so a failed start
/// is reported to the UI instead of panicking.
pub struct AppState {
    /// The initialized application, or the error that prevented it.
    pub app: Result<Arc<App>, AppError>,
    /// Keeps the log writer alive for the life of the process.
    pub _log_guard: Option<LogGuard>,
}

impl AppState {
    /// Returns the application or a clone of the init error.
    pub fn app(&self) -> Result<Arc<App>, AppError> {
        self.app.as_ref().map(Arc::clone).map_err(Clone::clone)
    }
}
