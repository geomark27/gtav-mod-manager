//! Streaming SHA-256 hashing.

use crate::error::{Error, Result};
use crate::fs::Fs;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

const BUF_SIZE: usize = 1024 * 1024;

/// Hashes everything from `reader`; returns lowercase hex.
///
/// `progress` receives the cumulative byte count after each chunk; returning `false`
/// aborts with `Error::Cancelled`.
pub fn sha256_reader(
    reader: &mut dyn Read,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; BUF_SIZE];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Error::io("read", Path::new(""), e)),
        };
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
        if !progress(total) {
            return Err(Error::Cancelled);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Hashes a file read through `fs`.
pub fn sha256_file(fs: &dyn Fs, path: &Path) -> Result<String> {
    let mut r = fs.open_read(path)?;
    sha256_reader(&mut r, &mut |_| true).map_err(|e| match e {
        Error::Io { op, source, .. } => Error::Io {
            op,
            path: path.to_path_buf(),
            source,
        },
        other => other,
    })
}

/// Hashes a byte slice.
pub fn sha256_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::std_fs::StdFs;

    #[test]
    fn known_vectors() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let h = sha256_reader(&mut &b"abc"[..], &mut |_| true).unwrap();
        assert_eq!(h, sha256_bytes(b"abc"));
    }

    fn big() -> Vec<u8> {
        (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn large_file_matches_bytes_and_progress_increases() {
        let d = tempfile::TempDir::new().unwrap();
        let p = d.path().join("big");
        let data = big();
        std::fs::write(&p, &data).unwrap();
        assert_eq!(sha256_file(&StdFs, &p).unwrap(), sha256_bytes(&data));
        let mut seen = Vec::new();
        let mut r = StdFs.open_read(&p).unwrap();
        sha256_reader(&mut r, &mut |n| {
            seen.push(n);
            true
        })
        .unwrap();
        assert!(seen.len() >= 3);
        assert!(seen.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(*seen.last().unwrap(), data.len() as u64);
    }

    #[test]
    fn cancel_returns_cancelled() {
        let data = big();
        let r = sha256_reader(&mut &data[..], &mut |_| false);
        assert!(matches!(r, Err(Error::Cancelled)));
    }

    #[test]
    fn missing_file_errors() {
        let d = tempfile::TempDir::new().unwrap();
        assert!(sha256_file(&StdFs, &d.path().join("nope")).is_err());
    }
}
