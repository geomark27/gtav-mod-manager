//! Vanilla mode: restore point, disable everything, restore, guards and provenance of adopted
//! mods.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{ErrorCode, NullSink, VanillaStatusDto};
use gmm_domain::{ModSourceKind, ModState, ProfileKind, TransactionKind};

fn status(e: &Env) -> VanillaStatusDto {
    e.app.vanilla_status(&e.id).expect("status")
}

#[test]
fn adopted_mods_are_manual() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    assert_eq!(m.source.kind, ModSourceKind::Manual);
    assert_eq!(m.source.detail, None);
    let json = serde_json::to_value(&m).expect("json");
    assert_eq!(json["source"]["kind"], "manual");
}

#[test]
fn vanilla_then_restore_brings_back_exactly_the_previous_states() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let c = adopt(&e, &BETA, "Beta");
    set(&e, &c.id, false);
    let with_mods = snapshot(&e);

    let s = status(&e);
    assert!(!s.active);
    assert!(s.restore_point.is_none());
    assert_eq!((s.mods_total, s.mods_in_game), (3, 2));
    assert!(s.unmanaged_files.is_some(), "the installation was scanned");

    let before = tx_count(&e);
    let s = e.app.enter_vanilla(&e.id, &NullSink).expect("vanilla");
    assert!(s.active);
    assert_eq!(s.mods_in_game, 0);
    let rp = s.restore_point.expect("restore point");
    assert_eq!(rp.kind, ProfileKind::RestorePoint);
    assert!(rp.name.starts_with("Before vanilla — "));
    assert_eq!(rp.entry_count, 3);
    assert_eq!(tx_count(&e), before + 1, "one transaction");
    for id in [&a.id, &b.id, &c.id] {
        assert_eq!(state_of(&e, id), ModState::Disabled);
    }
    for f in TRAINER.iter().chain(&ALPHA).chain(&BETA) {
        assert!(!e.install.exists(f), "{f} removed from the game folder");
    }
    let tx = e.app.list_transactions(&e.id, 1, 0).expect("history").items;
    assert_eq!(tx[0].kind, TransactionKind::Profile);

    // The restore point is listed with the profiles.
    let profiles = e.app.list_profiles(&e.id).expect("profiles");
    assert!(
        profiles
            .iter()
            .any(|p| p.id == rp.id && p.kind == ProfileKind::RestorePoint)
    );

    // Pressing vanilla again changes nothing and keeps the restore point.
    let again = e.app.enter_vanilla(&e.id, &NullSink).expect("again");
    assert_eq!(again.restore_point.as_ref().map(|p| &p.id), Some(&rp.id));
    assert_eq!(tx_count(&e), before + 1);

    let s = e
        .app
        .restore_from_vanilla(&e.id, &NullSink)
        .expect("restore");
    assert!(!s.active);
    assert!(s.restore_point.is_none(), "deleted once restored");
    assert_eq!(s.mods_in_game, 2);
    assert_eq!(state_of(&e, &a.id), ModState::Enabled);
    assert_eq!(state_of(&e, &b.id), ModState::Enabled);
    assert_eq!(state_of(&e, &c.id), ModState::Disabled);
    assert_eq!(
        snapshot(&e).diff(&with_mods),
        vec![],
        "game folder byte-identical to before vanilla"
    );

    // Nothing left to restore.
    assert_eq!(
        err(e.app.restore_from_vanilla(&e.id, &NullSink)).code,
        ErrorCode::NotFound
    );
}

#[test]
fn a_new_vanilla_replaces_the_restore_point_when_mods_were_enabled_again() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let first = e
        .app
        .enter_vanilla(&e.id, &NullSink)
        .expect("vanilla")
        .restore_point
        .expect("rp");
    // The user turns one mod back on by hand, then goes vanilla again.
    set(&e, &a.id, true);
    let s = status(&e);
    assert!(!s.active, "a mod is in the game again");
    let second = e
        .app
        .enter_vanilla(&e.id, &NullSink)
        .expect("vanilla 2")
        .restore_point
        .expect("rp 2");
    assert_ne!(first.id, second.id);
    assert_eq!(
        e.app
            .list_profiles(&e.id)
            .expect("profiles")
            .iter()
            .filter(|p| p.kind == ProfileKind::RestorePoint)
            .count(),
        1
    );
    e.app
        .restore_from_vanilla(&e.id, &NullSink)
        .expect("restore");
    assert_eq!(state_of(&e, &a.id), ModState::Enabled);
    assert_eq!(
        state_of(&e, &b.id),
        ModState::Disabled,
        "latest snapshot wins"
    );
}

#[test]
fn mods_adopted_after_vanilla_are_left_alone_by_restore() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    e.app.enter_vanilla(&e.id, &NullSink).expect("vanilla");
    let b = adopt(&e, &ALPHA, "Alpha");
    e.app
        .restore_from_vanilla(&e.id, &NullSink)
        .expect("restore");
    assert_eq!(state_of(&e, &a.id), ModState::Enabled);
    assert_eq!(state_of(&e, &b.id), ModState::Enabled);
}

#[test]
fn vanilla_is_refused_while_the_game_runs_and_with_drift() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    assert_eq!(
        err(e.app.enter_vanilla(&e.id, &NullSink)).code,
        ErrorCode::GameRunning
    );
    e.os.0.lock().expect("lock").clear();
    assert!(status(&e).restore_point.is_none());

    e.install.write("trainer.asi", b"edited").expect("write");
    assert_eq!(
        err(e.app.enter_vanilla(&e.id, &NullSink)).code,
        ErrorCode::DriftDetected
    );
    assert!(
        status(&e).restore_point.is_none(),
        "drift blocks before the restore point is saved"
    );
    assert_eq!(state_of(&e, &a.id), ModState::Drifted);
    assert!(e.install.exists("trainer.asi"));
}

#[test]
fn restore_is_refused_while_the_game_runs_and_keeps_the_restore_point() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    e.app.enter_vanilla(&e.id, &NullSink).expect("vanilla");
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    assert_eq!(
        err(e.app.restore_from_vanilla(&e.id, &NullSink)).code,
        ErrorCode::GameRunning
    );
    e.os.0.lock().expect("lock").clear();
    assert!(status(&e).active);
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);
}

#[test]
fn vanilla_with_no_mods_does_nothing() {
    let e = env();
    let before = tx_count(&e);
    let s = e.app.enter_vanilla(&e.id, &NullSink).expect("vanilla");
    assert!(!s.active);
    assert!(s.restore_point.is_none());
    assert_eq!(s.mods_total, 0);
    assert_eq!(tx_count(&e), before);
}

#[test]
fn unknown_installation_is_not_found() {
    let e = env();
    let missing = "00000000-0000-7000-8000-000000000000";
    assert_eq!(err(e.app.vanilla_status(missing)).code, ErrorCode::NotFound);
}
