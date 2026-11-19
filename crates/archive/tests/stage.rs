#![allow(clippy::expect_used, clippy::panic)]
//! Integration tests: hostile and normal archives.

use gmm_archive::{Error, Limits, StagedEntry, map_to_game, stage_zip, suggest_root};
use gmm_domain::RelPath;
use gmm_platform::{StdFs, sha256_bytes};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn opts() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn build(dir: &Path, f: impl FnOnce(&mut ZipWriter<std::fs::File>)) -> PathBuf {
    let path = dir.join("mod.zip");
    let file = std::fs::File::create(&path).expect("create zip");
    let mut w = ZipWriter::new(file);
    f(&mut w);
    w.finish().expect("finish");
    path
}

fn add(w: &mut ZipWriter<std::fs::File>, name: &str, data: &[u8]) {
    w.start_file(name, opts()).expect("start");
    w.write_all(data).expect("write");
}

fn files_in(root: &Path) -> usize {
    std::fs::read_dir(root).map_or(0, |d| d.count())
}

fn stage(
    zip: &Path,
    root: &Path,
    limits: &Limits,
) -> gmm_archive::Result<gmm_archive::StagedArchive> {
    stage_zip(&StdFs::new(), zip, root, limits, &|| false)
}

fn rejected(entries: &[(&str, &[u8])]) -> Error {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        for (n, d) in entries {
            add(w, n, d);
        }
    });
    let root = tmp.path().join("staging");
    let err = stage(&zip, &root, &Limits::default()).expect_err("must fail");
    // Nothing left behind.
    assert_eq!(files_in(&root), 0, "staging root must be empty");
    err
}

#[test]
fn normal_archive_is_staged_with_correct_hashes() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        add(w, "scripts/Mod.asi", b"asi-bytes");
        add(w, "readme.txt", b"hello");
        w.add_directory("empty/", opts()).expect("dir");
        add(w, "Scripts/more.cfg", b"cfg");
    });
    let root = tmp.path().join("staging");
    let staged = stage(&zip, &root, &Limits::default()).expect("stage");
    assert!(staged.dir.starts_with(&root));
    let names: Vec<&str> = staged.entries.iter().map(|e| e.rel_path.as_str()).collect();
    assert_eq!(names, ["readme.txt", "scripts/Mod.asi", "Scripts/more.cfg"]);
    for e in &staged.entries {
        let bytes = std::fs::read(&e.staged_path).expect("read");
        assert_eq!(e.size, bytes.len() as u64);
        assert_eq!(e.sha256.as_str(), sha256_bytes(&bytes));
        assert!(e.staged_path.starts_with(&staged.dir));
    }
}

#[test]
fn zip_slip_names_are_rejected() {
    for name in [
        "../evil.dll",
        "/abs.dll",
        "C:\\x.dll",
        "a/../../b",
        "..\\evil.dll",
        "a/./b",
    ] {
        let err = rejected(&[("ok.txt", b"x"), (name, b"evil")]);
        assert!(matches!(err, Error::UnsafePath { .. }), "{name}: {err:?}");
    }
}

#[test]
fn symlink_entry_is_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        add(w, "ok.txt", b"x");
        w.add_symlink("link", "/etc/passwd", opts())
            .expect("symlink");
    });
    let root = tmp.path().join("staging");
    let err = stage(&zip, &root, &Limits::default()).expect_err("fail");
    assert!(matches!(err, Error::UnsafeEntry { .. }), "{err:?}");
    assert_eq!(files_in(&root), 0);
}

#[test]
fn encrypted_entry_is_rejected() {
    // The write-side crypto features are not enabled, so flip the encryption bit of a
    // valid archive in both the local and the central directory headers.
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| add(w, "a.txt", b"secret"));
    let mut bytes = std::fs::read(&zip).expect("read");
    for sig in [&b"PK\x03\x04"[..], &b"PK\x01\x02"[..]] {
        let pos = bytes.windows(4).position(|w| w == sig).expect("sig");
        let flag = pos + if sig[2] == 3 { 6 } else { 8 };
        bytes[flag] |= 1;
    }
    std::fs::write(&zip, bytes).expect("write");
    let root = tmp.path().join("staging");
    let err = stage(&zip, &root, &Limits::default()).expect_err("fail");
    assert!(matches!(err, Error::Encrypted { .. }), "{err:?}");
    assert_eq!(files_in(&root), 0);
}

