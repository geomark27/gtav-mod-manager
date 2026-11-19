//! `CommitEffects` entry updates and removals.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_domain::{ModState, TransactionKind, TransactionStatus};
use gmm_store::{CommitEffects, Error};

fn committing(
    db: &gmm_store::Database,
    inst: gmm_domain::InstallationId,
    n: i64,
) -> gmm_domain::TransactionId {
    let tx = new_tx(db, inst, TransactionKind::Adopt, n);
    db.set_transaction_status(tx, TransactionStatus::Committing, None, ts(n))
        .unwrap();
    tx
}

#[test]
fn updates_and_removals_apply_atomically() {
    let (db, inst) = setup();
    let m = adopt_mod_files(
        &db,
        inst,
        "M",
        ModState::Drifted,
        &[("a.dll", 1), ("b.dll", 2)],
        1,
    );
    let entries = db.list_mod_entries(m.id).unwrap();
    let a = entries.iter().find(|e| e.rel_path == rp("a.dll")).unwrap();
    let b = entries.iter().find(|e| e.rel_path == rp("b.dll")).unwrap();
    let tx = committing(&db, inst, 5);
    db.commit_transaction(
        tx,
        &CommitEffects {
            vault_objects: vec![(sha(9), 77)],
            entry_updates: vec![(a.id, sha(9), 77)],
            entry_removals: vec![b.id],
            mod_states: vec![(m.id, ModState::Enabled)],
            ..CommitEffects::default()
        },
        ts(6),
    )
    .unwrap();
    let after = db.list_mod_entries(m.id).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!((after[0].sha256.clone(), after[0].size), (sha(9), 77));
    assert_eq!(db.get_mod(m.id).unwrap().unwrap().state, ModState::Enabled);
}

#[test]
fn removing_every_entry_is_invalid_and_changes_nothing() {
    let (db, inst) = setup();
    let m = adopt_mod_files(
        &db,
        inst,
        "M",
        ModState::Drifted,
        &[("a.dll", 1), ("b.dll", 2)],
        1,
    );
    let ids: Vec<_> = db
        .list_mod_entries(m.id)
        .unwrap()
        .iter()
        .map(|e| e.id)
        .collect();
    let tx = committing(&db, inst, 5);
    let err = db
        .commit_transaction(
            tx,
            &CommitEffects {
                entry_removals: ids,
                mod_states: vec![(m.id, ModState::Enabled)],
                ..CommitEffects::default()
            },
            ts(6),
        )
        .unwrap_err();
    assert!(matches!(err, Error::InvalidState { .. }));
    assert_eq!(db.list_mod_entries(m.id).unwrap().len(), 2);
    assert_eq!(db.get_mod(m.id).unwrap().unwrap().state, ModState::Drifted);
    assert_eq!(
        db.load_transaction(tx).unwrap().unwrap().0.status,
        TransactionStatus::Committing
    );
}

#[test]
fn unknown_entry_is_not_found() {
    let (db, inst) = setup();
    let tx = committing(&db, inst, 5);
    let err = db
        .commit_transaction(
            tx,
            &CommitEffects {
                entry_removals: vec![gmm_domain::ModEntryId::new()],
                ..CommitEffects::default()
            },
            ts(6),
        )
        .unwrap_err();
    assert!(matches!(err, Error::NotFound { .. }));
}
