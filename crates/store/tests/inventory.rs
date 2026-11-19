//! Integration tests for scans, inventory, baseline and managed entries.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, TimeZone, Utc};
use gmm_domain::{
    BaselineTrust, Category, Classification, DistributionPlatform, Edition, Evidence, Installation,
    InstallationId, InstallationStatus, InventoryItem, ModId, RelPath, Sha256, root_key,
};
use gmm_store::{Database, Error, InventoryFilter, ScanStatus};

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(1_800_000_000_000 + secs * 1000)
        .unwrap()
}

fn setup() -> (Database, InstallationId) {
    let db = Database::open_in_memory().unwrap();
    let id = InstallationId::new();
    let root = "C:/Games/Test";
    db.insert_installation(&Installation {
        id,
        edition: Edition::Legacy,
        platform: DistributionPlatform::Steam,
        display_name: "T".into(),
        root_path: PathBuf::from(root),
        root_key: root_key(Path::new(root)),
        exe_version: None,
        status: InstallationStatus::Ok,
        baseline: BaselineTrust::None,
        created_at: ts(0),
        updated_at: ts(0),
    })
    .unwrap();
    (db, id)
}

fn sha(n: u8) -> Sha256 {
    Sha256::from_bytes([n; 32])
}

fn item(inst: InstallationId, path: &str, class: Classification) -> InventoryItem {
    InventoryItem {
        installation_id: inst,
        rel_path: RelPath::parse(path).unwrap(),
        size: 10,
        mtime_ns: 5,
        sha256: None,
        classification: class,
        suggested_category: None,
        evidence: vec![],
        first_seen_at: ts(0),
        last_seen_at: ts(0),
        last_scan_id: None,
    }
}

fn all() -> InventoryFilter {
    InventoryFilter {
        limit: 500,
        ..InventoryFilter::default()
    }
}

#[test]
fn scan_lifecycle_and_last_scan() {
    let (db, inst) = setup();
    assert!(db.last_scan(inst).unwrap().is_none());
    let s1 = db.start_scan(inst, ts(1)).unwrap();
    let r = db.last_scan(inst).unwrap().unwrap();
    assert_eq!(r.id, s1);
    assert_eq!(r.status, ScanStatus::Running);
    assert_eq!(r.finished_at, None);
    db.finish_scan(s1, ScanStatus::Completed, 42, None, ts(2))
        .unwrap();
    let s2 = db.start_scan(inst, ts(10)).unwrap();
    db.finish_scan(s2, ScanStatus::Failed, 3, Some("boom"), ts(11))
        .unwrap();
    let r = db.last_scan(inst).unwrap().unwrap();
    assert_eq!(r.id, s2);
    assert_eq!(r.status, ScanStatus::Failed);
    assert_eq!(r.files_seen, 3);
    assert_eq!(r.error.as_deref(), Some("boom"));
    assert_eq!(r.finished_at, Some(ts(11)));
    assert_eq!(r.started_at, ts(10));
    assert!(matches!(
        db.finish_scan(
            gmm_domain::ScanId::new(),
            ScanStatus::Cancelled,
            0,
            None,
            ts(1)
        ),
        Err(Error::NotFound { .. })
    ));
    assert_eq!(ScanStatus::Cancelled.as_str(), "cancelled");
    assert!(db.start_scan(InstallationId::new(), ts(1)).is_err());
}

#[test]
fn replace_inventory_keeps_first_seen_and_deletes_missing() {
    let (db, inst) = setup();
    let s1 = db.start_scan(inst, ts(1)).unwrap();
    let a = item(inst, "a.asi", Classification::Candidate);
    let b = item(inst, "Dir/B.dll", Classification::Unknown);
    let c = item(inst, "c.txt", Classification::Original);
    db.replace_inventory(inst, s1, &[a.clone(), b.clone(), c], ts(100))
        .unwrap();
    assert_eq!(db.list_inventory(inst, &all()).unwrap().total, 3);

    let s2 = db.start_scan(inst, ts(200)).unwrap();
    let mut b2 = b.clone();
    b2.size = 99;
    b2.classification = Classification::Drifted;
    // Same path with different case maps to the same key.
    b2.rel_path = RelPath::parse("dir/b.DLL").unwrap();
    db.replace_inventory(inst, s2, &[a.clone(), b2], ts(300))
        .unwrap();

    let page = db.list_inventory(inst, &all()).unwrap();
    assert_eq!(page.total, 2);
    let got_a = &page.items[0];
    let got_b = &page.items[1];
    assert_eq!(got_a.rel_path.as_str(), "a.asi");
    assert_eq!(got_a.first_seen_at, ts(100));
    assert_eq!(got_a.last_seen_at, ts(300));
    assert_eq!(got_a.last_scan_id, Some(s2));
    assert_eq!(got_b.rel_path.as_str(), "dir/b.DLL");
    assert_eq!(got_b.size, 99);
    assert_eq!(got_b.classification, Classification::Drifted);
    assert_eq!(got_b.first_seen_at, ts(100));
    assert_eq!(got_b.last_seen_at, ts(300));
}

