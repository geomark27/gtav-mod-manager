//! Disk snapshots: path, size and SHA-256 of everything under a root.

use crate::error::{Error, Result};
use gmm_platform::{StdFs, sha256_file};
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

/// One entry of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A regular file.
    File {
        /// Size in bytes.
        size: u64,
        /// Lowercase hex SHA-256 of the content.
        sha256: String,
    },
    /// A directory.
    Dir,
}

/// A change between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Present only in the later snapshot.
    Added(String),
    /// Present only in the earlier snapshot.
    Removed(String),
    /// Present in both with different content or kind.
    Modified(String),
}

impl Change {
    fn path(&self) -> &str {
        match self {
            Self::Added(p) | Self::Removed(p) | Self::Modified(p) => p,
        }
    }
}

/// The state of a directory tree. Keys use `/` and no trailing slash; the root is not an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    entries: BTreeMap<String, Entry>,
}

impl Snapshot {
    /// Walks `root` without following symlinks; a symlink is an error.
    pub fn of(root: &Path) -> Result<Snapshot> {
        let fs = StdFs::new();
        let mut entries = BTreeMap::new();
        for item in WalkDir::new(root).follow_links(false).min_depth(1) {
            let item = item.map_err(|e| {
                let path = e.path().unwrap_or(root).to_path_buf();
                let source = e
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("directory walk failed"));
                Error::Io {
                    op: "walk",
                    path,
                    source,
                }
            })?;
            let rel = item
                .path()
                .strip_prefix(root)
                .map_err(|e| Error::io("strip_prefix", item.path(), std::io::Error::other(e)))?;
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let ft = item.file_type();
            if ft.is_symlink() {
                return Err(Error::SymlinkInSnapshot(key));
            }
            let entry = if ft.is_dir() {
                Entry::Dir
            } else {
                let meta = item
                    .metadata()
                    .map_err(|e| Error::io("metadata", item.path(), std::io::Error::other(e)))?;
                let sha256 = sha256_file(&fs, item.path())
                    .map_err(|e| Error::io("hash", item.path(), std::io::Error::other(e)))?;
                Entry::File {
                    size: meta.len(),
                    sha256,
                }
            };
            entries.insert(key, entry);
        }
        Ok(Snapshot { entries })
    }

    /// All entries by relative path.
    pub fn entries(&self) -> &BTreeMap<String, Entry> {
        &self.entries
    }

    /// Changes needed to go from `self` (before) to `after`, sorted by path.
    pub fn diff(&self, after: &Snapshot) -> Vec<Change> {
        let mut out = Vec::new();
        for (path, entry) in &self.entries {
            match after.entries.get(path) {
                None => out.push(Change::Removed(path.clone())),
                Some(other) if other != entry => out.push(Change::Modified(path.clone())),
                Some(_) => {}
            }
        }
        for path in after.entries.keys() {
            if !self.entries.contains_key(path) {
                out.push(Change::Added(path.clone()));
            }
        }
        out.sort_by(|a, b| a.path().cmp(b.path()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_is_recorded_and_root_is_not() -> Result<()> {
        let tmp = tempfile::tempdir().map_err(|e| Error::io("tempdir", Path::new(""), e))?;
        std::fs::create_dir_all(tmp.path().join("a/b"))
            .map_err(|e| Error::io("mkdir", tmp.path(), e))?;
        let snap = Snapshot::of(tmp.path())?;
        let keys: Vec<_> = snap.entries().keys().cloned().collect();
        assert_eq!(keys, ["a", "a/b"]);
        assert_eq!(snap.entries().get("a/b"), Some(&Entry::Dir));
        Ok(())
    }

    #[test]
    fn diff_kinds_sorted() {
        let file = |h: &str| Entry::File {
            size: 1,
            sha256: h.into(),
        };
        let mk = |v: &[(&str, Entry)]| Snapshot {
            entries: v
                .iter()
                .map(|(k, e)| ((*k).to_owned(), e.clone()))
                .collect(),
        };
        let before = mk(&[("b", file("1")), ("c", file("1")), ("d", Entry::Dir)]);
        let after = mk(&[("a", file("1")), ("b", file("2")), ("d", file("1"))]);
        assert_eq!(
            before.diff(&after),
            vec![
                Change::Added("a".into()),
                Change::Modified("b".into()),
                Change::Removed("c".into()),
                Change::Modified("d".into()),
            ]
        );
    }
}
