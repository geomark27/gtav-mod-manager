//! `StdFs`: the real file system implementation built on `std::fs`.

use crate::error::{Error, Result};
use crate::fs::{DirEntryInfo, EntryKind, FileMeta, Fs};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

const BUF_SIZE: usize = 1024 * 1024;

/// Real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFs;

impl StdFs {
    /// Creates a new instance.
    pub fn new() -> Self {
        StdFs
    }
}

fn copy_inner(from: &Path, to: &Path) -> Result<u64> {
    let mut src = std::fs::File::open(from).map_err(|e| Error::io("open", from, e))?;
    let mut dst = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| Error::io("create", to, e))?;
    // From here on the destination is ours: any failure must remove it.
    let result = (|| -> Result<u64> {
        let mut buf = vec![0u8; BUF_SIZE];
        let mut total = 0u64;
        loop {
            let n = src.read(&mut buf).map_err(|e| Error::io("read", from, e))?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])
                .map_err(|e| Error::io("write", to, e))?;
            total += n as u64;
        }
        dst.sync_all().map_err(|e| Error::io("sync", to, e))?;
        Ok(total)
    })();
    if result.is_err() {
        drop(dst);
        let _ = std::fs::remove_file(to);
    }
    result
}

#[cfg(windows)]
fn rename_impl(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain(Some(0)).collect() };
    let (f, t) = (wide(from), wide(to));
    // SAFETY: `f` and `t` are valid NUL-terminated UTF-16 buffers that outlive the call.
    // No MOVEFILE_REPLACE_EXISTING flag is passed, so an existing target is never replaced.
    let ok = unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error().map(|c| c as u32) {
        Some(ERROR_ALREADY_EXISTS) | Some(ERROR_FILE_EXISTS) => Err(Error::AlreadyExists {
            path: to.to_path_buf(),
        }),
        _ => Err(Error::Io {
            op: "rename",
            path: from.to_path_buf(),
            source: err,
        }),
    }
}

/// Non-Windows fallback: hard link (fails if `to` exists) then unlink. Files only.
#[cfg(not(windows))]
fn rename_impl(from: &Path, to: &Path) -> Result<()> {
    std::fs::hard_link(from, to).map_err(|e| Error::io("rename", to, e))?;
    std::fs::remove_file(from).map_err(|e| Error::io("rename", from, e))
}

#[cfg(windows)]
fn hard_links_of(path: &Path) -> Option<u64> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        GetFileInformationByHandle,
    };

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .ok()?;
    // SAFETY: BY_HANDLE_FILE_INFORMATION is plain-old-data, valid when zeroed.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the handle is valid for the lifetime of `file`; `info` is a valid out pointer.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) };
    if ok == 0 {
        None
    } else {
        Some(u64::from(info.nNumberOfLinks))
    }
}

fn to_meta(md: &std::fs::Metadata, path: &Path) -> FileMeta {
    let _ = path; // only used on Windows
    let ft = md.file_type();
    let kind = if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    let modified_ns = md
        .modified()
        .ok()
        .map(|t| match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_nanos()).unwrap_or(i64::MAX),
            Err(e) => i64::try_from(e.duration().as_nanos())
                .map(|n| -n)
                .unwrap_or(i64::MIN),
        })
        .unwrap_or(0);
    #[cfg(windows)]
    let (is_reparse_point, hard_links) = {
        use std::os::windows::fs::MetadataExt;
        (md.file_attributes() & 0x400 != 0, hard_links_of(path))
    };
    #[cfg(unix)]
    let (is_reparse_point, hard_links) = {
        use std::os::unix::fs::MetadataExt;
        (ft.is_symlink(), Some(md.nlink()))
    };
    #[cfg(not(any(windows, unix)))]
    let (is_reparse_point, hard_links) = (ft.is_symlink(), None);
    FileMeta {
        len: md.len(),
        modified_ns,
        kind,
        is_reparse_point,
        hard_links,
        readonly: md.permissions().readonly(),
    }
}

impl Fs for StdFs {
    fn metadata(&self, path: &Path) -> Result<FileMeta> {
        let md = std::fs::symlink_metadata(path).map_err(|e| Error::io("metadata", path, e))?;
        Ok(to_meta(&md, path))
    }

