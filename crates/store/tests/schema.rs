//! Integration tests for connection setup and the schema.
#![allow(clippy::expect_used, clippy::panic)]

use gmm_store::{Database, Error, LATEST_SCHEMA_VERSION};
use rusqlite::Connection;
use tempfile::TempDir;

const TS: &str = "'2026-11-18T00:00:00.000Z'";
const INST: &str = "'00000000-0000-7000-8000-000000000001'";
const MOD: &str = "'00000000-0000-7000-8000-000000000002'";
const TX: &str = "'00000000-0000-7000-8000-000000000003'";
const HASH: &str = "'0000000000000000000000000000000000000000000000000000000000000000'";

fn fresh() -> (TempDir, std::path::PathBuf, Database) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("gmm.sqlite3");
    let db = Database::open(&path).expect("open");
    (dir, path, db)
}

fn raw(path: &std::path::Path) -> Connection {
    let c = Connection::open(path).expect("raw open");
    c.execute_batch("PRAGMA foreign_keys = ON;").expect("fk");
    c
}

fn insert_installation(c: &Connection) {
    c.execute_batch(&format!(
        "INSERT INTO installations (id, edition, platform, display_name, root_path, root_key, status, created_at, updated_at)
         VALUES ({INST}, 'legacy', 'steam', 'Test', 'X', 'x', 'ok', {TS}, {TS});"
    ))
    .expect("installation");
}

fn insert_mod(c: &Connection) {
    c.execute_batch(&format!(
        "INSERT INTO mods (id, installation_id, name, category, state, created_at, updated_at)
         VALUES ({MOD}, {INST}, 'M', 'vehicles', 'disabled', {TS}, {TS});"
    ))
    .expect("mod");
}

fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count")
}

