//! `FaultyFs`: wraps an `Fs` and injects a failure or simulated crash at the N-th
//! matching mutating operation. The faulted operation is never performed.

use crate::error::{Error, Result};
use crate::fs::{DirEntryInfo, FileMeta, Fs, FsOp};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Which operations a plan counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpMatch {
    /// Any mutating operation.
    Any,
    /// Only this operation kind.
    Op(FsOp),
}

/// What happens when the plan fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultMode {
    /// Return `Error::SimulatedFailure`.
    Error,
    /// Return `Error::SimulatedCrash`.
    Crash,
}

/// A fault to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultPlan {
    /// Operations to count.
    pub op: OpMatch,
    /// 1-based index among matching operations.
    pub nth: usize,
    /// Failure mode.
    pub mode: FaultMode,
}

#[derive(Debug, Default)]
struct State {
    log: Vec<(FsOp, PathBuf)>,
    matched: usize,
    fired: bool,
}

/// Fault-injecting file system wrapper.
pub struct FaultyFs<F: Fs> {
    inner: F,
    plan: Option<FaultPlan>,
    state: Mutex<State>,
}

impl<F: Fs> FaultyFs<F> {
    /// Wraps `inner` with no fault planned.
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            plan: None,
            state: Mutex::new(State::default()),
        }
    }

    /// Wraps `inner` with a fault plan.
    pub fn with_plan(inner: F, plan: FaultPlan) -> Self {
        Self {
            inner,
            plan: Some(plan),
            state: Mutex::new(State::default()),
        }
    }

    /// Number of mutating operations attempted so far (including the faulted one).
    pub fn ops_count(&self) -> usize {
        self.lock().log.len()
    }

    /// Log of mutating operations attempted so far.
    pub fn log(&self) -> Vec<(FsOp, PathBuf)> {
        self.lock().log.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Records the operation; returns an error if the plan fires on it.
    fn gate(&self, op: FsOp, path: &Path) -> Result<()> {
        let mut st = self.lock();
        st.log.push((op, path.to_path_buf()));
        let Some(plan) = self.plan else { return Ok(()) };
        if st.fired || !(plan.op == OpMatch::Any || plan.op == OpMatch::Op(op)) {
            return Ok(());
        }
        st.matched += 1;
        if st.matched != plan.nth {
            return Ok(());
        }
        st.fired = true;
        let path = path.to_path_buf();
        Err(match plan.mode {
            FaultMode::Error => Error::SimulatedFailure { op, path },
            FaultMode::Crash => Error::SimulatedCrash { op, path },
        })
    }
}

