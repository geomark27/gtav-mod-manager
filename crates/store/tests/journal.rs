//! Integration tests for the transactions journal and the atomic commit.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use common::*;
use gmm_domain::{
    ModState, StepKind, StepStatus, TransactionId, TransactionKind, TransactionStatus,
};
use gmm_store::{CommitEffects, Error, NewStep};

fn steps() -> Vec<NewStep> {
    vec![
        NewStep {
            seq: 0,
            kind: StepKind::CaptureToVault,
            mod_id: None,
            rel_path: rp("a/b.dll"),
            sha256: Some(sha(1)),
        },
        NewStep {
            seq: 1,
            kind: StepKind::EnsureDir,
            mod_id: None,
            rel_path: rp("a"),
            sha256: None,
        },
    ]
}

#[test]
fn create_and_load() {
    let (db, inst) = setup();
    let tx = new_tx(&db, inst, TransactionKind::Adopt, 1);
    let (row, st) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::Open);
    assert_eq!(row.kind, TransactionKind::Adopt);
    assert_eq!(row.finished_at, None);
    assert_eq!(st.len(), 2);
    assert_eq!((st[0].seq, st[1].seq), (0, 1));
    assert!(st.iter().all(|s| s.status == StepStatus::Pending));
    assert_eq!(st[0].sha256, Some(sha(1)));
    assert!(!st[0].created_by_step);
    assert!(db.load_transaction(TransactionId::new()).unwrap().is_none());
}

#[test]
fn create_is_atomic_on_duplicate_seq() {
    let (db, inst) = setup();
    let mut s = steps();
    s[1].seq = 0;
    let row = tx_row(inst, TransactionKind::Enable, 1);
    let err = db.create_transaction(&row, &s, ts(1)).unwrap_err();
    assert!(matches!(err, Error::Conflict { .. }));
    assert!(db.load_transaction(row.id).unwrap().is_none());
    // Duplicate id.
    let tx = new_tx(&db, inst, TransactionKind::Enable, 2);
    let mut dup = tx_row(inst, TransactionKind::Enable, 3);
    dup.id = tx;
    assert!(matches!(
        db.create_transaction(&dup, &[], ts(3)),
        Err(Error::Conflict { .. })
    ));
}

