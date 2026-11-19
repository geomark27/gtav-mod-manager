//! Deterministic, no-follow directory walk shared by scanning and baseline capture.

use crate::error::Result;
use gmm_domain::RelPath;
use gmm_platform::{EntryKind, FileMeta, Fs};
use std::path::{Path, PathBuf};

/// A file (or reparse point) found by [`walk`].
pub(crate) struct WalkEntry {
    pub rel: RelPath,
    pub abs: PathBuf,
    pub meta: FileMeta,
}

/// Visits every regular file and every reparse point under `root`, depth first, entries
/// ordered by lowercase name then exact name. Reparse points are reported but never
/// descended. Names that are not valid [`RelPath`]s increment `skipped_unparsable`.
pub(crate) fn walk(
    fs: &dyn Fs,
    root: &Path,
    skipped_unparsable: &mut u64,
    visit: &mut dyn FnMut(WalkEntry) -> Result<()>,
) -> Result<()> {
    walk_dir(fs, root, None, skipped_unparsable, visit)
}

fn walk_dir(
    fs: &dyn Fs,
    dir: &Path,
    dir_rel: Option<&RelPath>,
    skipped: &mut u64,
    visit: &mut dyn FnMut(WalkEntry) -> Result<()>,
) -> Result<()> {
    let mut entries = fs.read_dir(dir)?;
    entries.sort_by(|a, b| {
        (a.file_name.to_lowercase(), &a.file_name).cmp(&(b.file_name.to_lowercase(), &b.file_name))
    });
    for entry in entries {
        let parsed = match dir_rel {
            None => RelPath::parse(&entry.file_name),
            Some(parent) => parent.join(&entry.file_name),
        };
        let rel = match parsed {
            Ok(rel) => rel,
            Err(e) => {
                *skipped += 1;
                tracing::warn!(path = %entry.path.display(), error = %e, "skipping unparsable name");
                continue;
            }
        };
        let meta = match fs.metadata(&entry.path) {
            Ok(m) => m,
            Err(gmm_platform::Error::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                tracing::warn!(path = %entry.path.display(), "entry vanished during the walk");
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if meta.is_reparse_point || meta.kind == EntryKind::File {
            visit(WalkEntry {
                rel,
                abs: entry.path,
                meta,
            })?;
        } else if meta.kind == EntryKind::Dir {
            walk_dir(fs, &entry.path, Some(&rel), skipped, visit)?;
        } else {
            tracing::debug!(path = %entry.path.display(), "ignoring special file");
        }
    }
    Ok(())
}
