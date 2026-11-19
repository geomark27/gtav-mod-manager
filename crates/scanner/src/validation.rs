//! Structural validation of a game folder (not a DRM or integrity check).
//!
//! Validation never modifies the installation except for one write probe: an empty file
//! named `.gmm-probe-<uuid>` is created in the root and removed right away.

use crate::catalog::Catalogs;
use crate::os::OsProbe;
use gmm_domain::{Edition, InstallationStatus, RelPath};
use gmm_platform::{EntryKind, Fs};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A reason why a folder is not a healthy installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationProblem {
    /// The root does not exist (or cannot be inspected).
    RootNotFound,
    /// The root exists but is not a directory.
    NotADirectory,
    /// Neither edition executable is present.
    NoExecutable,
    /// Both edition executables are present.
    AmbiguousEdition,
    /// Some catalog files are missing; see [`ValidationReport::missing`].
    MissingRequiredFiles,
    /// The write probe failed.
    NotWritable,
}

/// Outcome of [`validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Folder that was validated.
    pub root: PathBuf,
    /// Detected edition (`None` when unknown or ambiguous).
    pub edition: Option<Edition>,
    /// Resulting status.
    pub status: InstallationStatus,
    /// Required files that are missing, sorted.
    pub missing: Vec<RelPath>,
    /// Executable version from the PE version resource, when available.
    pub exe_version: Option<String>,
    /// Result of the write probe. Only meaningful when the structure is complete; it is
    /// `false` whenever the probe was not run.
    pub writable: bool,
    /// Every problem found.
    pub problems: Vec<ValidationProblem>,
}

fn is_regular_file(fs: &dyn Fs, path: &Path) -> bool {
    matches!(fs.metadata(path), Ok(m) if m.kind == EntryKind::File)
}

fn report(root: &Path, status: InstallationStatus, problem: ValidationProblem) -> ValidationReport {
    ValidationReport {
        root: root.to_path_buf(),
        edition: None,
        status,
        missing: Vec::new(),
        exe_version: None,
        writable: false,
        problems: vec![problem],
    }
}

/// Runs the write probe. Returns `true` when a file could be created and removed.
fn write_probe(fs: &dyn Fs, root: &Path) -> bool {
    let name = format!(".gmm-probe-{}", uuid::Uuid::new_v4().simple());
    let probe = root.join(name);
    if fs.write_new(&probe, b"").is_err() {
        // A failed create may still have left a file behind (for example a failed fsync).
        if matches!(fs.exists(&probe), Ok(true))
            && let Err(e) = fs.remove_file(&probe)
        {
            tracing::error!(path = %probe.display(), error = %e, "write probe file could not be removed");
        }
        return false;
    }
    match fs.remove_file(&probe) {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(path = %probe.display(), error = %e, "write probe file could not be removed");
            false
        }
    }
}