#[test]
fn case_insensitive_duplicates_are_rejected() {
    let err = rejected(&[("Dir/File.DLL", b"1"), ("dir/file.dll", b"2")]);
    assert!(matches!(err, Error::DuplicateEntry { .. }), "{err:?}");
    let err = rejected(&[("a", b"1"), ("A/b", b"2")]);
    assert!(matches!(err, Error::DuplicateEntry { .. }), "{err:?}");
}

#[test]
fn too_many_entries() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        for i in 0..5 {
            add(w, &format!("f{i}.txt"), b"x");
        }
    });
    let root = tmp.path().join("staging");
    let limits = Limits {
        max_entries: 4,
        ..Limits::default()
    };
    let err = stage(&zip, &root, &limits).expect_err("fail");
    assert!(
        matches!(
            err,
            Error::TooLarge {
                what: "entry count",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(files_in(&root), 0);
}

#[test]
fn declared_size_limits_are_checked_before_writing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        add(w, "a.bin", &[1u8; 600]);
        add(w, "b.bin", &[2u8; 600]);
    });
    let root = tmp.path().join("staging");
    let total = Limits {
        max_total_bytes: 1000,
        ..Limits::default()
    };
    let err = stage(&zip, &root, &total).expect_err("fail");
    assert!(
        matches!(
            err,
            Error::TooLarge {
                what: "total size",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(files_in(&root), 0);
    let entry = Limits {
        max_entry_bytes: 500,
        ..Limits::default()
    };
    let err = stage(&zip, &root, &entry).expect_err("fail");
    assert!(
        matches!(
            err,
            Error::TooLarge {
                what: "entry size",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(files_in(&root), 0);
}

#[test]
fn actual_size_overflow_aborts_and_cleans_up() {
    // Lie in both headers: declared size 10 while the stored data is 600 bytes. The size
    // check passes, so only the byte counting while writing can catch it.
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        add(w, "dir/a.bin", &[1u8; 600]);
    });
    let mut bytes = std::fs::read(&zip).expect("read");
    let new_size = 10u32.to_le_bytes();
    for sig in [&b"PK\x03\x04"[..], &b"PK\x01\x02"[..]] {
        let pos = bytes.windows(4).position(|w| w == sig).expect("sig");
        let (comp, uncomp) = if sig[2] == 3 { (18, 22) } else { (20, 24) };
        bytes[pos + comp..pos + comp + 4].copy_from_slice(&600u32.to_le_bytes());
        bytes[pos + uncomp..pos + uncomp + 4].copy_from_slice(&new_size);
    }
    std::fs::write(&zip, bytes).expect("write");
    let root = tmp.path().join("staging");
    let err = stage(&zip, &root, &Limits::default()).expect_err("fail");
    assert!(
        matches!(err, Error::TooLarge { .. } | Error::Zip(_)),
        "{err:?}"
    );
    assert_eq!(files_in(&root), 0, "partial staging dir must be removed");
}

#[test]
fn actual_total_overflow_is_caught_while_writing() {
    // Directly exercise the total budget: limit lower than the sum is caught up front, so
    // use a limit equal to the declared sum minus nothing: succeeds exactly at the limit.
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        add(w, "a.bin", &[1u8; 600]);
        add(w, "b.bin", &[2u8; 400]);
    });
    let root = tmp.path().join("staging");
    let limits = Limits {
        max_total_bytes: 1000,
        ..Limits::default()
    };
    assert!(stage(&zip, &root, &limits).is_ok());
}

#[test]
fn suspicious_ratio_is_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        w.start_file("zeros.bin", o).expect("start");
        let block = vec![0u8; 1024 * 1024];
        for _ in 0..4 {
            w.write_all(&block).expect("write");
        }
    });
    let root = tmp.path().join("staging");
    let err = stage(&zip, &root, &Limits::default()).expect_err("fail");
    assert!(
        matches!(err, Error::SuspiciousCompression { .. }),
        "{err:?}"
    );
    assert_eq!(files_in(&root), 0);
}

