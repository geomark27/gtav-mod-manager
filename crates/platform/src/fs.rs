//! The `Fs` trait: every file-system effect of the product goes through it.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// Kind of a directory entry (never follows symlinks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link.
    Symlink,
    /// Anything else.
    Other,
}

/// Metadata of an entry, obtained without following links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    /// Size in bytes.
    pub len: u64,
    /// Modification time, nanoseconds since the Unix epoch.
    pub modified_ns: i64,
    /// Entry kind.
    pub kind: EntryKind,
    /// True for symlinks and Windows reparse points.
    pub is_reparse_point: bool,
    /// Hard link count, when it could be determined.
    pub hard_links: Option<u64>,
    /// Read-only attribute.
    pub readonly: bool,
}

/// A directory listing entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntryInfo {
    /// Full path of the entry.
    pub path: PathBuf,
    /// File name (lossy UTF-8).
    pub file_name: String,
}

/// Mutating operation kinds (the ones fault injection can target).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FsOp {
    /// `create_dir` / `create_dir_all`.
    CreateDir,
    /// `copy_new`.
    Copy,
    /// `write_new`.
    Write,
    /// `rename_no_replace`.
    Rename,
    /// `remove_file`.
    RemoveFile,
    /// `remove_dir`.
    RemoveDir,
    /// `set_readonly`.
    SetReadonly,
}

/// File-system abstraction.
pub trait Fs: Send + Sync {
    /// Metadata WITHOUT following symlinks / reparse points.
    fn metadata(&self, path: &Path) -> Result<FileMeta>;
    /// No-follow existence check; `Ok(false)` on NotFound.
    fn exists(&self, path: &Path) -> Result<bool>;
    /// Lists a directory, sorted by file name.
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>>;
    /// Opens a file for reading.
    fn open_read(&self, path: &Path) -> Result<Box<dyn std::io::Read + Send>>;
    /// Creates a single directory level; `AlreadyExists` if present.
    fn create_dir(&self, path: &Path) -> Result<()>;
    /// Creates a directory and its parents.
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    /// Copies to a NEW file (fails with `AlreadyExists` if `to` exists), fsyncs it,
    /// removes the partial destination on failure. Returns bytes copied.
    fn copy_new(&self, from: &Path, to: &Path) -> Result<u64>;
    /// Writes a NEW file (create_new) and fsyncs it.
    fn write_new(&self, path: &Path, data: &[u8]) -> Result<()>;
    /// Renames a FILE; never overwrites (`AlreadyExists`). Same volume only.
    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<()>;
    /// Removes a file.
    fn remove_file(&self, path: &Path) -> Result<()>;
    /// Removes an empty directory.
    fn remove_dir(&self, path: &Path) -> Result<()>;
    /// Sets or clears the read-only attribute.
    fn set_readonly(&self, path: &Path, readonly: bool) -> Result<()>;
}
