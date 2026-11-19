//! Baseline capture: path, size and SHA-256 of every original-looking regular file.

use crate::error::{Error, Result};
use crate::scan::ScanProgress;
use crate::walk::walk;
use gmm_domain::{RelPath, Sha256, parse_artifact};
use gmm_platform::{Fs, sha256_reader};
use std::path::Path;

/// Prefix of the engine's write-probe files (never part of a baseline).
pub(crate) const PROBE_PREFIX: &str = ".gmm-probe-";

/// One captured baseline entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineEntryData {
    /// Path relative to the installation root.
    pub rel_path: RelPath,
    /// Size in bytes.
    pub size: u64,
    /// Content hash.
    pub sha256: Sha256,
}

/// True for engine artifacts and probe leftovers.
pub(crate) fn is_engine_name(file_name: &str) -> bool {
    parse_artifact(file_name).is_some() || file_name.to_lowercase().starts_with(PROBE_PREFIX)
}

/// Hashes every regular file under `root` except engine artifacts, probe files and reparse
/// points. `progress` returning `false` cancels with [`Error::Cancelled`].
pub fn capture_baseline(
    fs: &dyn Fs,
    root: &Path,
    progress: &mut dyn FnMut(&ScanProgress) -> bool,
) -> Result<Vec<BaselineEntryData>> {
    let mut out = Vec::new();
    let mut state = ScanProgress {
        files_seen: 0,
        bytes_hashed: 0,
        current: None,
    };
    let mut skipped = 0u64;
    walk(fs, root, &mut skipped, &mut |entry| {
        if entry.meta.is_reparse_point || is_engine_name(entry.rel.file_name()) {
            return Ok(());
        }
        state.files_seen += 1;
        state.current = Some(entry.rel.as_str().to_owned());
        if !progress(&state) {
            return Err(Error::Cancelled);
        }
        let base = state.bytes_hashed;
        let mut reader = fs.open_read(&entry.abs)?;
        let mut last = 0u64;
        let hex = sha256_reader(&mut reader, &mut |n| {
            last = n;
            state.bytes_hashed = base + n;
            progress(&state)
        })?;
        state.bytes_hashed = base + last;
        out.push(BaselineEntryData {
            rel_path: entry.rel,
            size: entry.meta.len,
            sha256: Sha256::parse(&hex)?,
        });
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmm_domain::{Edition, TransactionId, tmp_name};
    use gmm_platform::{StdFs, sha256_bytes};
    use gmm_testkit::{FakeInstall, Snapshot};

    fn install() -> FakeInstall {
        FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .file("mods/a.txt", b"hello".to_vec())
            .build()
            .unwrap()
    }

    #[test]
    fn hashes_match_content_and_artifacts_are_skipped() {
        let i = install();
        i.write(&tmp_name("x.asi", TransactionId::new()), b"t")
            .unwrap();
        i.write(".gmm-probe-1234", b"p").unwrap();
        let before = Snapshot::of(i.root()).unwrap();
        let entries = capture_baseline(&StdFs, i.root(), &mut |_| true).unwrap();
        assert_eq!(before, Snapshot::of(i.root()).unwrap());
        assert_eq!(entries.len(), 7);
        for e in &entries {
            let data = i.read(e.rel_path.as_str()).unwrap();
            assert_eq!(e.sha256.as_str(), sha256_bytes(&data));
            assert_eq!(e.size, data.len() as u64);
            assert!(!is_engine_name(e.rel_path.file_name()));
        }
        assert!(entries.iter().any(|e| e.rel_path.as_str() == "mods/a.txt"));
    }

    #[test]
    fn cancellable_and_reports_progress() {
        let i = install();
        let mut calls = 0;
        let r = capture_baseline(&StdFs, i.root(), &mut |_| {
            calls += 1;
            calls < 3
        });
        assert!(matches!(r, Err(Error::Cancelled)));
        let mut last = 0;
        capture_baseline(&StdFs, i.root(), &mut |p| {
            last = p.files_seen;
            true
        })
        .unwrap();
        assert_eq!(last, 7);
    }

    #[cfg(unix)]
    #[test]
    fn reparse_points_are_skipped() {
        let i = install();
        std::os::unix::fs::symlink(i.path("common.rpf"), i.path("link.rpf")).unwrap();
        let entries = capture_baseline(&StdFs, i.root(), &mut |_| true).unwrap();
        assert!(entries.iter().all(|e| e.rel_path.as_str() != "link.rpf"));
        assert_eq!(entries.len(), 7);
    }
}