impl<F: Fs> Fs for FaultyFs<F> {
    fn metadata(&self, path: &Path) -> Result<FileMeta> {
        self.inner.metadata(path)
    }
    fn exists(&self, path: &Path) -> Result<bool> {
        self.inner.exists(path)
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>> {
        self.inner.read_dir(path)
    }
    fn open_read(&self, path: &Path) -> Result<Box<dyn std::io::Read + Send>> {
        self.inner.open_read(path)
    }
    fn create_dir(&self, path: &Path) -> Result<()> {
        self.gate(FsOp::CreateDir, path)?;
        self.inner.create_dir(path)
    }
    fn create_dir_all(&self, path: &Path) -> Result<()> {
        self.gate(FsOp::CreateDir, path)?;
        self.inner.create_dir_all(path)
    }
    fn copy_new(&self, from: &Path, to: &Path) -> Result<u64> {
        self.gate(FsOp::Copy, to)?;
        self.inner.copy_new(from, to)
    }
    fn write_new(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.gate(FsOp::Write, path)?;
        self.inner.write_new(path, data)
    }
    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<()> {
        self.gate(FsOp::Rename, from)?;
        self.inner.rename_no_replace(from, to)
    }
    fn remove_file(&self, path: &Path) -> Result<()> {
        self.gate(FsOp::RemoveFile, path)?;
        self.inner.remove_file(path)
    }
    fn remove_dir(&self, path: &Path) -> Result<()> {
        self.gate(FsOp::RemoveDir, path)?;
        self.inner.remove_dir(path)
    }
    fn set_readonly(&self, path: &Path, readonly: bool) -> Result<()> {
        self.gate(FsOp::SetReadonly, path)?;
        self.inner.set_readonly(path, readonly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::std_fs::StdFs;
    use tempfile::TempDir;

    fn plan(op: OpMatch, nth: usize, mode: FaultMode) -> FaultPlan {
        FaultPlan { op, nth, mode }
    }

    #[test]
    fn error_at_second_copy() {
        let d = TempDir::new().unwrap();
        let src = d.path().join("src");
        std::fs::write(&src, b"x").unwrap();
        let fs = FaultyFs::with_plan(StdFs, plan(OpMatch::Op(FsOp::Copy), 2, FaultMode::Error));
        let (d1, d2, d3) = (
            d.path().join("d1"),
            d.path().join("d2"),
            d.path().join("d3"),
        );
        fs.copy_new(&src, &d1).unwrap();
        let e = fs.copy_new(&src, &d2).unwrap_err();
        assert!(matches!(e, Error::SimulatedFailure { op: FsOp::Copy, .. }));
        assert!(!e.is_crash());
        assert!(d1.exists());
        assert!(!d2.exists());
        // fires once
        fs.copy_new(&src, &d3).unwrap();
        assert!(d3.exists());
    }

    #[test]
    fn crash_is_crash_and_skips_inner() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("f");
        let fs = FaultyFs::with_plan(StdFs, plan(OpMatch::Any, 1, FaultMode::Crash));
        let e = fs.write_new(&p, b"x").unwrap_err();
        assert!(e.is_crash());
        assert!(!p.exists());
    }

    #[test]
    fn reads_are_never_counted() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("f");
        std::fs::write(&p, b"x").unwrap();
        let fs = FaultyFs::with_plan(StdFs, plan(OpMatch::Any, 1, FaultMode::Error));
        fs.metadata(&p).unwrap();
        fs.exists(&p).unwrap();
        fs.read_dir(d.path()).unwrap();
        fs.open_read(&p).unwrap();
        assert_eq!(fs.ops_count(), 0);
    }

    #[test]
    fn any_matcher_counts_all_kinds_and_logs() {
        let d = TempDir::new().unwrap();
        let (sub, f) = (d.path().join("sub"), d.path().join("sub/f"));
        let fs = FaultyFs::with_plan(StdFs, plan(OpMatch::Any, 3, FaultMode::Error));
        fs.create_dir(&sub).unwrap();
        fs.write_new(&f, b"x").unwrap();
        assert!(fs.remove_file(&f).is_err());
        assert!(f.exists());
        fs.remove_file(&f).unwrap();
        assert_eq!(fs.ops_count(), 4);
        let log = fs.log();
        assert_eq!(log[0], (FsOp::CreateDir, sub));
        assert_eq!(log[1].0, FsOp::Write);
        assert_eq!(log[2], (FsOp::RemoveFile, f.clone()));
        assert_eq!(log[3].0, FsOp::RemoveFile);
    }

    #[test]
    fn non_matching_ops_do_not_advance_counter() {
        let d = TempDir::new().unwrap();
        let fs = FaultyFs::with_plan(StdFs, plan(OpMatch::Op(FsOp::Write), 1, FaultMode::Error));
        fs.create_dir(&d.path().join("a")).unwrap();
        assert!(fs.write_new(&d.path().join("w"), b"x").is_err());
    }

    #[test]
    fn no_plan_passes_through() {
        let d = TempDir::new().unwrap();
        let fs = FaultyFs::new(StdFs);
        fs.write_new(&d.path().join("w"), b"x").unwrap();
        assert_eq!(fs.ops_count(), 1);
    }
}