#[test]
fn replace_inventory_does_not_touch_other_installations() {
    let (db, inst) = setup();
    let other = InstallationId::new();
    let root = "C:/Games/Other";
    db.insert_installation(&Installation {
        id: other,
        edition: Edition::Enhanced,
        platform: DistributionPlatform::Epic,
        display_name: "O".into(),
        root_path: PathBuf::from(root),
        root_key: root_key(Path::new(root)),
        exe_version: None,
        status: InstallationStatus::Ok,
        baseline: BaselineTrust::None,
        created_at: ts(0),
        updated_at: ts(0),
    })
    .unwrap();
    let so = db.start_scan(other, ts(1)).unwrap();
    db.replace_inventory(
        other,
        so,
        &[item(other, "x.dll", Classification::Unknown)],
        ts(2),
    )
    .unwrap();
    let s = db.start_scan(inst, ts(3)).unwrap();
    db.replace_inventory(inst, s, &[], ts(4)).unwrap();
    assert_eq!(db.list_inventory(other, &all()).unwrap().total, 1);
}

#[test]
fn evidence_and_optional_fields_round_trip() {
    let (db, inst) = setup();
    let s = db.start_scan(inst, ts(1)).unwrap();
    let mod_id = ModId::new();
    let evidence = vec![
        Evidence::CatalogOriginal,
        Evidence::BaselineMatch,
        Evidence::BaselineMismatch,
        Evidence::SignedBy {
            publisher: "Acme \"Co\" % _".into(),
        },
        Evidence::Unsigned,
        Evidence::ModPattern {
            pattern: "asi".into(),
        },
        Evidence::ReparsePoint,
        Evidence::HardLinked,
        Evidence::ManagedBy { mod_id },
    ];
    let mut full = item(inst, "full.dll", Classification::Managed);
    full.evidence = evidence.clone();
    full.sha256 = Some(sha(7));
    full.suggested_category = Some(Category::ScriptsGameplay);
    full.mtime_ns = -123;
    let bare = item(inst, "bare.dll", Classification::Unknown);
    db.replace_inventory(inst, s, &[full.clone(), bare], ts(5))
        .unwrap();
    let page = db.list_inventory(inst, &all()).unwrap();
    let bare_got = &page.items[0];
    let full_got = &page.items[1];
    assert_eq!(bare_got.sha256, None);
    assert_eq!(bare_got.suggested_category, None);
    assert!(bare_got.evidence.is_empty());
    assert_eq!(full_got.evidence, evidence);
    assert_eq!(full_got.sha256, Some(sha(7)));
    assert_eq!(full_got.suggested_category, Some(Category::ScriptsGameplay));
    assert_eq!(full_got.mtime_ns, -123);
}

#[test]
fn inventory_for_unknown_installation_fails() {
    let db = Database::open_in_memory().unwrap();
    let ghost = InstallationId::new();
    assert!(db.start_scan(ghost, ts(1)).is_err());
    let (db2, inst) = setup();
    let s = db2.start_scan(inst, ts(1)).unwrap();
    let r = db2.replace_inventory(
        ghost,
        s,
        &[item(ghost, "a.dll", Classification::Unknown)],
        ts(2),
    );
    assert!(matches!(r, Err(Error::Sqlite(_))));
    assert_eq!(db2.list_inventory(ghost, &all()).unwrap().total, 0);
}

#[test]
fn replace_inventory_is_atomic_on_failure() {
    let (db, inst) = setup();
    let s1 = db.start_scan(inst, ts(1)).unwrap();
    db.replace_inventory(
        inst,
        s1,
        &[item(inst, "keep.dll", Classification::Unknown)],
        ts(2),
    )
    .unwrap();
    // Unknown scan id violates the last_scan_id foreign key: nothing may change.
    let r = db.replace_inventory(
        inst,
        gmm_domain::ScanId::new(),
        &[item(inst, "new.dll", Classification::Unknown)],
        ts(3),
    );
    assert!(r.is_err());
    let page = db.list_inventory(inst, &all()).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].rel_path.as_str(), "keep.dll");
}

