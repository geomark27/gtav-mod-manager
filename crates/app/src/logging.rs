//! Structured logging: JSON lines to a daily-rolling file plus a compact console layer.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::error::{AppError, ErrorCode};

/// Keeps the non-blocking log writer alive; drop it to flush.
#[derive(Debug)]
pub struct LogGuard {
    _worker: WorkerGuard,
}

/// Installs the global subscriber. Returns an error (never panics) if one is already set.
pub fn init_logging(logs_dir: &Path) -> Result<LogGuard, AppError> {
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("gmm")
        .filename_suffix("jsonl")
        .max_log_files(14)
        .build(logs_dir)
        .map_err(|e| {
            AppError::new(ErrorCode::Io, "Could not open the log files.")
                .with_details(format!("{}: {e}", logs_dir.display()))
        })?;
    let (writer, worker) = tracing_appender::non_blocking(appender);
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = fmt::layer()
        .json()
        .with_ansi(false)
        .with_writer(writer)
        .with_filter(filter());
    let console_layer = fmt::layer().compact().with_filter(filter());

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .try_init()
        .map_err(|e| {
            AppError::new(ErrorCode::Internal, "Logging was already initialized.")
                .with_details(e.to_string())
        })?;
    Ok(LogGuard { _worker: worker })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The global subscriber can be set once per process, so a single test covers both calls.
    #[test]
    fn init_once_then_second_call_errors_and_file_is_created() {
        let dir = tempfile::TempDir::new().unwrap();
        let guard = init_logging(dir.path()).unwrap();
        tracing::info!("hello");
        let second = init_logging(dir.path()).unwrap_err();
        assert_eq!(second.code, ErrorCode::Internal);
        drop(guard);
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names
                .iter()
                .any(|n| n.starts_with("gmm.") && n.ends_with(".jsonl")),
            "{names:?}"
        );
    }
}
