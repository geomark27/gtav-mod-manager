//! Pass 2: streams validated entries into a fresh private staging directory.

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::validate::{Planned, plan};
use gmm_domain::{RelPath, Sha256};
use gmm_platform::{Error as PlatformError, Fs};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::ZipArchive;

const CHUNK: usize = 64 * 1024;
const NAME_ATTEMPTS: u32 = 64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A file extracted into staging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedEntry {
    /// Path of the file inside the archive.
    pub rel_path: RelPath,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 of the staged content.
    pub sha256: Sha256,
    /// Location of the staged file.
    pub staged_path: PathBuf,
}

/// The result of staging an archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedArchive {
    /// The private staging directory.
    pub dir: PathBuf,
    /// Files only, sorted by case-insensitive key.
    pub entries: Vec<StagedEntry>,
}

/// Tracks what this call created so a failure removes exactly that.
struct Created<'a> {
    fs: &'a dyn Fs,
    root: PathBuf,
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Created<'_> {
    fn cleanup(&mut self) {
        for f in self.files.drain(..).rev() {
            if let Err(e) = self.fs.remove_file(&f) {
                tracing::warn!(path = %f.display(), error = %e, "staging cleanup: remove_file failed");
            }
        }
        for d in self.dirs.drain(..).rev() {
            if let Err(e) = self.fs.remove_dir(&d) {
                tracing::warn!(path = %d.display(), error = %e, "staging cleanup: remove_dir failed");
            }
        }
        if let Err(e) = self.fs.remove_dir(&self.root) {
            tracing::warn!(path = %self.root.display(), error = %e, "staging cleanup: remove_dir failed");
        }
    }
}

/// Creates a NEW directory under `root` using names from `next_name`; never reuses one.
pub(crate) fn create_unique_dir(
    fs: &dyn Fs,
    root: &Path,
    next_name: &mut dyn FnMut() -> String,
) -> Result<PathBuf> {
    fs.create_dir_all(root)?;
    for _ in 0..NAME_ATTEMPTS {
        let dir = root.join(next_name());
        match fs.create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(PlatformError::AlreadyExists { .. }) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique staging directory",
    )))
}

fn staging_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("stage-{nanos}-{n}")
}

/// Streams `src` into a NEW file (never overwrites), hashing and counting while writing.
/// `budget` is decremented by the bytes written; exceeding it or `max` aborts.
fn write_streaming(
    src: &mut dyn Read,
    dest: &Path,
    max: u64,
    total_left: &mut u64,
    cancel: &dyn Fn() -> bool,
    created: &mut Created<'_>,
) -> Result<(u64, Sha256)> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(dest)?;
    created.files.push(dest.to_path_buf());
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut written = 0u64;
    loop {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Error::Zip(e.to_string())),
        };
        let n64 = n as u64;
        written += n64;
        if written > max {
            return Err(Error::TooLarge {
                what: "entry size",
                limit: max,
            });
        }
        if n64 > *total_left {
            return Err(Error::TooLarge {
                what: "total size",
                limit: 0,
            });
        }
        *total_left -= n64;
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
    }
    file.sync_all()?;
    let digest: [u8; 32] = hasher.finalize().into();
    Ok((written, Sha256::from_bytes(digest)))
}

/// Extracts `zip_path` into a new private directory under `staging_root`.
///
/// The whole archive is validated from metadata first; nothing is written if any entry is
/// unsafe. Declared sizes are checked up front and the real byte counts again while writing.
/// On any error the files and directories created by this call are removed.
pub fn stage_zip(
    fs: &dyn Fs,
    zip_path: &Path,
    staging_root: &Path,
    limits: &Limits,
    cancel: &dyn Fn() -> bool,
) -> Result<StagedArchive> {
    stage_with_names(
        fs,
        zip_path,
        staging_root,
        limits,
        cancel,
        &mut staging_name,
    )
}