#[test]
fn step_transitions() {
    let (db, inst) = setup();
    let tx = new_tx(&db, inst, TransactionKind::Adopt, 1);
    // pending -> done is illegal and changes nothing.
    let err = db
        .mark_step(tx, 0, StepStatus::Done, None, None, None, ts(2))
        .unwrap_err();
    assert!(matches!(err, Error::InvalidState { .. }));
    db.mark_step(
        tx,
        0,
        StepStatus::Started,
        Some("x.gmm-tmp-ab"),
        None,
        None,
        ts(2),
    )
    .unwrap();
    db.mark_step(tx, 0, StepStatus::Done, None, Some(true), None, ts(3))
        .unwrap();
    let (_, st) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(st[0].status, StepStatus::Done);
    assert_eq!(st[0].artifact_name.as_deref(), Some("x.gmm-tmp-ab")); // kept
    assert!(st[0].created_by_step);
    assert_eq!(st[0].updated_at, ts(3));
    db.mark_step(tx, 0, StepStatus::Undone, None, None, Some("boom"), ts(4))
        .unwrap();
    let (_, st) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(st[0].error.as_deref(), Some("boom"));
    assert!(st[0].created_by_step); // unchanged
    assert!(matches!(
        db.mark_step(tx, 0, StepStatus::Started, None, None, None, ts(5)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.mark_step(tx, 9, StepStatus::Started, None, None, None, ts(5)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn transaction_transitions_and_finished_at() {
    let (db, inst) = setup();
    let tx = new_tx(&db, inst, TransactionKind::Enable, 1);
    assert!(matches!(
        db.set_transaction_status(tx, TransactionStatus::Committed, None, ts(2)),
        Err(Error::InvalidState { .. })
    ));
    db.set_transaction_status(tx, TransactionStatus::RollingBack, None, ts(2))
        .unwrap();
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.finished_at, None);
    db.set_transaction_status(tx, TransactionStatus::RolledBack, Some("why"), ts(3))
        .unwrap();
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::RolledBack);
    assert_eq!(row.finished_at, Some(ts(3)));
    assert_eq!(row.error.as_deref(), Some("why"));
    assert!(matches!(
        db.set_transaction_status(tx, TransactionStatus::Open, None, ts(4)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.set_transaction_status(
            TransactionId::new(),
            TransactionStatus::Committing,
            None,
            ts(4)
        ),
        Err(Error::NotFound { .. })
    ));
    // needs_attention sets finished_at too.
    let t2 = new_tx(&db, inst, TransactionKind::Enable, 5);
    db.set_transaction_status(t2, TransactionStatus::Committing, None, ts(6))
        .unwrap();
    db.set_transaction_status(t2, TransactionStatus::NeedsAttention, Some("e"), ts(7))
        .unwrap();
    let (row, _) = db.load_transaction(t2).unwrap().unwrap();
    assert_eq!(row.finished_at, Some(ts(7)));
}

fn committing(db: &Env, inst: gmm_domain::InstallationId, n: i64) -> TransactionId {
    let tx = new_tx(db, inst, TransactionKind::Adopt, n);
    db.set_transaction_status(tx, TransactionStatus::Committing, None, ts(n + 1))
        .unwrap();
    tx
}

#[test]
fn commit_adopt_inserts_everything() {
    let (db, inst) = setup();
    let tx = committing(&db, inst, 1);
    let m = new_mod(inst, "Cool", ModState::Enabled);
    let fx = CommitEffects {
        adopt: Some(adopt(&m, &[("a/b.dll", 1), ("c.asi", 2)])),
        vault_objects: vec![(sha(1), 10), (sha(2), 20)],
        mod_states: vec![],
        ..CommitEffects::default()
    };
    db.commit_transaction(tx, &fx, ts(10)).unwrap();
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::Committed);
    assert_eq!(row.finished_at, Some(ts(10)));
    assert_eq!(db.get_mod(m.id).unwrap().unwrap(), m);
    assert_eq!(db.list_mod_entries(m.id).unwrap().len(), 2);
    assert_eq!(db.get_vault_object(&sha(1)).unwrap().unwrap().0, 10);
    assert_eq!(count(&db, "ownership_confirmations"), 1);
    assert!(db.vault_object_referenced(&sha(2)).unwrap());
    // Cannot commit twice.
    assert!(matches!(
        db.commit_transaction(tx, &CommitEffects::default(), ts(11)),
        Err(Error::InvalidState { .. })
    ));
}

#[test]
fn commit_when_not_committing_changes_nothing() {
    let (db, inst) = setup();
    let tx = new_tx(&db, inst, TransactionKind::Adopt, 1); // open
    let m = new_mod(inst, "Cool", ModState::Enabled);
    let fx = CommitEffects {
        adopt: Some(adopt(&m, &[("a.dll", 1)])),
        vault_objects: vec![(sha(1), 10)],
        mod_states: vec![],
        ..CommitEffects::default()
    };
    assert!(matches!(
        db.commit_transaction(tx, &fx, ts(5)),
        Err(Error::InvalidState { .. })
    ));
    assert_eq!(count(&db, "mods"), 0);
    assert_eq!(count(&db, "vault_objects"), 0);
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::Open);
    assert!(matches!(
        db.commit_transaction(TransactionId::new(), &fx, ts(5)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn commit_failure_leaves_no_partial_rows() {
    let (db, inst) = setup();
    // Existing mod with the same name makes the adopt insert fail after vault upserts.
    let t0 = committing(&db, inst, 1);
    let existing = new_mod(inst, "Dup", ModState::Disabled);
    db.commit_transaction(
        t0,
        &CommitEffects {
            adopt: Some(adopt(&existing, &[("e.dll", 9)])),
            vault_objects: vec![(sha(9), 5)],
            mod_states: vec![],
            ..CommitEffects::default()
        },
        ts(3),
    )
    .unwrap();

    let tx = committing(&db, inst, 10);
    let m = new_mod(inst, "dup", ModState::Enabled); // different case: allowed
    let m2 = new_mod(inst, "Dup", ModState::Enabled); // exact duplicate
    let fx = CommitEffects {
        adopt: Some(adopt(&m2, &[("x.dll", 1)])),
        vault_objects: vec![(sha(1), 10)],
        mod_states: vec![(existing.id, ModState::Enabled)],
        ..CommitEffects::default()
    };
    assert!(matches!(
        db.commit_transaction(tx, &fx, ts(20)),
        Err(Error::Conflict { .. })
    ));
    assert_eq!(count(&db, "mods"), 1);
    assert_eq!(count(&db, "mod_entries"), 1);
    assert_eq!(count(&db, "ownership_confirmations"), 1);
    assert!(db.get_vault_object(&sha(1)).unwrap().is_none());
    assert_eq!(
        db.get_mod(existing.id).unwrap().unwrap().state,
        ModState::Disabled
    );
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::Committing);
    assert_eq!(row.finished_at, None);
    let _ = m;
}

#[test]
fn commit_rejects_foreign_installation_and_missing_mod_state() {
    let (db, inst) = setup();
    let other = add_installation(&db, "D:/Other");
    let tx = committing(&db, inst, 1);
    let m = new_mod(other, "Foreign", ModState::Enabled);
    let fx = CommitEffects {
        adopt: Some(adopt(&m, &[("a.dll", 1)])),
        vault_objects: vec![(sha(1), 1)],
        mod_states: vec![],
        ..CommitEffects::default()
    };
    assert!(matches!(
        db.commit_transaction(tx, &fx, ts(5)),
        Err(Error::InvalidState { .. })
    ));
    assert_eq!(count(&db, "mods"), 0);
    let fx = CommitEffects {
        mod_states: vec![(gmm_domain::ModId::new(), ModState::Enabled)],
        ..CommitEffects::default()
    };
    assert!(matches!(
        db.commit_transaction(tx, &fx, ts(5)),
        Err(Error::NotFound { .. })
    ));
    let (row, _) = db.load_transaction(tx).unwrap().unwrap();
    assert_eq!(row.status, TransactionStatus::Committing);
}

#[test]
fn mod_states_applied_atomically() {
    let (db, inst) = setup();
    let m1 = adopt_mod(&db, inst, "One", ModState::Disabled, 1, 1);
    let m2 = adopt_mod(&db, inst, "Two", ModState::Disabled, 2, 2);
    // Second state targets a missing mod: first must not stick.
    let tx = new_tx(&db, inst, TransactionKind::Group, 20);
    db.set_transaction_status(tx, TransactionStatus::Committing, None, ts(21))
        .unwrap();
    let bad = CommitEffects {
        mod_states: vec![
            (m1.id, ModState::Enabled),
            (gmm_domain::ModId::new(), ModState::Enabled),
        ],
        ..CommitEffects::default()
    };
    assert!(db.commit_transaction(tx, &bad, ts(22)).is_err());
    assert_eq!(
        db.get_mod(m1.id).unwrap().unwrap().state,
        ModState::Disabled
    );
    let good = CommitEffects {
        mod_states: vec![(m1.id, ModState::Enabled), (m2.id, ModState::Drifted)],
        ..CommitEffects::default()
    };
    db.commit_transaction(tx, &good, ts(23)).unwrap();
    assert_eq!(db.get_mod(m1.id).unwrap().unwrap().state, ModState::Enabled);
    assert_eq!(db.get_mod(m2.id).unwrap().unwrap().state, ModState::Drifted);
    assert_eq!(db.get_mod(m2.id).unwrap().unwrap().updated_at, ts(23));
}

#[test]
fn recovery_listing_history_and_attention() {
    let (db, inst) = setup();
    let t1 = new_tx(&db, inst, TransactionKind::Enable, 1); // open
    let t2 = new_tx(&db, inst, TransactionKind::Enable, 2);
    db.set_transaction_status(t2, TransactionStatus::Committing, None, ts(3))
        .unwrap();
    let t3 = new_tx(&db, inst, TransactionKind::Disable, 4);
    db.set_transaction_status(t3, TransactionStatus::RollingBack, None, ts(5))
        .unwrap();
    let t4 = new_tx(&db, inst, TransactionKind::Disable, 6);
    db.set_transaction_status(t4, TransactionStatus::RollingBack, None, ts(7))
        .unwrap();
    db.set_transaction_status(t4, TransactionStatus::RolledBack, None, ts(8))
        .unwrap();
    let t5 = new_tx(&db, inst, TransactionKind::Disable, 9);
    assert!(!db.installation_has_attention(inst).unwrap());
    db.set_transaction_status(t5, TransactionStatus::Committing, None, ts(10))
        .unwrap();
    db.set_transaction_status(t5, TransactionStatus::NeedsAttention, Some("x"), ts(11))
        .unwrap();

    let ids: Vec<_> = db
        .transactions_needing_recovery()
        .unwrap()
        .iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(ids, vec![t1, t2, t3]);
    assert!(db.installation_has_attention(inst).unwrap());
    let other = add_installation(&db, "D:/Other");
    assert!(!db.installation_has_attention(other).unwrap());

    let (page, total) = db.list_transactions(inst, 2, 0).unwrap();
    assert_eq!(total, 5);
    assert_eq!(page.iter().map(|r| r.id).collect::<Vec<_>>(), vec![t5, t4]);
    let (page, _) = db.list_transactions(inst, 2, 2).unwrap();
    assert_eq!(page.iter().map(|r| r.id).collect::<Vec<_>>(), vec![t3, t2]);
    let (page, _) = db.list_transactions(inst, 2, 4).unwrap();
    assert_eq!(page.iter().map(|r| r.id).collect::<Vec<_>>(), vec![t1]);
    let (page, total) = db.list_transactions(other, 10, 0).unwrap();
    assert!(page.is_empty() && total == 0);
}