/// Validates a folder against the catalogs.
pub fn validate(
    fs: &dyn Fs,
    os: &dyn OsProbe,
    catalogs: &Catalogs,
    root: &Path,
) -> ValidationReport {
    match fs.metadata(root) {
        Err(e) => {
            tracing::debug!(root = %root.display(), error = %e, "root cannot be inspected");
            return report(
                root,
                InstallationStatus::Missing,
                ValidationProblem::RootNotFound,
            );
        }
        Ok(m) if m.kind != EntryKind::Dir => {
            return report(
                root,
                InstallationStatus::Missing,
                ValidationProblem::NotADirectory,
            );
        }
        Ok(_) => {}
    }

    let present: Vec<Edition> = catalogs
        .iter()
        .filter(|c| is_regular_file(fs, &root.join(&c.executable)))
        .map(|c| c.edition)
        .collect();
    let edition = match present.as_slice() {
        [] => {
            return report(
                root,
                InstallationStatus::Missing,
                ValidationProblem::NoExecutable,
            );
        }
        [one] => *one,
        _ => {
            return report(
                root,
                InstallationStatus::Incomplete,
                ValidationProblem::AmbiguousEdition,
            );
        }
    };

    let catalog = catalogs.get(edition);
    let mut missing: Vec<RelPath> = catalog
        .required_files
        .iter()
        .filter(|rel| !is_regular_file(fs, &root.join(rel.as_str())))
        .cloned()
        .collect();
    missing.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let exe_version = os.file_version(&root.join(&catalog.executable));

    let mut out = ValidationReport {
        root: root.to_path_buf(),
        edition: Some(edition),
        status: InstallationStatus::Ok,
        missing,
        exe_version,
        writable: false,
        problems: Vec::new(),
    };
    if !out.missing.is_empty() {
        out.status = InstallationStatus::Incomplete;
        out.problems.push(ValidationProblem::MissingRequiredFiles);
        return out;
    }
    out.writable = write_probe(fs, root);
    if !out.writable {
        out.status = InstallationStatus::NotWritable;
        out.problems.push(ValidationProblem::NotWritable);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::testing::FakeOs;
    use gmm_platform::{FaultMode, FaultPlan, FaultyFs, FsOp, OpMatch, StdFs};
    use gmm_testkit::{FakeInstall, Snapshot};

    fn catalogs() -> Catalogs {
        Catalogs::load().unwrap()
    }

    fn run(root: &Path) -> ValidationReport {
        validate(&StdFs::new(), &FakeOs::new(), &catalogs(), root)
    }

    #[test]
    fn legacy_ok() {
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .build()
            .unwrap();
        let before = Snapshot::of(inst.root()).unwrap();
        let os = FakeOs::new().version(inst.root().join("GTA5.exe"), "1.0.3179.0");
        let r = validate(&StdFs::new(), &os, &catalogs(), inst.root());
        assert_eq!(r.status, InstallationStatus::Ok);
        assert_eq!(r.edition, Some(Edition::Legacy));
        assert!(r.missing.is_empty() && r.problems.is_empty() && r.writable);
        assert_eq!(r.exe_version.as_deref(), Some("1.0.3179.0"));
        // The probe leaves no trace.
        assert!(before.diff(&Snapshot::of(inst.root()).unwrap()).is_empty());
    }

    #[test]
    fn enhanced_ok_without_version() {
        let inst = FakeInstall::builder(Edition::Enhanced)
            .with_base_files()
            .build()
            .unwrap();
        let r = run(inst.root());
        assert_eq!(r.status, InstallationStatus::Ok);
        assert_eq!(r.edition, Some(Edition::Enhanced));
        assert_eq!(r.exe_version, None);
    }

    #[test]
    fn missing_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let r = run(&tmp.path().join("nope"));
        assert_eq!(r.status, InstallationStatus::Missing);
        assert_eq!(r.problems, vec![ValidationProblem::RootNotFound]);
        assert_eq!(r.edition, None);
    }

    #[test]
    fn file_instead_of_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let f = tmp.path().join("file.txt");
        std::fs::write(&f, b"x").unwrap();
        let r = run(&f);
        assert_eq!(r.status, InstallationStatus::Missing);
        assert_eq!(r.problems, vec![ValidationProblem::NotADirectory]);
    }

    #[test]
    fn no_executable() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("common.rpf"), b"x").unwrap();
        let r = run(tmp.path());
        assert_eq!(r.status, InstallationStatus::Missing);
        assert_eq!(r.problems, vec![ValidationProblem::NoExecutable]);
    }

    #[test]
    fn executable_that_is_a_directory_does_not_count() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("GTA5.exe")).unwrap();
        let r = run(tmp.path());
        assert_eq!(r.problems, vec![ValidationProblem::NoExecutable]);
    }

    #[test]
    fn both_executables_are_ambiguous() {
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .file("GTA5_Enhanced.exe", b"x".to_vec())
            .build()
            .unwrap();
        let r = run(inst.root());
        assert_eq!(r.status, InstallationStatus::Incomplete);
        assert_eq!(r.edition, None);
        assert_eq!(r.problems, vec![ValidationProblem::AmbiguousEdition]);
    }

    #[test]
    fn missing_required_files_are_listed_sorted() {
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .without("x64a.rpf")
            .without("update/update.rpf")
            .without("PlayGTAV.exe")
            .build()
            .unwrap();
        let r = run(inst.root());
        assert_eq!(r.status, InstallationStatus::Incomplete);
        assert_eq!(r.edition, Some(Edition::Legacy));
        let names: Vec<_> = r.missing.iter().map(RelPath::as_str).collect();
        assert_eq!(names, vec!["PlayGTAV.exe", "update/update.rpf", "x64a.rpf"]);
        assert_eq!(r.problems, vec![ValidationProblem::MissingRequiredFiles]);
        assert!(!r.writable);
    }

    #[test]
    fn required_file_that_is_a_directory_counts_as_missing() {
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .without("common.rpf")
            .dir("common.rpf")
            .build()
            .unwrap();
        let r = run(inst.root());
        assert_eq!(r.missing.len(), 1);
        assert_eq!(r.missing[0].as_str(), "common.rpf");
    }

    #[cfg(unix)]
    #[test]
    fn read_only_root_is_not_writable() {
        use std::os::unix::fs::PermissionsExt;
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .build()
            .unwrap();
        let root = inst.root().to_path_buf();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root (uid 0) ignores permissions: skip the assertion if a file can still be created.
        let probe = root.join(".can-create");
        let can_create = std::fs::write(&probe, b"").is_ok();
        if can_create {
            std::fs::remove_file(&probe).unwrap();
        }
        let r = run(&root);
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !can_create {
            assert_eq!(r.status, InstallationStatus::NotWritable);
            assert_eq!(r.problems, vec![ValidationProblem::NotWritable]);
            assert!(!r.writable);
            assert_eq!(r.edition, Some(Edition::Legacy));
        }
    }

    #[test]
    fn probe_removal_failure_reports_not_writable() {
        let inst = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .build()
            .unwrap();
        let fs = FaultyFs::with_plan(
            StdFs::new(),
            FaultPlan {
                op: OpMatch::Op(FsOp::RemoveFile),
                nth: 1,
                mode: FaultMode::Error,
            },
        );
        let r = validate(&fs, &FakeOs::new(), &catalogs(), inst.root());
        assert_eq!(r.status, InstallationStatus::NotWritable);
        assert_eq!(r.problems, vec![ValidationProblem::NotWritable]);
    }
}