#[test]
fn performance_smoke_20000_items() {
    let (db, inst) = setup();
    let s = db.start_scan(inst, ts(1)).unwrap();
    let items: Vec<InventoryItem> = (0..20_000)
        .map(|i| {
            let mut it = item(
                inst,
                &format!("mods/dir{}/file{i}.dat", i % 100),
                Classification::Unknown,
            );
            it.sha256 = Some(sha((i % 250) as u8));
            it.evidence = vec![Evidence::Unsigned];
            it
        })
        .collect();
    let t = Instant::now();
    db.replace_inventory(inst, s, &items, ts(2)).unwrap();
    let first = t.elapsed();
    let s2 = db.start_scan(inst, ts(3)).unwrap();
    let t = Instant::now();
    db.replace_inventory(inst, s2, &items, ts(4)).unwrap();
    let second = t.elapsed();
    assert!(first.as_secs_f64() < 2.0, "first replace took {first:?}");
    assert!(second.as_secs_f64() < 2.0, "second replace took {second:?}");
    assert_eq!(db.hash_cache(inst).unwrap().len(), 20_000);
}

#[test]
fn list_filter_search_and_pagination() {
    let (db, inst) = setup();
    let s = db.start_scan(inst, ts(1)).unwrap();
    let items = vec![
        item(inst, "a.dll", Classification::Original),
        item(inst, "b_c.asi", Classification::Candidate),
        item(inst, "bxc.asi", Classification::Candidate),
        item(inst, "100%/Real.ASI", Classification::Unknown),
        item(
            inst,
            "back\\slash.txt".replace('\\', "/").as_str(),
            Classification::Unknown,
        ),
        item(inst, "z.dll", Classification::Unknown),
    ];
    db.replace_inventory(inst, s, &items, ts(2)).unwrap();

    let cand = db
        .list_inventory(
            inst,
            &InventoryFilter {
                classifications: vec![Classification::Candidate],
                ..all()
            },
        )
        .unwrap();
    assert_eq!(cand.total, 2);

    let multi = db
        .list_inventory(
            inst,
            &InventoryFilter {
                classifications: vec![Classification::Original, Classification::Candidate],
                ..all()
            },
        )
        .unwrap();
    assert_eq!(multi.total, 3);

    let search = |q: &str| {
        db.list_inventory(
            inst,
            &InventoryFilter {
                search: Some(q.to_owned()),
                ..all()
            },
        )
        .unwrap()
    };
    assert_eq!(search("REAL").total, 1);
    assert_eq!(search("%").total, 1);
    assert_eq!(search("b_c").total, 1);
    assert_eq!(search("b_c").items[0].rel_path.as_str(), "b_c.asi");
    assert_eq!(search("\\").total, 0);
    assert_eq!(search(".asi").total, 3);
    assert_eq!(search("").total, 6);

    let combined = db
        .list_inventory(
            inst,
            &InventoryFilter {
                classifications: vec![Classification::Unknown],
                search: Some(".dll".into()),
                ..all()
            },
        )
        .unwrap();
    assert_eq!(combined.total, 1);

    let p1 = db
        .list_inventory(
            inst,
            &InventoryFilter {
                limit: 4,
                offset: 0,
                ..all()
            },
        )
        .unwrap();
    let p2 = db
        .list_inventory(
            inst,
            &InventoryFilter {
                limit: 4,
                offset: 4,
                ..all()
            },
        )
        .unwrap();
    assert_eq!((p1.total, p1.items.len()), (6, 4));
    assert_eq!((p2.total, p2.items.len()), (6, 2));
    let keys: Vec<String> = p1
        .items
        .iter()
        .chain(&p2.items)
        .map(|i| i.rel_path.key())
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);

    // limit is capped at 500
    let big = db
        .list_inventory(
            inst,
            &InventoryFilter {
                limit: 10_000,
                ..all()
            },
        )
        .unwrap();
    assert_eq!(big.items.len(), 6);
}