pub(crate) fn stage_with_names(
    fs: &dyn Fs,
    zip_path: &Path,
    staging_root: &Path,
    limits: &Limits,
    cancel: &dyn Fn() -> bool,
    next_name: &mut dyn FnMut() -> String,
) -> Result<StagedArchive> {
    let file = std::fs::File::open(zip_path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| Error::Zip(e.to_string()))?;
    let planned = plan(&mut zip, limits, cancel)?;

    let dir = create_unique_dir(fs, staging_root, next_name)?;
    let mut created = Created {
        fs,
        root: dir.clone(),
        files: Vec::new(),
        dirs: Vec::new(),
    };
    match extract(fs, &mut zip, planned, &dir, limits, cancel, &mut created) {
        Ok(entries) => {
            tracing::debug!(dir = %dir.display(), files = entries.len(), "archive staged");
            Ok(StagedArchive { dir, entries })
        }
        Err(e) => {
            created.cleanup();
            Err(e)
        }
    }
}

fn extract(
    fs: &dyn Fs,
    zip: &mut ZipArchive<std::fs::File>,
    planned: Vec<Planned>,
    dir: &Path,
    limits: &Limits,
    cancel: &dyn Fn() -> bool,
    created: &mut Created<'_>,
) -> Result<Vec<StagedEntry>> {
    // Lowercase relative dir key -> real path on disk (first spelling wins).
    let mut dirs: HashMap<String, PathBuf> = HashMap::new();
    let mut total_left = limits.max_total_bytes;
    let mut entries = Vec::with_capacity(planned.len());
    for p in planned {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let mut parent = dir.to_path_buf();
        let mut key = String::new();
        let comps: Vec<&str> = p.rel.components().collect();
        let (name, folders) = comps.split_last().ok_or_else(|| Error::UnsafePath {
            name: p.rel.to_string(),
        })?;
        for comp in folders {
            if !key.is_empty() {
                key.push('/');
            }
            key.push_str(&comp.to_lowercase());
            if let Some(existing) = dirs.get(&key) {
                parent.clone_from(existing);
            } else {
                parent.push(comp);
                fs.create_dir(&parent)?;
                created.dirs.push(parent.clone());
                dirs.insert(key.clone(), parent.clone());
            }
        }
        let dest = parent.join(name);
        let mut entry = zip
            .by_index(p.index)
            .map_err(|e| Error::Zip(e.to_string()))?;
        let max = limits.max_entry_bytes.min(p.declared_size);
        let (size, sha256) =
            write_streaming(&mut entry, &dest, max, &mut total_left, cancel, created)?;
        if size != p.declared_size {
            return Err(Error::Zip(format!(
                "entry {} is shorter than declared",
                p.rel
            )));
        }
        entries.push(StagedEntry {
            rel_path: p.rel,
            size,
            sha256,
            staged_path: dest,
        });
    }
    entries.sort_by_key(|e| e.rel_path.key());
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmm_platform::StdFs;

    #[test]
    fn unique_dir_never_reuses_an_existing_one() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fs = StdFs::new();
        let existing = tmp.path().join("fixed");
        std::fs::create_dir(&existing).expect("mkdir");
        std::fs::write(existing.join("keep.txt"), b"keep").expect("write");
        // First name collides with the existing dir, second is free.
        let mut names = vec!["fixed".to_owned(), "other".to_owned()].into_iter();
        let dir = create_unique_dir(&fs, tmp.path(), &mut || names.next().unwrap_or_default())
            .expect("dir");
        assert_eq!(dir, tmp.path().join("other"));
        assert_eq!(
            std::fs::read(existing.join("keep.txt")).expect("read"),
            b"keep"
        );
        // Only colliding names: fails and leaves the dir untouched.
        let err = create_unique_dir(&fs, tmp.path(), &mut || "fixed".to_owned());
        assert!(err.is_err());
        assert_eq!(
            std::fs::read(existing.join("keep.txt")).expect("read"),
            b"keep"
        );
    }
}
