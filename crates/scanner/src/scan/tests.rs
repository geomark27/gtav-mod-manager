#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::baseline::capture_baseline;
use crate::catalog::Catalogs;
use crate::os::testing::FakeOs;
use gmm_domain::{Edition, RelPath, TransactionId, tmp_name, tomb_name};
use gmm_platform::{StdFs, sha256_bytes};
use gmm_testkit::{FakeInstall, Snapshot};

fn catalog(edition: Edition) -> Catalog {
    Catalogs::load().unwrap().get(edition).clone()
}

fn run(i: &FakeInstall, os: &FakeOs, ctx: &ScanContext) -> ScanOutcome {
    scan(
        &StdFs,
        os,
        &catalog(i.edition()),
        i.root(),
        ctx,
        &mut |_| true,
    )
    .unwrap()
}

fn find<'a>(o: &'a ScanOutcome, rel: &str) -> &'a InventoryItem {
    o.items
        .iter()
        .find(|i| i.rel_path.as_str() == rel)
        .unwrap_or_else(|| panic!("{rel} not scanned"))
}

fn class(o: &ScanOutcome, rel: &str) -> Classification {
    find(o, rel).classification
}

fn modded(edition: Edition) -> FakeInstall {
    FakeInstall::builder(edition)
        .with_base_files()
        .asi_mod("Trainer")
        .script_hook()
        .scripts_mod("Foo")
        .file("mods/update/update.rpf", b"mod rpf".to_vec())
        .file("mods/x64a.rpf", b"mod x64a".to_vec())
        .file("notes.txt", b"hello".to_vec())
        .build()
        .unwrap()
}

#[test]
fn originals_and_candidates_on_both_editions() {
    for edition in Edition::ALL {
        let i = modded(edition);
        let o = run(&i, &FakeOs::new(), &ScanContext::default());
        for f in gmm_testkit::base_files(edition) {
            let item = find(&o, f);
            assert_eq!(item.classification, Classification::Original, "{f}");
            assert!(item.evidence.contains(&Evidence::CatalogOriginal));
            assert_eq!(item.sha256, None);
        }
        let cat = Category::ScriptsGameplay;
        for (f, label, category) in [
            ("Trainer.asi", "ASI plugin", cat),
            ("dinput8.dll", "ASI loader", cat),
            ("ScriptHookV.dll", "Script Hook V", cat),
            ("scripts/Foo.dll", "Scripts folder", cat),
            ("scripts/Foo.ini", "Scripts folder", cat),
            (
                "mods/update/update.rpf",
                "OpenIV mods folder",
                Category::Uncategorized,
            ),
            (
                "mods/x64a.rpf",
                "OpenIV mods folder",
                Category::Uncategorized,
            ),
        ] {
            let item = find(&o, f);
            assert_eq!(item.classification, Classification::Candidate, "{f}");
            assert_eq!(item.suggested_category, Some(category), "{f}");
            assert_eq!(
                item.evidence,
                vec![Evidence::ModPattern {
                    pattern: label.into()
                }],
                "{f}"
            );
        }
        // Unknown evidence and category.
        assert_eq!(class(&o, "notes.txt"), Classification::Unknown);
        assert_eq!(find(&o, "notes.txt").suggested_category, None);
        // `Trainer.ini` has no pattern.
        assert_eq!(class(&o, "Trainer.ini"), Classification::Unknown);
        assert_eq!(o.stats.files, o.items.len() as u64);
        assert_eq!(o.stats.by_class.values().sum::<u64>(), o.stats.files);
        assert_eq!(o.stats.by_class["unknown"], 2);
        assert_eq!(o.stats.bytes_hashed, 0);
    }
}

#[test]
fn false_positives() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .file("vendor.dll", b"signed".to_vec())
        .file("mystery.dll", b"unsigned".to_vec())
        .file("signed.asi", b"signed asi".to_vec())
        .file("other.exe", b"x".to_vec())
        .build()
        .unwrap();
    let os = FakeOs::new()
        .publisher(i.path("vendor.dll"), "Rockstar Games, Inc.")
        .publisher(i.path("signed.asi"), "Take-Two Interactive Software, Inc.")
        .publisher(i.path("other.exe"), "Evil Corp");
    let o = run(&i, &os, &ScanContext::default());
    assert_eq!(class(&o, "bink2w64.dll"), Classification::Original);
    assert_eq!(class(&o, "vendor.dll"), Classification::Original);
    assert_eq!(
        find(&o, "vendor.dll").evidence,
        vec![Evidence::SignedBy {
            publisher: "Rockstar Games, Inc.".into()
        }]
    );
    // A trusted signature beats the *.asi mod pattern.
    assert_eq!(class(&o, "signed.asi"), Classification::Original);
    assert_eq!(class(&o, "mystery.dll"), Classification::Unknown);
    assert_eq!(class(&o, "other.exe"), Classification::Unknown);
}

