//! Integration tests for mods, entries, ownership queries and the vault registry.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use common::*;
use gmm_domain::{Category, ModState};
use gmm_store::Error;
use rusqlite::params;

#[test]
fn vault_upsert_keeps_created_at() {
    let (db, _) = setup();
    assert!(db.get_vault_object(&sha(1)).unwrap().is_none());
    db.upsert_vault_object(&sha(1), 10, ts(1)).unwrap();
    db.upsert_vault_object(&sha(1), 10, ts(5)).unwrap();
    let (size, created, verified) = db.get_vault_object(&sha(1)).unwrap().unwrap();
    assert_eq!((size, created, verified), (10, ts(1), ts(5)));
    assert!(!db.vault_object_referenced(&sha(1)).unwrap());
}

#[test]
fn list_and_get_mods() {
    let (db, inst) = setup();
    let b = adopt_mod(&db, inst, "banana", ModState::Disabled, 1, 1);
    let a = adopt_mod(&db, inst, "Apple", ModState::Enabled, 2, 2);
    let c = adopt_mod(&db, inst, "cherry", ModState::Enabled, 3, 3);
    let names: Vec<_> = db
        .list_mods(inst)
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert_eq!(names, ["Apple", "banana", "cherry"]);
    assert_eq!(db.get_mod(a.id).unwrap().unwrap(), a);
    assert!(db.get_mod(gmm_domain::ModId::new()).unwrap().is_none());
    assert!(
        db.list_mods(add_installation(&db, "D:/X"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(db.list_mod_entries(b.id).unwrap().len(), 1);
    assert_eq!(
        db.list_mod_entries(c.id).unwrap()[0].rel_path.as_str(),
        "f3.dll"
    );
}

#[test]
fn entries_for_installation_and_owners() {
    let (db, inst) = setup();
    let a = adopt_mod_files(
        &db,
        inst,
        "A",
        ModState::Enabled,
        &[("Mods/Shared.DLL", 1), ("a.asi", 2)],
        1,
    );
    let b = adopt_mod_files(
        &db,
        inst,
        "B",
        ModState::Disabled,
        &[("mods/shared.dll", 1)],
        2,
    );
    let c = adopt_mod_files(&db, inst, "C", ModState::Drifted, &[("c.asi", 3)], 3);
    let all = db.list_entries_for_installation(inst).unwrap();
    assert_eq!(all.len(), 4);
    assert!(
        all.windows(2)
            .all(|w| { (w[0].2.rel_path.key(), w[0].0) <= (w[1].2.rel_path.key(), w[1].0) })
    );
    assert!(
        all.iter()
            .any(|(m, s, _)| *m == c.id && *s == ModState::Drifted)
    );
    assert!(
        all.iter()
            .any(|(m, s, _)| *m == b.id && *s == ModState::Disabled)
    );

    let owners = db
        .enabled_owners(
            inst,
            &[
                "mods/shared.dll".to_owned(),
                "c.asi".to_owned(),
                "none".to_owned(),
            ],
        )
        .unwrap();
    assert_eq!(owners, vec![("mods/shared.dll".to_owned(), a.id)]);
    assert!(db.enabled_owners(inst, &[]).unwrap().is_empty());
    // Other installation sees nothing.
    let other = add_installation(&db, "D:/Other");
    assert!(
        db.enabled_owners(other, &["mods/shared.dll".to_owned()])
            .unwrap()
            .is_empty()
    );
    // Many keys (chunking).
    let keys: Vec<String> = (0..1200).map(|i| format!("k{i}")).collect();
    assert!(db.enabled_owners(inst, &keys).unwrap().is_empty());
    let mut keys = keys;
    keys.push("a.asi".into());
    assert_eq!(db.enabled_owners(inst, &keys).unwrap().len(), 1);
}

#[test]
fn vault_referenced_follows_entries() {
    let (db, inst) = setup();
    let m = adopt_mod(&db, inst, "A", ModState::Enabled, 7, 1);
    assert!(db.vault_object_referenced(&sha(7)).unwrap());
    db.delete_mod(m.id).unwrap();
    assert!(!db.vault_object_referenced(&sha(7)).unwrap());
    // The vault object row itself stays.
    assert!(db.get_vault_object(&sha(7)).unwrap().is_some());
}

#[test]
fn update_state_and_meta() {
    let (db, inst) = setup();
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let b = adopt_mod(&db, inst, "B", ModState::Enabled, 2, 2);
    db.update_mod_state(a.id, ModState::Error, ts(9)).unwrap();
    let got = db.get_mod(a.id).unwrap().unwrap();
    assert_eq!((got.state, got.updated_at), (ModState::Error, ts(9)));
    db.update_mod_meta(a.id, "A2", Category::ScriptsGameplay, Some("note"), ts(10))
        .unwrap();
    let got = db.get_mod(a.id).unwrap().unwrap();
    assert_eq!(got.name, "A2");
    assert_eq!(got.category, Category::ScriptsGameplay);
    assert_eq!(got.notes.as_deref(), Some("note"));
    assert!(matches!(
        db.update_mod_meta(a.id, "B", Category::Vehicles, None, ts(11)),
        Err(Error::Conflict { .. })
    ));
    assert_eq!(db.get_mod(a.id).unwrap().unwrap().name, "A2");
    assert!(matches!(
        db.update_mod_state(gmm_domain::ModId::new(), ModState::Enabled, ts(1)),
        Err(Error::NotFound { .. })
    ));
    assert!(matches!(
        db.update_mod_meta(
            gmm_domain::ModId::new(),
            "Z",
            Category::Vehicles,
            None,
            ts(1)
        ),
        Err(Error::NotFound { .. })
    ));
    let _ = b;
}

#[test]
fn delete_mod_cascades() {
    let (db, inst) = setup();
    let m = adopt_mod_files(
        &db,
        inst,
        "A",
        ModState::Enabled,
        &[("a.dll", 1), ("b.dll", 2)],
        1,
    );
    let keep = adopt_mod(&db, inst, "Keep", ModState::Enabled, 3, 2);
    {
        let conn = rusqlite::Connection::open(db_path(&db)).unwrap();
        conn.execute(
            "INSERT INTO profiles (id, installation_id, name, created_at, updated_at) \
             VALUES ('p1', ?1, 'P', 't', 't')",
            params![inst.to_string()],
        )
        .unwrap();
        for id in [m.id, keep.id] {
            conn.execute(
                "INSERT INTO profile_entries (profile_id, mod_id, desired) VALUES ('p1', ?1, 'enabled')",
                params![id.to_string()],
            )
            .unwrap();
        }
    }
    assert_eq!(count(&db, "mod_entries"), 3);
    assert_eq!(count(&db, "ownership_confirmations"), 2);
    assert_eq!(count(&db, "profile_entries"), 2);
    db.delete_mod(m.id).unwrap();
    assert_eq!(count(&db, "mods"), 1);
    assert_eq!(count(&db, "mod_entries"), 1);
    assert_eq!(count(&db, "ownership_confirmations"), 1);
    assert_eq!(count(&db, "profile_entries"), 1);
    assert!(matches!(db.delete_mod(m.id), Err(Error::NotFound { .. })));
}