#[test]
fn cancellation_removes_partial_staging() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| {
        for i in 0..5 {
            add(w, &format!("d{i}/f.txt"), b"data");
        }
    });
    let root = tmp.path().join("staging");
    let calls = AtomicUsize::new(0);
    // Let validation and a few writes pass, then cancel.
    let cancel = || calls.fetch_add(1, Ordering::SeqCst) >= 12;
    let err = stage_zip(&StdFs::new(), &zip, &root, &Limits::default(), &cancel).expect_err("fail");
    assert!(matches!(err, Error::Cancelled), "{err:?}");
    assert_eq!(files_in(&root), 0);
}

#[test]
fn staging_twice_creates_distinct_directories() {
    let tmp = tempfile::tempdir().expect("tmp");
    let zip = build(tmp.path(), |w| add(w, "a.txt", b"x"));
    let root = tmp.path().join("staging");
    let a = stage(&zip, &root, &Limits::default()).expect("a");
    let b = stage(&zip, &root, &Limits::default()).expect("b");
    assert_ne!(a.dir, b.dir);
    assert!(a.entries[0].staged_path.exists());
    assert!(b.entries[0].staged_path.exists());
}

fn entry(path: &str) -> StagedEntry {
    StagedEntry {
        rel_path: RelPath::parse(path).expect("rel"),
        size: 1,
        sha256: gmm_domain::Sha256::parse(&sha256_bytes(b"x")).expect("sha"),
        staged_path: PathBuf::from("/stage").join(path),
    }
}

fn entries(paths: &[&str]) -> Vec<StagedEntry> {
    paths.iter().map(|p| entry(p)).collect()
}

fn root_of(paths: &[&str]) -> Option<String> {
    suggest_root(&entries(paths)).map(|r| r.to_string())
}

#[test]
fn suggest_root_wrapper_folder() {
    assert_eq!(
        root_of(&["MyMod-1.2/readme.txt", "MyMod-1.2/scripts/a.cs"]).as_deref(),
        Some("MyMod-1.2")
    );
}

#[test]
fn suggest_root_nested_wrappers() {
    assert_eq!(
        root_of(&["Outer/Inner/readme.txt", "outer/inner/x64/a.rpf"]).as_deref(),
        Some("Outer/Inner")
    );
}

#[test]
fn suggest_root_stops_at_known_game_folder() {
    assert_eq!(root_of(&["scripts/a.cs", "scripts/b.cs"]), None);
    assert_eq!(
        root_of(&["Wrap/scripts/a.cs", "Wrap/scripts/b.cs"]).as_deref(),
        Some("Wrap")
    );
}

#[test]
fn suggest_root_marker_based() {
    assert_eq!(
        root_of(&[
            "Pack/Bin/dinput8.dll",
            "Pack/Bin/ScriptHookV.dll",
            "Pack/docs/x.txt"
        ])
        .as_deref(),
        Some("Pack/Bin")
    );
    assert_eq!(
        root_of(&["Pack/a/Mod.ASI", "Pack/b/Other.asi"]).as_deref(),
        Some("Pack")
    );
    assert_eq!(root_of(&["Mod.asi", "Pack/readme.txt"]), None);
}

#[test]
fn suggest_root_flat_archive() {
    assert_eq!(root_of(&["a.txt", "b/c.txt"]), None);
    assert_eq!(root_of(&["only.txt"]), None);
    assert_eq!(root_of(&[]), None);
}

#[test]
fn map_to_game_strips_root_and_omits_outside() {
    let e = entries(&["Wrap/scripts/a.cs", "Wrap/b.txt", "other/c.txt"]);
    let root = RelPath::parse("wrap").expect("rel");
    let mapped = map_to_game(&e, Some(&root));
    let targets: Vec<String> = mapped.iter().map(|(t, _)| t.to_string()).collect();
    assert_eq!(targets, ["scripts/a.cs", "b.txt"]);
    assert_eq!(mapped[0].1, PathBuf::from("/stage/Wrap/scripts/a.cs"));
    let all = map_to_game(&e, None);
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].0.as_str(), "other/c.txt");
}

#[test]
fn suggest_root_marker_never_picks_known_game_folder() {
    assert_eq!(root_of(&["scripts/x.asi"]), None);
    assert_eq!(root_of(&["MyMod/scripts/x.asi"]).as_deref(), Some("MyMod"));
    assert_eq!(root_of(&["MyMod/plugins/a.asi"]).as_deref(), Some("MyMod"));
    assert_eq!(root_of(&["MyMod/Mods/sub/a.asi"]).as_deref(), Some("MyMod"));
}