#[test]
fn other_edition_executable_is_not_original() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .file("GTA5_Enhanced.exe", b"x".to_vec())
        .build()
        .unwrap();
    let o = run(&i, &FakeOs::new(), &ScanContext::default());
    assert_eq!(class(&o, "GTA5_Enhanced.exe"), Classification::Unknown);
}

#[test]
fn engine_artifacts_and_probe_leftovers() {
    let tx = TransactionId::new();
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .file(&tmp_name("Trainer.asi", tx), b"t".to_vec())
        .file(&tomb_name("common.rpf", tx), b"t".to_vec())
        .file("scripts/.gmm-probe-abc123", b"".to_vec())
        .file(".gmm-probe-xyz", b"".to_vec())
        .build()
        .unwrap();
    let o = run(&i, &FakeOs::new(), &ScanContext::default());
    assert_eq!(o.stats.by_class["engine_artifact"], 4);
    assert_eq!(
        class(&o, &tomb_name("common.rpf", tx)),
        Classification::EngineArtifact
    );
}

fn managed_ctx(i: &FakeInstall, rel: &str, content: &[u8], size: u64) -> (ScanContext, ModId) {
    let mod_id = ModId::new();
    let mut ctx = ScanContext::default();
    ctx.managed.insert(
        RelPath::parse(rel).unwrap().key(),
        ManagedRef {
            mod_id,
            sha256: Sha256::parse(&sha256_bytes(content)).unwrap(),
            size,
        },
    );
    let _ = i;
    (ctx, mod_id)
}

#[test]
fn managed_matching_hash() {
    let i = modded(Edition::Legacy);
    let content = i.read("Trainer.asi").unwrap();
    let (ctx, mod_id) = managed_ctx(&i, "trainer.ASI", &content, content.len() as u64);
    let o = run(&i, &FakeOs::new(), &ctx);
    let item = find(&o, "Trainer.asi");
    assert_eq!(item.classification, Classification::Managed);
    assert_eq!(item.evidence, vec![Evidence::ManagedBy { mod_id }]);
    assert_eq!(
        item.sha256.as_ref().map(Sha256::as_str),
        Some(sha256_bytes(&content).as_str())
    );
    assert_eq!(o.stats.bytes_hashed, content.len() as u64);
}

#[test]
fn managed_modified_same_size_is_drifted() {
    let i = modded(Edition::Legacy);
    let content = i.read("Trainer.asi").unwrap();
    let (ctx, _) = managed_ctx(&i, "Trainer.asi", &content, content.len() as u64);
    let mut changed = content.clone();
    changed[0] ^= 0xff;
    i.write("Trainer.asi", &changed).unwrap();
    let o = run(&i, &FakeOs::new(), &ctx);
    assert_eq!(class(&o, "Trainer.asi"), Classification::Drifted);
    assert_eq!(o.stats.bytes_hashed, changed.len() as u64);
}

#[test]
fn managed_size_change_is_drifted_without_hashing() {
    let i = modded(Edition::Legacy);
    let content = i.read("Trainer.asi").unwrap();
    let (ctx, _) = managed_ctx(&i, "Trainer.asi", &content, content.len() as u64 + 1);
    let o = run(&i, &FakeOs::new(), &ctx);
    let item = find(&o, "Trainer.asi");
    assert_eq!(item.classification, Classification::Drifted);
    assert_eq!(item.sha256, None);
    assert_eq!(o.stats.bytes_hashed, 0);
}

fn baseline_ctx(i: &FakeInstall, trust: BaselineTrust) -> ScanContext {
    let entries = capture_baseline(&StdFs, i.root(), &mut |_| true).unwrap();
    let mut ctx = ScanContext {
        baseline_trust: trust,
        ..ScanContext::default()
    };
    for e in entries {
        ctx.baseline.insert(
            e.rel_path.key(),
            BaselineRef {
                size: e.size,
                sha256: e.sha256,
            },
        );
    }
    ctx
}