#[test]
fn fresh_database_has_version_and_tables() {
    let (_d, path, db) = fresh();
    assert_eq!(db.schema_version().expect("v"), 2);
    assert_eq!(LATEST_SCHEMA_VERSION, 2);
    let c = raw(&path);
    let mut stmt = c
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("prepare");
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    let mut expected = vec![
        "installations",
        "scans",
        "inventory_items",
        "baseline_entries",
        "vault_objects",
        "mods",
        "mod_entries",
        "ownership_confirmations",
        "profiles",
        "profile_entries",
        "transactions",
        "transaction_steps",
        "settings",
        "mod_dependencies",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
}

#[test]
fn pragmas_are_applied() {
    let (_d, _p, db) = fresh();
    let s = db.pragma_snapshot().expect("snapshot");
    assert_eq!(s.journal_mode, "wal");
    assert_eq!(s.synchronous, 2);
    assert!(s.foreign_keys);
}

#[test]
fn reopen_is_idempotent_and_preserves_data() {
    let (_d, path, db) = fresh();
    insert_installation(&raw(&path));
    drop(db);
    let db = Database::open(&path).expect("reopen");
    assert_eq!(db.schema_version().expect("v"), 2);
    assert_eq!(count(&raw(&path), "installations"), 1);
}

#[test]
fn newer_schema_is_rejected() {
    let (_d, path, db) = fresh();
    drop(db);
    raw(&path)
        .execute_batch("PRAGMA user_version = 99;")
        .expect("set version");
    match Database::open(&path) {
        Err(Error::SchemaTooNew {
            found: 99,
            supported: 2,
        }) => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn missing_parent_dir_is_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nope").join("gmm.sqlite3");
    assert!(matches!(
        Database::open(&path),
        Err(Error::ParentDirMissing { .. })
    ));
}

#[test]
fn check_constraints_reject_bad_values() {
    let (_d, path, _db) = fresh();
    let c = raw(&path);
    let bad = c.execute_batch(&format!(
        "INSERT INTO installations (id, edition, platform, display_name, root_path, root_key, status, created_at, updated_at)
         VALUES ({INST}, 'online', 'steam', 'T', 'X', 'x', 'ok', {TS}, {TS});"
    ));
    assert!(bad.is_err());
    insert_installation(&c);
    let bad = c.execute_batch(&format!(
        "INSERT INTO mods (id, installation_id, name, category, state, created_at, updated_at)
         VALUES ({MOD}, {INST}, 'M', 'weapons', 'disabled', {TS}, {TS});"
    ));
    assert!(bad.is_err());
    c.execute_batch(&format!(
        "INSERT INTO transactions (id, installation_id, kind, status, created_at)
         VALUES ({TX}, {INST}, 'enable', 'open', {TS});"
    ))
    .expect("tx");
    let bad = c.execute_batch(&format!(
        "INSERT INTO transaction_steps (transaction_id, seq, kind, rel_path, status, updated_at)
         VALUES ({TX}, 0, 'ensure_dir', 'a', 'running', {TS});"
    ));
    assert!(bad.is_err());
}

#[test]
fn foreign_key_is_enforced() {
    let (_d, path, _db) = fresh();
    let bad = raw(&path).execute_batch(&format!(
        "INSERT INTO mods (id, installation_id, name, category, state, created_at, updated_at)
         VALUES ({MOD}, {INST}, 'M', 'vehicles', 'disabled', {TS}, {TS});"
    ));
    assert!(bad.is_err());
}

#[test]
fn delete_installation_cascades() {
    let (_d, path, _db) = fresh();
    let c = raw(&path);
    insert_installation(&c);
    insert_mod(&c);
    c.execute_batch(&format!(
        "INSERT INTO vault_objects (sha256, size, created_at, verified_at) VALUES ({HASH}, 1, {TS}, {TS});
         INSERT INTO mod_entries (id, mod_id, rel_path, path_key, sha256, size)
           VALUES ('00000000-0000-7000-8000-000000000004', {MOD}, 'A.dll', 'a.dll', {HASH}, 1);
         INSERT INTO profiles (id, installation_id, name, created_at, updated_at)
           VALUES ('00000000-0000-7000-8000-000000000005', {INST}, 'P', {TS}, {TS});
         INSERT INTO inventory_items (installation_id, path_key, rel_path, size, mtime_ns, classification, first_seen_at, last_seen_at)
           VALUES ({INST}, 'a.dll', 'A.dll', 1, 0, 'unknown', {TS}, {TS});"
    ))
    .expect("seed");
    c.execute_batch(&format!("DELETE FROM installations WHERE id = {INST};"))
        .expect("delete");
    for t in ["mods", "mod_entries", "profiles", "inventory_items"] {
        assert_eq!(count(&c, t), 0, "{t}");
    }
}

#[test]
fn delete_installation_with_transaction_is_restricted() {
    let (_d, path, _db) = fresh();
    let c = raw(&path);
    insert_installation(&c);
    c.execute_batch(&format!(
        "INSERT INTO transactions (id, installation_id, kind, status, created_at)
         VALUES ({TX}, {INST}, 'enable', 'open', {TS});"
    ))
    .expect("tx");
    assert!(
        c.execute_batch(&format!("DELETE FROM installations WHERE id = {INST};"))
            .is_err()
    );
    assert_eq!(count(&c, "installations"), 1);
}

#[test]
fn in_memory_reaches_latest_version() {
    let db = Database::open_in_memory().expect("open");
    assert_eq!(db.schema_version().expect("v"), 2);
    assert!(db.pragma_snapshot().expect("snap").foreign_keys);
}

#[test]
fn migration_from_v1_preserves_data_and_adds_dependencies() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("gmm.sqlite3");
    {
        let c = raw(&path);
        c.execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("v1 schema");
        c.execute_batch("PRAGMA user_version = 1;")
            .expect("version");
        insert_installation(&c);
        insert_mod(&c);
    }
    let db = Database::open(&path).expect("migrate");
    assert_eq!(db.schema_version().expect("v"), 2);
    let c = raw(&path);
    assert_eq!(count(&c, "installations"), 1);
    assert_eq!(count(&c, "mods"), 1);
    assert_eq!(count(&c, "mod_dependencies"), 0);
    let self_dep = c.execute_batch(&format!(
        "INSERT INTO mod_dependencies VALUES ({MOD}, {MOD}, {TS});"
    ));
    assert!(self_dep.is_err(), "self dependency must violate the CHECK");
}