    fn exists(&self, path: &Path) -> Result<bool> {
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Error::io("exists", path, e)),
        }
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path).map_err(|e| Error::io("read_dir", path, e))? {
            let entry = entry.map_err(|e| Error::io("read_dir", path, e))?;
            out.push(DirEntryInfo {
                path: entry.path(),
                file_name: entry.file_name().to_string_lossy().into_owned(),
            });
        }
        out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(out)
    }

    fn open_read(&self, path: &Path) -> Result<Box<dyn Read + Send>> {
        let f = std::fs::File::open(path).map_err(|e| Error::io("open", path, e))?;
        Ok(Box::new(f))
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        std::fs::create_dir(path).map_err(|e| Error::io("create_dir", path, e))
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path).map_err(|e| Error::io("create_dir_all", path, e))
    }

    fn copy_new(&self, from: &Path, to: &Path) -> Result<u64> {
        copy_inner(from, to)
    }

    fn write_new(&self, path: &Path, data: &[u8]) -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| Error::io("create", path, e))?;
        let res = f
            .write_all(data)
            .map_err(|e| Error::io("write", path, e))
            .and_then(|()| f.sync_all().map_err(|e| Error::io("sync", path, e)));
        if res.is_err() {
            drop(f);
            let _ = std::fs::remove_file(path);
        }
        res
    }

    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<()> {
        rename_impl(from, to)
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path).map_err(|e| Error::io("remove_file", path, e))
    }

    fn remove_dir(&self, path: &Path) -> Result<()> {
        std::fs::remove_dir(path).map_err(|e| Error::io("remove_dir", path, e))
    }

    fn set_readonly(&self, path: &Path, readonly: bool) -> Result<()> {
        let md = std::fs::metadata(path).map_err(|e| Error::io("set_readonly", path, e))?;
        let mut perms = md.permissions();
        perms.set_readonly(readonly);
        std::fs::set_permissions(path, perms).map_err(|e| Error::io("set_readonly", path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, StdFs) {
        (TempDir::new().unwrap(), StdFs)
    }

    #[test]
    fn copy_new_copies_and_refuses_existing() {
        let (d, fs) = setup();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        std::fs::write(&a, b"hello").unwrap();
        assert_eq!(fs.copy_new(&a, &b).unwrap(), 5);
        assert_eq!(std::fs::read(&b).unwrap(), b"hello");
        std::fs::write(&a, b"other").unwrap();
        assert!(matches!(
            fs.copy_new(&a, &b),
            Err(Error::AlreadyExists { .. })
        ));
        assert_eq!(std::fs::read(&b).unwrap(), b"hello");
    }

    #[test]
    fn copy_new_missing_source_leaves_no_destination() {
        let (d, fs) = setup();
        let b = d.path().join("b");
        assert!(fs.copy_new(&d.path().join("nope"), &b).is_err());
        assert!(!b.exists());
    }

    #[test]
    fn write_new_writes_and_refuses_existing() {
        let (d, fs) = setup();
        let p = d.path().join("w");
        fs.write_new(&p, b"one").unwrap();
        assert!(matches!(
            fs.write_new(&p, b"two"),
            Err(Error::AlreadyExists { .. })
        ));
        assert_eq!(std::fs::read(&p).unwrap(), b"one");
    }

    #[test]
    fn rename_moves_and_never_overwrites() {
        let (d, fs) = setup();
        let (a, b, c) = (d.path().join("a"), d.path().join("b"), d.path().join("c"));
        std::fs::write(&a, b"AAA").unwrap();
        fs.rename_no_replace(&a, &b).unwrap();
        assert!(!a.exists());
        assert_eq!(std::fs::read(&b).unwrap(), b"AAA");
        std::fs::write(&c, b"CCC").unwrap();
        assert!(matches!(
            fs.rename_no_replace(&c, &b),
            Err(Error::AlreadyExists { .. })
        ));
        assert_eq!(std::fs::read(&b).unwrap(), b"AAA");
        assert_eq!(std::fs::read(&c).unwrap(), b"CCC");
    }

    #[test]
    fn remove_dir_fails_when_not_empty() {
        let (d, fs) = setup();
        let sub = d.path().join("sub");
        fs.create_dir(&sub).unwrap();
        fs.write_new(&sub.join("f"), b"x").unwrap();
        assert!(fs.remove_dir(&sub).is_err());
        fs.remove_file(&sub.join("f")).unwrap();
        fs.remove_dir(&sub).unwrap();
        assert!(!fs.exists(&sub).unwrap());
    }

    #[test]
    fn create_dir_reports_already_exists() {
        let (d, fs) = setup();
        let sub = d.path().join("sub");
        fs.create_dir(&sub).unwrap();
        assert!(matches!(
            fs.create_dir(&sub),
            Err(Error::AlreadyExists { .. })
        ));
        fs.create_dir_all(&sub.join("x/y")).unwrap();
    }

    #[test]
    fn metadata_reports_kind_len_readonly() {
        let (d, fs) = setup();
        let f = d.path().join("f");
        std::fs::write(&f, b"12345").unwrap();
        let m = fs.metadata(&f).unwrap();
        assert_eq!(m.kind, EntryKind::File);
        assert_eq!(m.len, 5);
        assert!(!m.readonly);
        assert!(m.modified_ns > 0);
        fs.set_readonly(&f, true).unwrap();
        assert!(fs.metadata(&f).unwrap().readonly);
        fs.set_readonly(&f, false).unwrap();
        assert_eq!(fs.metadata(d.path()).unwrap().kind, EntryKind::Dir);
    }

    #[test]
    fn exists_false_for_missing() {
        let (d, fs) = setup();
        assert!(!fs.exists(&d.path().join("missing")).unwrap());
        assert!(fs.metadata(&d.path().join("missing")).is_err());
    }

    #[test]
    fn read_dir_is_sorted() {
        let (d, fs) = setup();
        for n in ["c", "a", "b"] {
            std::fs::write(d.path().join(n), b"").unwrap();
        }
        let names: Vec<_> = fs
            .read_dir(d.path())
            .unwrap()
            .into_iter()
            .map(|e| e.file_name)
            .collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn open_read_reads_content() {
        let (d, fs) = setup();
        let f = d.path().join("f");
        std::fs::write(&f, b"data").unwrap();
        let mut s = String::new();
        fs.open_read(&f).unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "data");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_not_followed() {
        let (d, fs) = setup();
        let (t, l) = (d.path().join("t"), d.path().join("l"));
        std::fs::write(&t, b"x").unwrap();
        std::os::unix::fs::symlink(&t, &l).unwrap();
        let m = fs.metadata(&l).unwrap();
        assert_eq!(m.kind, EntryKind::Symlink);
        assert!(m.is_reparse_point);
        assert!(!fs.metadata(&t).unwrap().is_reparse_point);
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_counted() {
        let (d, fs) = setup();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        std::fs::write(&a, b"x").unwrap();
        assert_eq!(fs.metadata(&a).unwrap().hard_links, Some(1));
        std::fs::hard_link(&a, &b).unwrap();
        assert_eq!(fs.metadata(&a).unwrap().hard_links, Some(2));
    }
}