#[test]
fn trusted_baseline_detects_modified_originals() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .file("custom.dat", b"in baseline".to_vec())
        .build()
        .unwrap();
    let ctx = baseline_ctx(&i, BaselineTrust::Trusted);
    // Same size, different content.
    let mut rpf = i.read("update/update.rpf").unwrap();
    rpf[10] ^= 0xff;
    i.write("update/update.rpf", &rpf).unwrap();
    // Size change.
    i.write("common.rpf", b"short").unwrap();
    let o = run(&i, &FakeOs::new(), &ctx);
    for f in ["update/update.rpf", "common.rpf"] {
        let item = find(&o, f);
        assert_eq!(item.classification, Classification::OriginalModified, "{f}");
        assert!(item.evidence.contains(&Evidence::BaselineMismatch));
        assert!(item.evidence.contains(&Evidence::CatalogOriginal));
    }
    let ok = find(&o, "x64a.rpf");
    assert_eq!(ok.classification, Classification::Original);
    assert!(ok.evidence.contains(&Evidence::BaselineMatch));
    // In a trusted baseline but not in the catalog.
    let custom = find(&o, "custom.dat");
    assert_eq!(custom.classification, Classification::Original);
    assert_eq!(custom.evidence, vec![Evidence::BaselineMatch]);
}

#[test]
fn untrusted_baseline_never_changes_the_class() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .file("custom.dat", b"in baseline".to_vec())
        .asi_mod("Trainer")
        .build()
        .unwrap();
    let ctx = baseline_ctx(&i, BaselineTrust::Untrusted);
    let mut rpf = i.read("update/update.rpf").unwrap();
    rpf[10] ^= 0xff;
    i.write("update/update.rpf", &rpf).unwrap();
    let o = run(&i, &FakeOs::new(), &ctx);
    assert!(o.items.iter().all(|i| {
        i.classification != Classification::OriginalModified
            && !i.evidence.contains(&Evidence::BaselineMismatch)
    }));
    let rpf_item = find(&o, "update/update.rpf");
    assert_eq!(rpf_item.classification, Classification::Original);
    assert_eq!(rpf_item.evidence, vec![Evidence::CatalogOriginal]);
    // Information only: match evidence, class from the other rules.
    let custom = find(&o, "custom.dat");
    assert_eq!(custom.classification, Classification::Unknown);
    assert_eq!(custom.evidence, vec![Evidence::BaselineMatch]);
    let asi = find(&o, "Trainer.asi");
    assert_eq!(asi.classification, Classification::Candidate);
    assert!(asi.evidence.contains(&Evidence::BaselineMatch));
}

fn cache_of(o: &ScanOutcome) -> HashMap<String, CachedHash> {
    o.items
        .iter()
        .filter_map(|i| {
            Some((
                i.rel_path.key(),
                CachedHash {
                    size: i.size,
                    mtime_ns: i.mtime_ns,
                    sha256: i.sha256.clone()?,
                },
            ))
        })
        .collect()
}

#[test]
fn hash_cache_avoids_rehashing_until_mtime_changes() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .build()
        .unwrap();
    let mut ctx = baseline_ctx(&i, BaselineTrust::Trusted);
    let first = run(&i, &FakeOs::new(), &ctx);
    assert!(first.stats.bytes_hashed > 0);
    ctx.hash_cache = cache_of(&first);
    assert_eq!(ctx.hash_cache.len(), first.items.len());
    let second = run(&i, &FakeOs::new(), &ctx);
    assert_eq!(second.stats.bytes_hashed, 0);
    assert!(second.items.iter().all(|i| i.sha256.is_some()));
    // Touch one file: same size, same content, newer mtime.
    let f = std::fs::File::options()
        .write(true)
        .open(i.path("common.rpf"))
        .unwrap();
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap();
    drop(f);
    let third = run(&i, &FakeOs::new(), &ctx);
    assert_eq!(third.stats.bytes_hashed, 4096);
    assert_eq!(class(&third, "common.rpf"), Classification::Original);
}

