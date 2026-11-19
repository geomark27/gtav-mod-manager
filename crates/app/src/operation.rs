//! The single operation slot, cancellation and progress reporting.
//!
//! Only one heavy operation (scan, baseline capture) runs at a time. Starting a second one
//! fails with `OPERATION_IN_PROGRESS`; `cancel_operation` asks the running one to stop.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};

/// Kind of a long-running operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OperationKind {
    /// Inventory scan.
    Scan,
    /// Baseline capture.
    Baseline,
    /// Adoption of game files as a mod.
    Adopt,
    /// Enable, disable or category switch of mods (one journaled transaction).
    Transaction,
    /// Staging or confirming the import of a mod from a ZIP archive.
    Import,
}

/// Progress of the running operation, pushed to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProgressDto {
    /// Identifier of the operation this event belongs to.
    pub operation_id: String,
    /// Operation kind.
    pub kind: OperationKind,
    /// Files visited so far.
    #[ts(type = "number")]
    pub files_seen: u64,
    /// Bytes hashed so far.
    #[ts(type = "number")]
    pub bytes_hashed: u64,
    /// File being processed, when known.
    pub current: Option<String>,
    /// True on the final event of the operation (success, failure or cancellation).
    pub done: bool,
}

/// Whether a heavy operation currently holds the single operation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OperationStatusDto {
    /// True while an operation is running.
    pub busy: bool,
    /// Kind of the running operation, or `null` when idle.
    pub kind: Option<OperationKind>,
}

/// Receiver of progress events.
pub trait ProgressSink: Send + Sync {
    /// Called for every progress snapshot, from the thread running the operation.
    fn emit(&self, p: &ProgressDto);
}

/// A sink that discards every event.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl ProgressSink for NullSink {
    fn emit(&self, _p: &ProgressDto) {}
}

/// Wraps a sink so that at most one event per `interval` reaches it; the final `done` event
/// always passes.
#[derive(Debug)]
pub struct ThrottledSink<S> {
    inner: S,
    interval: Duration,
    last: Mutex<Option<Instant>>,
}

impl<S: ProgressSink> ThrottledSink<S> {
    /// Creates a throttled sink.
    pub fn new(inner: S, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            last: Mutex::new(None),
        }
    }

    fn admit(&self) -> bool {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        match *last {
            Some(t) if now.duration_since(t) < self.interval => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

impl<S: ProgressSink> ProgressSink for ThrottledSink<S> {
    fn emit(&self, p: &ProgressDto) {
        if p.done || self.admit() {
            self.inner.emit(p);
        }
    }
}

/// The operation currently holding the slot.
#[derive(Debug)]
pub(crate) struct RunningOp {
    id: String,
    kind: OperationKind,
    cancel: Arc<AtomicBool>,
}

/// A started operation. Dropping it frees the slot.
pub(crate) struct Operation<'a> {
    app: &'a App,
    id: String,
    kind: OperationKind,
    sink: &'a dyn ProgressSink,
    cancel: Arc<AtomicBool>,
    files_seen: Cell<u64>,
    bytes_hashed: Cell<u64>,
}

impl Operation<'_> {
    /// Records and emits a snapshot; returns `false` when the operation must stop.
    pub(crate) fn report(&self, files_seen: u64, bytes_hashed: u64, current: Option<&str>) -> bool {
        self.files_seen.set(files_seen);
        self.bytes_hashed.set(bytes_hashed);
        self.sink.emit(&ProgressDto {
            operation_id: self.id.clone(),
            kind: self.kind,
            files_seen,
            bytes_hashed,
            current: current.map(str::to_owned),
            done: false,
        });
        !self.cancel.load(Ordering::SeqCst)
    }

    /// True once the user asked to cancel the operation.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Files seen at the last snapshot.
    pub(crate) fn files_seen(&self) -> u64 {
        self.files_seen.get()
    }

    /// Frees the slot, then emits the final `done` event.
    fn finish(self) {
        let dto = ProgressDto {
            operation_id: self.id.clone(),
            kind: self.kind,
            files_seen: self.files_seen.get(),
            bytes_hashed: self.bytes_hashed.get(),
            current: None,
            done: true,
        };
        let sink = self.sink;
        drop(self);
        sink.emit(&dto);
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        let mut slot = self.app.op.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_some_and(|r| r.id == self.id) {
            *slot = None;
        }
    }
}

