//! Integration tests for the installation repository.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, TimeZone, Utc};
use gmm_domain::{
    BaselineTrust, DistributionPlatform, Edition, Installation, InstallationId, InstallationStatus,
    root_key,
};
use gmm_store::{Database, Error};
use tempfile::TempDir;

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(1_800_000_000_000 + secs * 1000 + 123)
        .unwrap()
}

fn sample(root: &str, created: DateTime<Utc>) -> Installation {
    Installation {
        id: InstallationId::new(),
        edition: Edition::Legacy,
        platform: DistributionPlatform::Steam,
        display_name: "GTA V".into(),
        root_path: PathBuf::from(root),
        root_key: root_key(Path::new(root)),
        exe_version: Some("1.0.3095.0".into()),
        status: InstallationStatus::Ok,
        baseline: BaselineTrust::None,
        created_at: created,
        updated_at: created,
    }
}

#[test]
fn round_trip_every_field_and_enum_value() {
    let db = Database::open_in_memory().unwrap();
    let mut n = 0;
    for edition in Edition::ALL {
        for platform in DistributionPlatform::ALL {
            for status in InstallationStatus::ALL {
                for baseline in BaselineTrust::ALL {
                    n += 1;
                    let mut i = sample(&format!("C:\\Games\\GTA{n}"), ts(n));
                    i.edition = edition;
                    i.platform = platform;
                    i.status = status;
                    i.baseline = baseline;
                    i.exe_version = if n % 2 == 0 {
                        None
                    } else {
                        Some("1.2.3".into())
                    };
                    i.updated_at = ts(n + 100);
                    db.insert_installation(&i).unwrap();
                    assert_eq!(db.get_installation(i.id).unwrap(), Some(i));
                }
            }
        }
    }
    assert_eq!(db.list_installations().unwrap().len(), n as usize);
}

#[test]
fn get_missing_is_none() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.get_installation(InstallationId::new()).unwrap(), None);
    assert_eq!(db.find_installation_by_root_key("nope").unwrap(), None);
}

#[test]
fn list_is_ordered_by_created_at_then_id() {
    let db = Database::open_in_memory().unwrap();
    let late = sample("/g/late", ts(10));
    let early = sample("/g/early", ts(1));
    let mut a = sample("/g/same_a", ts(5));
    let mut b = sample("/g/same_b", ts(5));
    if a.id > b.id {
        std::mem::swap(&mut a.id, &mut b.id);
    }
    for i in [&late, &b, &early, &a] {
        db.insert_installation(i).unwrap();
    }
    let ids: Vec<_> = db
        .list_installations()
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec![early.id, a.id, b.id, late.id]);
}

#[test]
fn duplicate_root_key_is_conflict() {
    let db = Database::open_in_memory().unwrap();
    db.insert_installation(&sample("C:\\Games\\GTA", ts(0)))
        .unwrap();
    let dup = sample("c:/games/gta/", ts(1));
    assert!(matches!(
        db.insert_installation(&dup),
        Err(Error::Conflict { .. })
    ));
    assert_eq!(db.list_installations().unwrap().len(), 1);
}

#[test]
fn find_by_root_key() {
    let db = Database::open_in_memory().unwrap();
    let i = sample("D:\\Steam\\GTAV", ts(0));
    db.insert_installation(&i).unwrap();
    db.insert_installation(&sample("D:\\Other", ts(1))).unwrap();
    assert_eq!(
        db.find_installation_by_root_key("d:/steam/gtav").unwrap(),
        Some(i)
    );
}

#[test]
fn update_validation_changes_only_validation_fields() {
    let db = Database::open_in_memory().unwrap();
    let i = sample("/g/a", ts(0));
    db.insert_installation(&i).unwrap();
    let when = ts(0) + Duration::seconds(60);
    db.update_installation_validation(i.id, InstallationStatus::Incomplete, None, when)
        .unwrap();
    let got = db.get_installation(i.id).unwrap().unwrap();
    assert_eq!(got.status, InstallationStatus::Incomplete);
    assert_eq!(got.exe_version, None);
    assert_eq!(got.updated_at, when);
    assert_eq!(got.created_at, i.created_at);
    assert_eq!(got.display_name, i.display_name);
    db.update_installation_validation(i.id, InstallationStatus::Ok, Some("9.9"), when)
        .unwrap();
    let got = db.get_installation(i.id).unwrap().unwrap();
    assert_eq!(got.exe_version.as_deref(), Some("9.9"));
}

#[test]
fn rename_and_not_found() {
    let db = Database::open_in_memory().unwrap();
    let i = sample("/g/a", ts(0));
    db.insert_installation(&i).unwrap();
    let when = ts(30);
    db.rename_installation(i.id, "My GTA", when).unwrap();
    let got = db.get_installation(i.id).unwrap().unwrap();
    assert_eq!(got.display_name, "My GTA");
    assert_eq!(got.updated_at, when);
    assert_eq!(got.status, i.status);

    let ghost = InstallationId::new();
    assert!(matches!(
        db.rename_installation(ghost, "x", when),
        Err(Error::NotFound { .. })
    ));
    assert!(matches!(
        db.update_installation_validation(ghost, InstallationStatus::Ok, None, when),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn corrupt_timestamp_is_reported() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gmm.sqlite3");
    let db = Database::open(&path).unwrap();
    let i = sample("/g/a", ts(0));
    db.insert_installation(&i).unwrap();
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute(
        "UPDATE installations SET created_at = 'yesterday-ish' WHERE id = ?1",
        [i.id.to_string()],
    )
    .unwrap();
    for result in [
        db.get_installation(i.id).map(|_| ()),
        db.list_installations().map(|_| ()),
        db.find_installation_by_root_key(&i.root_key).map(|_| ()),
    ] {
        match result {
            Err(Error::Corrupt {
                table,
                column,
                value,
            }) => {
                assert_eq!((table, column), ("installations", "created_at"));
                assert_eq!(value, "yesterday-ish");
            }
            other => panic!("expected Corrupt, got {other:?}"),
        }
    }
}

#[test]
fn survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gmm.sqlite3");
    let i = sample("/g/a", ts(0));
    Database::open(&path)
        .unwrap()
        .insert_installation(&i)
        .unwrap();
    let db = Database::open(&path).unwrap();
    assert_eq!(db.get_installation(i.id).unwrap(), Some(i));
}
