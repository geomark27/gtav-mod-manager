//! Mod dependencies and vault garbage-collection queries.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_domain::{ModState, TransactionKind, TransactionStatus};
use gmm_store::{CommitEffects, Error};

fn three(db: &gmm_store::Database, inst: gmm_domain::InstallationId) -> [gmm_domain::Mod; 3] {
    [
        adopt_mod(db, inst, "A", ModState::Enabled, 1, 1),
        adopt_mod(db, inst, "B", ModState::Enabled, 2, 2),
        adopt_mod(db, inst, "C", ModState::Disabled, 3, 3),
    ]
}

fn step7() -> gmm_store::NewStep {
    gmm_store::NewStep {
        seq: 0,
        kind: gmm_domain::StepKind::CaptureToVault,
        mod_id: None,
        rel_path: rp("z.dll"),
        sha256: Some(sha(7)),
    }
}

#[test]
fn add_list_and_remove() {
    let (db, inst) = setup();
    let [a, b, c] = three(&db, inst);
    db.add_dependency(a.id, b.id, ts(10)).unwrap();
    db.add_dependency(a.id, c.id, ts(10)).unwrap();
    db.add_dependency(b.id, c.id, ts(10)).unwrap();
    let mut deps = db.dependencies_of(a.id).unwrap();
    deps.sort();
    let mut want = vec![b.id, c.id];
    want.sort();
    assert_eq!(deps, want);
    let mut users = db.dependents_of(c.id).unwrap();
    users.sort();
    let mut want = vec![a.id, b.id];
    want.sort();
    assert_eq!(users, want);
    assert_eq!(db.dependency_edges(inst).unwrap().len(), 3);

    db.remove_dependency(a.id, c.id).unwrap();
    assert_eq!(db.dependencies_of(a.id).unwrap(), vec![b.id]);
    assert!(matches!(
        db.remove_dependency(a.id, c.id),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn duplicates_self_and_cycles_are_rejected() {
    let (db, inst) = setup();
    let [a, b, c] = three(&db, inst);
    db.add_dependency(a.id, b.id, ts(10)).unwrap();
    db.add_dependency(b.id, c.id, ts(10)).unwrap();
    assert!(matches!(
        db.add_dependency(a.id, b.id, ts(11)),
        Err(Error::Conflict { .. })
    ));
    assert!(matches!(
        db.add_dependency(a.id, a.id, ts(11)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.add_dependency(b.id, a.id, ts(11)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.add_dependency(c.id, a.id, ts(11)),
        Err(Error::InvalidState { .. })
    ));
    assert_eq!(db.dependency_edges(inst).unwrap().len(), 2);
}

#[test]
fn cross_installation_and_unknown_mods_are_rejected() {
    let (db, inst) = setup();
    let other = add_installation(&db, "C:/Games/Other");
    let a = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    let x = adopt_mod(&db, other, "X", ModState::Enabled, 2, 2);
    assert!(matches!(
        db.add_dependency(a.id, x.id, ts(10)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.add_dependency(a.id, gmm_domain::ModId::new(), ts(10)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn deleting_a_mod_cascades_its_edges() {
    let (db, inst) = setup();
    let [a, b, _] = three(&db, inst);
    db.add_dependency(a.id, b.id, ts(10)).unwrap();
    db.delete_mod(b.id).unwrap();
    assert!(db.dependency_edges(inst).unwrap().is_empty());
    assert!(db.dependencies_of(a.id).unwrap().is_empty());
}

#[test]
fn gc_query_excludes_entries_and_unresolved_transactions() {
    let (db, inst) = setup();
    let _m = adopt_mod(&db, inst, "A", ModState::Enabled, 1, 1);
    db.upsert_vault_object(&sha(7), 5, ts(2)).unwrap();
    db.upsert_vault_object(&sha(8), 6, ts(2)).unwrap();
    // Object 1 is referenced by an entry; 7 and 8 are orphans.
    let mut orphans = db.unreferenced_vault_objects().unwrap();
    orphans.sort();
    assert_eq!(orphans, vec![(sha(7), 5), (sha(8), 6)]);

    // An open transaction with a step on sha(1)... use a step on 7 via a custom transaction.
    let row = tx_row(inst, TransactionKind::Adopt, 20);
    db.create_transaction(&row, &[step7()], ts(20)).unwrap();
    assert_eq!(db.unreferenced_vault_objects().unwrap(), vec![(sha(8), 6)]);
    assert!(matches!(
        db.delete_vault_object_row(&sha(7)),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        db.delete_vault_object_row(&sha(1)),
        Err(Error::InvalidState { .. })
    ));

    // Same protection for every unresolved status (each reached by a legal transition path).
    db.set_transaction_status(row.id, TransactionStatus::Committing, None, ts(21))
        .unwrap();
    assert_eq!(db.unreferenced_vault_objects().unwrap(), vec![(sha(8), 6)]);
    db.commit_transaction(row.id, &CommitEffects::default(), ts(22))
        .unwrap();
    // Resolved: released; the row can then be deleted exactly once.
    assert_eq!(db.unreferenced_vault_objects().unwrap().len(), 2);
    db.delete_vault_object_row(&sha(7)).unwrap();
    assert!(db.get_vault_object(&sha(7)).unwrap().is_none());
    assert!(matches!(
        db.delete_vault_object_row(&sha(7)),
        Err(Error::NotFound { .. })
    ));

    // rolling_back and needs_attention also protect their objects.
    db.upsert_vault_object(&sha(7), 5, ts(26)).unwrap();
    let row2 = tx_row(inst, TransactionKind::Adopt, 27);
    db.create_transaction(&row2, &[step7()], ts(27)).unwrap();
    for status in [
        TransactionStatus::RollingBack,
        TransactionStatus::NeedsAttention,
    ] {
        db.set_transaction_status(row2.id, status, None, ts(28))
            .unwrap();
        assert_eq!(db.unreferenced_vault_objects().unwrap(), vec![(sha(8), 6)]);
    }
}

#[test]
fn gc_query_protects_objects_named_only_in_a_summary() {
    let (db, inst) = setup();
    db.upsert_vault_object(&sha(7), 5, ts(2)).unwrap();
    let mut row = tx_row(inst, TransactionKind::Adopt, 30);
    row.summary_json = format!(
        "{{\"adopt\":{{\"entries\":[[\"a.dll\",\"{}\",5]]}}}}",
        sha(7)
    );
    db.create_transaction(&row, &[], ts(30)).unwrap();
    assert!(db.unreferenced_vault_objects().unwrap().is_empty());
    db.set_transaction_status(row.id, TransactionStatus::Committing, None, ts(31))
        .unwrap();
    db.commit_transaction(row.id, &CommitEffects::default(), ts(32))
        .unwrap();
    assert_eq!(db.unreferenced_vault_objects().unwrap().len(), 1);
}