impl App {
    fn begin_operation<'a>(
        &'a self,
        kind: OperationKind,
        sink: &'a dyn ProgressSink,
    ) -> Result<Operation<'a>, AppError> {
        let mut slot = self.op.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_some() {
            return Err(AppError::new(
                ErrorCode::OperationInProgress,
                "Another operation is running. Wait for it to finish or cancel it.",
            ));
        }
        let id = uuid::Uuid::now_v7().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(RunningOp {
            id: id.clone(),
            kind,
            cancel: Arc::clone(&cancel),
        });
        Ok(Operation {
            app: self,
            id,
            kind,
            sink,
            cancel,
            files_seen: Cell::new(0),
            bytes_hashed: Cell::new(0),
        })
    }

    /// Runs `f` while holding the operation slot, then emits the final `done` event.
    pub(crate) fn run_operation<T>(
        &self,
        kind: OperationKind,
        sink: &dyn ProgressSink,
        f: impl FnOnce(&Operation<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let op = self.begin_operation(kind, sink)?;
        let result = f(&op);
        op.finish();
        result
    }

    /// Reports whether an operation holds the slot and, if so, its kind.
    ///
    /// The UI uses this to keep an update from restarting the app while a scan,
    /// transaction or import is running.
    pub fn operation_status(&self) -> OperationStatusDto {
        let slot = self.op.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.as_ref() {
            Some(running) => OperationStatusDto {
                busy: true,
                kind: Some(running.kind),
            },
            None => OperationStatusDto {
                busy: false,
                kind: None,
            },
        }
    }

    /// Asks the running operation to stop. Does nothing when idle.
    pub fn cancel_operation(&self) {
        let slot = self.op.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(running) = slot.as_ref() {
            running.cancel.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct Count(AtomicUsize);
    impl ProgressSink for &Count {
        fn emit(&self, _p: &ProgressDto) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn dto(done: bool) -> ProgressDto {
        ProgressDto {
            operation_id: "o".into(),
            kind: OperationKind::Scan,
            files_seen: 1,
            bytes_hashed: 2,
            current: None,
            done,
        }
    }

    #[test]
    fn throttle_drops_bursts_but_passes_done() {
        let c = Count::default();
        let s = ThrottledSink::new(&c, Duration::from_secs(3600));
        for _ in 0..10 {
            s.emit(&dto(false));
        }
        assert_eq!(c.0.load(Ordering::SeqCst), 1);
        s.emit(&dto(true));
        assert_eq!(c.0.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn zero_interval_passes_everything() {
        let c = Count::default();
        let s = ThrottledSink::new(&c, Duration::ZERO);
        for _ in 0..5 {
            s.emit(&dto(false));
        }
        assert_eq!(c.0.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn progress_json_shape() {
        let v = serde_json::to_value(dto(true)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"operationId":"o","kind":"scan","filesSeen":1,"bytesHashed":2,
                "current":null,"done":true})
        );
        let v = serde_json::to_string(&OperationKind::Baseline).unwrap();
        assert_eq!(v, "\"baseline\"");
    }

    #[test]
    fn operation_status_reports_idle_then_busy() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let app = App::init_with(
            crate::AppDirs::at(&tmp.path().join("data")),
            crate::AppOptions::default(),
        )
        .expect("init");

        let idle = app.operation_status();
        assert!(!idle.busy);
        assert_eq!(idle.kind, None);

        let op = app
            .begin_operation(OperationKind::Scan, &NullSink)
            .expect("begin operation");
        let busy = app.operation_status();
        assert!(busy.busy);
        assert_eq!(busy.kind, Some(OperationKind::Scan));
        drop(op);

        let idle = app.operation_status();
        assert!(!idle.busy);
        assert_eq!(idle.kind, None);
    }

    #[test]
    fn operation_status_json_shape() {
        let v = serde_json::to_value(OperationStatusDto {
            busy: true,
            kind: Some(OperationKind::Baseline),
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"busy":true,"kind":"baseline"}));
        let v = serde_json::to_value(OperationStatusDto {
            busy: false,
            kind: None,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"busy":false,"kind":null}));
    }
}