#[cfg(unix)]
#[test]
fn symlinks_and_hard_links_are_never_candidates() {
    let i = modded(Edition::Legacy);
    std::os::unix::fs::symlink(i.path("notes.txt"), i.path("link.asi")).unwrap();
    std::os::unix::fs::symlink(i.path("mods"), i.path("dirlink")).unwrap();
    std::fs::hard_link(i.path("Trainer.asi"), i.path("Copy.asi")).unwrap();
    let o = run(&i, &FakeOs::new(), &ScanContext::default());
    let link = find(&o, "link.asi");
    assert_eq!(link.classification, Classification::Unknown);
    assert!(link.evidence.contains(&Evidence::ReparsePoint));
    assert!(
        link.evidence
            .iter()
            .any(|e| matches!(e, Evidence::ModPattern { .. }))
    );
    assert_eq!(link.sha256, None);
    // The linked directory is an item and is not descended.
    let dir = find(&o, "dirlink");
    assert!(dir.evidence.contains(&Evidence::ReparsePoint));
    assert!(
        o.items
            .iter()
            .all(|i| !i.rel_path.as_str().starts_with("dirlink/"))
    );
    for f in ["Trainer.asi", "Copy.asi"] {
        let item = find(&o, f);
        assert_eq!(item.classification, Classification::Unknown, "{f}");
        assert!(item.evidence.contains(&Evidence::HardLinked), "{f}");
    }
    // A reparse point in the managed set is drifted, never managed.
    let (ctx, _) = managed_ctx(&i, "link.asi", b"hello", 5);
    let o = run(&i, &FakeOs::new(), &ctx);
    assert_eq!(class(&o, "link.asi"), Classification::Drifted);
}

#[test]
fn cancellation_and_progress() {
    let mut b = FakeInstall::builder(Edition::Legacy).with_base_files();
    for n in 0..300 {
        b = b.file(&format!("many/f{n:03}.txt"), b"x".to_vec());
    }
    let i = b.build().unwrap();
    let cat = catalog(Edition::Legacy);
    let ctx = ScanContext::default();
    let mut seen = Vec::new();
    let o = scan(&StdFs, &FakeOs::new(), &cat, i.root(), &ctx, &mut |p| {
        seen.push(p.files_seen);
        true
    })
    .unwrap();
    assert_eq!(o.stats.files, 306);
    assert!(seen.contains(&256));
    assert_eq!(seen.last(), Some(&306));
    // Cancel at the 256-file tick.
    let r = scan(&StdFs, &FakeOs::new(), &cat, i.root(), &ctx, &mut |p| {
        p.files_seen < 256
    });
    assert!(matches!(r, Err(Error::Cancelled)));
    // Cancel while hashing.
    let base = baseline_ctx(&i, BaselineTrust::Trusted);
    let r = scan(&StdFs, &FakeOs::new(), &cat, i.root(), &base, &mut |p| {
        p.bytes_hashed == 0
    });
    assert!(matches!(r, Err(Error::Cancelled)));
}

#[test]
fn ordering_is_deterministic_and_depth_first() {
    let i = modded(Edition::Enhanced);
    let a = run(&i, &FakeOs::new(), &ScanContext::default());
    let b = run(&i, &FakeOs::new(), &ScanContext::default());
    let names = |o: &ScanOutcome| {
        o.items
            .iter()
            .map(|i| i.rel_path.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&a), names(&b));
    let n = names(&a);
    let pos = |s: &str| n.iter().position(|x| x == s).unwrap();
    assert!(pos("common.rpf") < pos("mods/update/update.rpf"));
    assert!(pos("mods/update/update.rpf") < pos("mods/x64a.rpf"));
    assert!(pos("mods/x64a.rpf") < pos("notes.txt"));
}

#[test]
fn scan_never_modifies_the_folder() {
    let i = modded(Edition::Legacy);
    let ctx = baseline_ctx(&i, BaselineTrust::Trusted);
    let before = Snapshot::of(i.root()).unwrap();
    let _ = run(&i, &FakeOs::new(), &ctx);
    assert_eq!(before, Snapshot::of(i.root()).unwrap());
}

#[test]
fn with_owner_stamps_items() {
    let i = modded(Edition::Legacy);
    let (inst, scan_id) = (InstallationId::new(), ScanId::new());
    let o = run(&i, &FakeOs::new(), &ScanContext::default()).with_owner(inst, scan_id);
    assert!(
        o.items
            .iter()
            .all(|i| i.installation_id == inst && i.last_scan_id == Some(scan_id))
    );
}

#[test]
fn unparsable_names_are_counted() {
    let i = modded(Edition::Legacy);
    // `a:b` is valid on Linux but forbidden in a RelPath; `CON` is a reserved device name.
    std::fs::write(i.root().join("a:b.txt"), b"x").unwrap();
    std::fs::write(i.root().join("CON"), b"x").unwrap();
    let o = run(&i, &FakeOs::new(), &ScanContext::default());
    assert_eq!(o.stats.skipped_unparsable, 2);
    assert!(o.items.iter().all(|i| i.rel_path.as_str() != "CON"));
}