#[test]
fn counts_and_hash_cache() {
    let (db, inst) = setup();
    let s = db.start_scan(inst, ts(1)).unwrap();
    let mut hashed = item(inst, "h.dll", Classification::Unknown);
    hashed.sha256 = Some(sha(1));
    hashed.size = 77;
    hashed.mtime_ns = 999;
    db.replace_inventory(
        inst,
        s,
        &[
            item(inst, "a", Classification::Original),
            item(inst, "b", Classification::Original),
            hashed,
            item(inst, "c", Classification::Candidate),
        ],
        ts(2),
    )
    .unwrap();
    let counts = db.inventory_counts(inst).unwrap();
    assert_eq!(
        counts,
        vec![
            (Classification::Candidate, 1),
            (Classification::Original, 2),
            (Classification::Unknown, 1),
        ]
    );
    let cache = db.hash_cache(inst).unwrap();
    assert_eq!(cache, vec![("h.dll".to_owned(), 77, 999, sha(1))]);
    assert!(
        db.inventory_counts(InstallationId::new())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn baseline_captured_at_reports_latest_capture() {
    let (db, inst) = setup();
    assert_eq!(db.baseline_captured_at(inst).unwrap(), None);
    let entries = vec![(RelPath::parse("GTA5.exe").unwrap(), 100, sha(1))];
    db.replace_baseline(inst, &entries, BaselineTrust::Trusted, ts(50))
        .unwrap();
    assert_eq!(db.baseline_captured_at(inst).unwrap(), Some(ts(50)));
    db.clear_baseline(inst, ts(60)).unwrap();
    assert_eq!(db.baseline_captured_at(inst).unwrap(), None);
}

#[test]
fn baseline_replace_load_clear() {
    let (db, inst) = setup();
    assert!(db.load_baseline(inst).unwrap().is_empty());
    let entries = vec![
        (RelPath::parse("GTA5.exe").unwrap(), 100, sha(1)),
        (RelPath::parse("x64/a.rpf").unwrap(), 200, sha(2)),
    ];
    db.replace_baseline(inst, &entries, BaselineTrust::Trusted, ts(50))
        .unwrap();
    let loaded = db.load_baseline(inst).unwrap();
    assert_eq!(
        loaded,
        vec![
            ("gta5.exe".to_owned(), 100, sha(1)),
            ("x64/a.rpf".to_owned(), 200, sha(2)),
        ]
    );
    let i = db.get_installation(inst).unwrap().unwrap();
    assert_eq!(i.baseline, BaselineTrust::Trusted);
    assert_eq!(i.updated_at, ts(50));

    db.replace_baseline(
        inst,
        &[(RelPath::parse("new.dll").unwrap(), 5, sha(3))],
        BaselineTrust::Untrusted,
        ts(60),
    )
    .unwrap();
    assert_eq!(db.load_baseline(inst).unwrap().len(), 1);
    assert_eq!(
        db.get_installation(inst).unwrap().unwrap().baseline,
        BaselineTrust::Untrusted
    );

    db.clear_baseline(inst, ts(70)).unwrap();
    assert!(db.load_baseline(inst).unwrap().is_empty());
    let i = db.get_installation(inst).unwrap().unwrap();
    assert_eq!(i.baseline, BaselineTrust::None);
    assert_eq!(i.updated_at, ts(70));

    assert!(matches!(
        db.replace_baseline(InstallationId::new(), &[], BaselineTrust::None, ts(1)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn managed_entries_join() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("gmm.sqlite3");
    let db = Database::open(&path).unwrap();
    let inst = InstallationId::new();
    let root = "C:/Games/Test";
    db.insert_installation(&Installation {
        id: inst,
        edition: Edition::Legacy,
        platform: DistributionPlatform::Steam,
        display_name: "T".into(),
        root_path: PathBuf::from(root),
        root_key: root_key(Path::new(root)),
        exe_version: None,
        status: InstallationStatus::Ok,
        baseline: BaselineTrust::None,
        created_at: ts(0),
        updated_at: ts(0),
    })
    .unwrap();
    assert!(db.managed_entries(inst).unwrap().is_empty());

    // Raw SQL: mods have no repository yet (M4).
    let mod_id = ModId::new();
    let hash = sha(9);
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    raw.execute(
        "INSERT INTO vault_objects (sha256, size, created_at, verified_at) VALUES (?1, 321, 'x', 'x')",
        [hash.as_str()],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO mods (id, installation_id, name, category, state, created_at, updated_at) \
         VALUES (?1, ?2, 'M', 'uncategorized', 'enabled', 'x', 'x')",
        [mod_id.to_string(), inst.to_string()],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO mod_entries (id, mod_id, rel_path, path_key, sha256, size) \
         VALUES (?1, ?2, 'Scripts/Mod.asi', 'scripts/mod.asi', ?3, 321)",
        [
            gmm_domain::ModEntryId::new().to_string(),
            mod_id.to_string(),
            hash.to_string(),
        ],
    )
    .unwrap();
    drop(raw);

    let rows = db.managed_entries(inst).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mod_id, mod_id);
    assert_eq!(rows[0].rel_path.as_str(), "Scripts/Mod.asi");
    assert_eq!(rows[0].sha256, hash);
    assert_eq!(rows[0].size, 321);
    assert!(
        db.managed_entries(InstallationId::new())
            .unwrap()
            .is_empty()
    );
}
