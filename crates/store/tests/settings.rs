//! Integration tests for the settings repository.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use gmm_store::{Database, SETTING_SELECTED_INSTALLATION};
use tempfile::TempDir;

#[test]
fn missing_key_is_none() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.get_setting("nope").unwrap(), None);
}

#[test]
fn set_get_upsert_delete() {
    let db = Database::open_in_memory().unwrap();
    let key = SETTING_SELECTED_INSTALLATION;
    assert_eq!(key, "selected_installation");
    db.set_setting(key, "a").unwrap();
    assert_eq!(db.get_setting(key).unwrap().as_deref(), Some("a"));
    db.set_setting(key, "b").unwrap();
    assert_eq!(db.get_setting(key).unwrap().as_deref(), Some("b"));
    db.set_setting("other", "").unwrap();
    assert_eq!(db.get_setting("other").unwrap().as_deref(), Some(""));
    db.delete_setting(key).unwrap();
    assert_eq!(db.get_setting(key).unwrap(), None);
    assert_eq!(db.get_setting("other").unwrap().as_deref(), Some(""));
    db.delete_setting(key).unwrap();
}

#[test]
fn settings_persist_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gmm.sqlite3");
    Database::open(&path)
        .unwrap()
        .set_setting("k", "v")
        .unwrap();
    assert_eq!(
        Database::open(&path)
            .unwrap()
            .get_setting("k")
            .unwrap()
            .as_deref(),
        Some("v")
    );
}
