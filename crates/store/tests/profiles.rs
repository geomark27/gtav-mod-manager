//! Integration tests for the profiles repository.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use common::*;
use gmm_domain::{DesiredState, InstallationId, ModId, ModState, Profile, ProfileEntry, ProfileId};
use gmm_store::Error;

fn profile(inst: InstallationId, name: &str) -> Profile {
    Profile {
        id: ProfileId::new(),
        installation_id: inst,
        name: name.into(),
        created_at: ts(1),
        updated_at: ts(1),
    }
}

fn entry(p: &Profile, m: ModId, d: DesiredState) -> ProfileEntry {
    ProfileEntry {
        profile_id: p.id,
        mod_id: m,
        desired: d,
    }
}

#[test]
fn create_list_get_round_trip() {
    let (db, inst) = setup();
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let b = adopt_mod(&db, inst, "B", ModState::Disabled, 2, 2);
    let p2 = profile(inst, "zeta");
    let p1 = profile(inst, "  Alpha ");
    db.create_profile(&p2, &[]).unwrap();
    let es = [
        entry(&p1, a.id, DesiredState::Enabled),
        entry(&p1, b.id, DesiredState::Disabled),
    ];
    db.create_profile(&p1, &es).unwrap();

    let list = db.list_profiles(inst).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].0.name.as_str(), list[0].1), ("Alpha", 2));
    assert_eq!((list[1].0.name.as_str(), list[1].1), ("zeta", 0));

    let (got, entries) = db.get_profile(p1.id).unwrap().unwrap();
    assert_eq!(got.name, "Alpha");
    assert_eq!(entries.len(), 2);
    for e in &es {
        assert!(entries.contains(e));
    }
    assert!(db.get_profile(ProfileId::new()).unwrap().is_none());
    assert!(
        db.list_profiles(add_installation(&db, "D:/X"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_names_rejected() {
    let (db, inst) = setup();
    for n in [
        "Vanilla",
        "vANILLA",
        "  vanilla  ",
        "",
        "   ",
        &"x".repeat(61),
    ] {
        let r = db.create_profile(&profile(inst, n), &[]);
        assert!(matches!(r, Err(Error::InvalidState { .. })), "{n:?}");
    }
    assert_eq!(count(&db, "profiles"), 0);
    db.create_profile(&profile(inst, &"x".repeat(60)), &[])
        .unwrap();
}

#[test]
fn duplicate_name_conflicts() {
    let (db, inst) = setup();
    db.create_profile(&profile(inst, "One"), &[]).unwrap();
    let r = db.create_profile(&profile(inst, "one"), &[]);
    assert!(matches!(r, Err(Error::Conflict { .. })));
    // Same name in another installation is fine.
    let other = add_installation(&db, "D:/Other");
    db.create_profile(&profile(other, "One"), &[]).unwrap();
}

#[test]
fn foreign_mod_rejected_and_nothing_written() {
    let (db, inst) = setup();
    let other = add_installation(&db, "D:/Other");
    let mine = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let foreign = adopt_mod(&db, other, "F", ModState::Enabled, 2, 2);
    let p = profile(inst, "P");
    let r = db.create_profile(
        &p,
        &[
            entry(&p, mine.id, DesiredState::Enabled),
            entry(&p, foreign.id, DesiredState::Enabled),
        ],
    );
    assert!(matches!(r, Err(Error::InvalidState { .. })));
    assert_eq!(count(&db, "profiles"), 0);
    assert_eq!(count(&db, "profile_entries"), 0);
    let r = db.create_profile(&p, &[entry(&p, ModId::new(), DesiredState::Enabled)]);
    assert!(matches!(r, Err(Error::InvalidState { .. })));
}

#[test]
fn replace_entries_is_atomic() {
    let (db, inst) = setup();
    let other = add_installation(&db, "D:/Other");
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let b = adopt_mod(&db, inst, "B", ModState::Enabled, 2, 2);
    let foreign = adopt_mod(&db, other, "F", ModState::Enabled, 3, 3);
    let p = profile(inst, "P");
    db.create_profile(&p, &[entry(&p, a.id, DesiredState::Enabled)])
        .unwrap();

    let bad = [
        entry(&p, b.id, DesiredState::Enabled),
        entry(&p, foreign.id, DesiredState::Enabled),
    ];
    let r = db.replace_profile_entries(p.id, &bad, ts(9));
    assert!(matches!(r, Err(Error::InvalidState { .. })));
    let (got, entries) = db.get_profile(p.id).unwrap().unwrap();
    assert_eq!(entries, [entry(&p, a.id, DesiredState::Enabled)]);
    assert_eq!(got.updated_at, ts(1));

    let good = [
        entry(&p, b.id, DesiredState::Disabled),
        entry(&p, a.id, DesiredState::Disabled),
    ];
    db.replace_profile_entries(p.id, &good, ts(10)).unwrap();
    let (got, entries) = db.get_profile(p.id).unwrap().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.desired == DesiredState::Disabled));
    assert_eq!(got.updated_at, ts(10));

    let r = db.replace_profile_entries(ProfileId::new(), &[], ts(11));
    assert!(matches!(r, Err(Error::NotFound { .. })));
}

#[test]
fn rename_rules() {
    let (db, inst) = setup();
    let p = profile(inst, "P");
    let q = profile(inst, "Q");
    db.create_profile(&p, &[]).unwrap();
    db.create_profile(&q, &[]).unwrap();
    db.rename_profile(p.id, " New ", ts(5)).unwrap();
    let (got, _) = db.get_profile(p.id).unwrap().unwrap();
    assert_eq!((got.name.as_str(), got.updated_at), ("New", ts(5)));
    // Renaming to its own name with different case is allowed.
    db.rename_profile(p.id, "NEW", ts(6)).unwrap();
    assert!(matches!(
        db.rename_profile(p.id, "q", ts(7)),
        Err(Error::Conflict { .. })
    ));
    assert!(matches!(
        db.rename_profile(p.id, "VANILLA", ts(7)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.rename_profile(ProfileId::new(), "Z", ts(7)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn delete_profile_cascades_entries() {
    let (db, inst) = setup();
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let p = profile(inst, "P");
    db.create_profile(&p, &[entry(&p, a.id, DesiredState::Enabled)])
        .unwrap();
    db.delete_profile(p.id).unwrap();
    assert!(db.get_profile(p.id).unwrap().is_none());
    assert_eq!(count(&db, "profile_entries"), 0);
    assert!(matches!(
        db.delete_profile(p.id),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn deleting_mod_removes_its_profile_entries() {
    let (db, inst) = setup();
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let b = adopt_mod(&db, inst, "B", ModState::Enabled, 2, 2);
    let p = profile(inst, "P");
    db.create_profile(
        &p,
        &[
            entry(&p, a.id, DesiredState::Enabled),
            entry(&p, b.id, DesiredState::Enabled),
        ],
    )
    .unwrap();
    db.delete_mod(a.id).unwrap();
    let (_, entries) = db.get_profile(p.id).unwrap().unwrap();
    assert_eq!(entries, [entry(&p, b.id, DesiredState::Enabled)]);
    assert_eq!(db.list_profiles(inst).unwrap()[0].1, 1);
}
