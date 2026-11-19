//! T18: resolving drift (restore from the vault, re-adopt current content), including the
//! exhaustive error and crash loops.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{DRIFT_KINDS, Fixture, Kind, M1_FILES, Scenario, observe, rp};
use gmm_domain::{ModState, Sha256, TransactionKind, TransactionStatus};
use gmm_engine::{Error, RecoveryReport, execute, plan_readopt, plan_restore, recover};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, OpMatch, StdFs};
use gmm_testkit::fake_bytes;

fn vault_has(fx: &Fixture, bytes: &[u8]) -> bool {
    let tmp = fx.app.path().join("probe.bin");
    std::fs::write(&tmp, bytes).expect("probe");
    let sha = Sha256::parse(&gmm_platform::sha256_file(&StdFs, &tmp).expect("hash")).expect("sha");
    std::fs::remove_file(&tmp).expect("rm probe");
    fx.vault.verify(&StdFs, &sha).expect("verify")
}

/// No content lost: every file that existed before is still at its path or in the vault.
fn assert_nothing_lost(fx: &Fixture, pre: &[(String, String)], ctx: &str) {
    let now = fx.game_hashes();
    for (path, hash) in pre {
        let there = now.iter().any(|(p, h)| p == path && h == hash);
        let sha = Sha256::parse(hash).expect("sha");
        let in_vault = fx.vault.verify(&StdFs, &sha).expect("verify");
        assert!(there || in_vault, "{ctx}: content of {path} was lost");
    }
}

#[test]
fn restore_places_missing_and_preserves_modified_content() {
    let sc = Scenario::new(Kind::Restore);
    let fx = &sc.fx;
    let user_bytes = fx.install.read("Trainer.asi").expect("read");
    assert_eq!(user_bytes, fake_bytes("user/Trainer.asi", 555));
    sc.run(&StdFs).expect("restore");
    // Vault version is back, byte-identical to what was adopted.
    for f in M1_FILES {
        assert_eq!(fx.install.read(f).expect("read"), fake_bytes(f, 300), "{f}");
    }
    assert!(
        vault_has(fx, &user_bytes),
        "user's version must be in the vault"
    );
    let m = fx.db.list_mods(fx.inst).expect("mods").remove(0);
    assert_eq!(m.state, ModState::Enabled);
    assert!(fx.reconcile().is_empty());
    assert_eq!(fx.mod_state(m.id), ModState::Enabled);
    assert!(fx.leftovers().is_empty());
    let tx = fx.tx_ids().into_iter().next().expect("latest tx");
    let (row, _) = fx.db.load_transaction(tx).expect("db").expect("tx");
    assert_eq!(row.kind, TransactionKind::Restore);
    assert_eq!(row.status, TransactionStatus::Committed);
    fx.assert_protected_intact();
}

#[test]
fn restore_requires_a_drifted_mod_and_a_valid_vault() {
    let sc = Scenario::new(Kind::Restore);
    let fx = &sc.fx;
    let id = fx.db.list_mods(fx.inst).expect("mods")[0].id;
    // Corrupt one vault object: refused at plan time, nothing touched.
    let entries = fx.db.list_mod_entries(id).expect("entries");
    let obj = fx.vault.object_path(&entries[0].sha256);
    let mut perm = std::fs::metadata(&obj).expect("meta").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&obj, perm).expect("perm");
    std::fs::write(&obj, b"corrupt").expect("corrupt");
    let before = fx.snapshot();
    let ctx = fx.ctx(&StdFs);
    assert!(matches!(
        plan_restore(&ctx, id),
        Err(Error::VaultCorrupt { .. })
    ));
    assert_eq!(fx.snapshot(), before);

    let healthy = Fixture::new();
    let m = healthy.adopt("M1", M1_FILES);
    let ctx = healthy.ctx(&StdFs);
    assert!(matches!(
        plan_restore(&ctx, m),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        plan_readopt(&ctx, m),
        Err(Error::InvalidState { .. })
    ));
}

#[test]
fn restore_refuses_hard_linked_entries() {
    let sc = Scenario::new(Kind::Restore);
    let fx = &sc.fx;
    std::fs::hard_link(
        fx.install.path("Trainer.asi"),
        fx.install.path("Trainer.link"),
    )
    .expect("hard link");
    let id = fx.db.list_mods(fx.inst).expect("mods")[0].id;
    let ctx = fx.ctx(&StdFs);
    assert!(matches!(
        plan_restore(&ctx, id),
        Err(Error::OwnershipUncertain { .. })
    ));
    assert!(matches!(
        plan_readopt(&ctx, id),
        Err(Error::OwnershipUncertain { .. })
    ));
}

#[test]
fn readopt_accepts_current_content_without_touching_files() {
    let sc = Scenario::new(Kind::Readopt);
    let fx = &sc.fx;
    let id = fx.db.list_mods(fx.inst).expect("mods")[0].id;
    let before = fx.snapshot();
    let new_asi = fx.install.read("Trainer.asi").expect("read");
    sc.run(&StdFs).expect("readopt");
    assert_eq!(fx.snapshot(), before, "game files untouched");
    assert_eq!(fx.mod_state(id), ModState::Enabled);
    let entries = fx.db.list_mod_entries(id).expect("entries");
    assert_eq!(entries.len(), 3, "missing entry removed");
    assert!(entries.iter().all(|e| e.rel_path != rp("scripts/y.ini")));
    let asi = entries
        .iter()
        .find(|e| e.rel_path == rp("Trainer.asi"))
        .expect("entry");
    assert_eq!(asi.size, new_asi.len() as u64);
    assert!(vault_has(fx, &new_asi));
    assert!(fx.reconcile().is_empty());
    assert_eq!(fx.mod_state(id), ModState::Enabled);
    assert!(fx.leftovers().is_empty());
    // The mod can be disabled and re-enabled with the adopted content.
    fx.disable(id);
    assert!(!fx.install.exists("Trainer.asi"));
    fx.enable(id);
    assert_eq!(fx.install.read("Trainer.asi").expect("read"), new_asi);
}

#[test]
fn readopt_with_only_missing_files_removes_entries_and_all_missing_is_invalid() {
    let fx = Fixture::new();
    let id = fx.adopt("M1", M1_FILES);
    fx.install.remove("scripts/y.ini").expect("rm");
    fx.reconcile();
    let ctx = fx.ctx(&StdFs);
    let plan = plan_readopt(&ctx, id).expect("plan");
    assert!(plan.steps.is_empty());
    execute(&ctx, plan).expect("execute without file steps");
    assert_eq!(fx.db.list_mod_entries(id).expect("entries").len(), 3);
    assert_eq!(fx.mod_state(id), ModState::Enabled);

    for f in ["Trainer.asi", "Trainer.ini", "scripts/sub", "scripts/y.ini"] {
        fx.install.remove(f).ok();
    }
    fx.reconcile();
    assert_eq!(fx.mod_state(id), ModState::Drifted);
    match plan_readopt(&ctx, id) {
        Err(Error::InvalidInput { what }) => {
            assert_eq!(
                what,
                "All files of this mod are missing; restore it instead."
            );
        }
        other => panic!("expected InvalidInput, got {other:?}"),
    }
    // Restore does work for the all-missing case.
    execute(&ctx, plan_restore(&ctx, id).expect("plan restore")).expect("restore");
    assert!(fx.reconcile().is_empty());
}

#[test]
fn error_at_every_operation_rolls_back_without_losing_content() {
    for kind in DRIFT_KINDS {
        let dry = Scenario::new(kind);
        let pre = observe(&dry.fx);
        let counter = FaultyFs::new(StdFs);
        dry.run(&counter).expect("dry run");
        let n = counter.ops_count();
        let post = observe(&dry.fx);
        assert!(n >= 8, "{kind:?}: {n}");
        assert_ne!(pre, post, "{kind:?}");

        let (mut rolled_back, mut roll_forward) = (0usize, 0usize);
        for nth in 1..=n {
            let sc = Scenario::new(kind);
            let pre_files = sc.fx.game_hashes();
            let before = sc.fx.tx_ids();
            let fs = FaultyFs::with_plan(
                StdFs,
                FaultPlan {
                    op: OpMatch::Any,
                    nth,
                    mode: FaultMode::Error,
                },
            );
            let err = sc.run(&fs).expect_err("injected error must fail");
            let tx = sc.fx.new_tx(&before).expect("journaled");
            let ctx = format!("{kind:?} op {nth}/{n}");
            match err {
                Error::CommitIncomplete { tx: t } => {
                    assert_eq!(t, tx, "{ctx}");
                    let root = sc.fx.install.root().to_path_buf();
                    let report = recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone()))
                        .expect("recover");
                    assert_eq!(report.committed, vec![tx], "{ctx}");
                    assert_eq!(observe(&sc.fx), post, "{ctx}");
                    roll_forward += 1;
                }
                other => {
                    assert!(matches!(other, Error::Fs(_)), "{ctx}: {other}");
                    assert_eq!(sc.fx.tx_status(tx), TransactionStatus::RolledBack, "{ctx}");
                    assert_eq!(observe(&sc.fx), pre, "{ctx}: not restored");
                    rolled_back += 1;
                }
            }
            assert_nothing_lost(&sc.fx, &pre_files, &ctx);
            assert!(
                sc.fx.leftovers().is_empty(),
                "{ctx}: {:?}",
                sc.fx.leftovers()
            );
            sc.fx.assert_protected_intact();
        }
        assert!(rolled_back > 0, "{kind:?}");
        println!(
            "fault-injection (error) {kind:?}: {n} cases = {rolled_back} rolled back + {roll_forward} rolled forward"
        );
    }
}

#[test]
fn crash_at_every_operation_recovers_without_losing_content() {
    for kind in DRIFT_KINDS {
        let dry = Scenario::new(kind);
        let pre = observe(&dry.fx);
        let counter = FaultyFs::new(StdFs);
        dry.run(&counter).expect("dry run");
        let n = counter.ops_count();
        let post = observe(&dry.fx);

        let (mut to_pre, mut to_post) = (0usize, 0usize);
        for nth in 1..=n {
            let mut sc = Scenario::new(kind);
            let pre_files = sc.fx.game_hashes();
            let before = sc.fx.tx_ids();
            let ctx = format!("{kind:?} crash at op {nth}/{n}");
            let fs = FaultyFs::with_plan(
                StdFs,
                FaultPlan {
                    op: OpMatch::Any,
                    nth,
                    mode: FaultMode::Crash,
                },
            );
            let err = sc.run(&fs).expect_err("crash stops the transaction");
            assert!(matches!(err, Error::Crashed), "{ctx}: {err}");
            let tx = sc.fx.new_tx(&before).expect("journaled");
            let committing = sc.fx.tx_status(tx) == TransactionStatus::Committing;
            drop(fs);

            sc.fx.reopen_db();
            let root = sc.fx.install.root().to_path_buf();
            let report =
                recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone())).expect("recover");
            assert!(
                report.needs_attention.is_empty() && report.skipped.is_empty(),
                "{ctx}"
            );
            let seen = observe(&sc.fx);
            if committing {
                assert_eq!(sc.fx.tx_status(tx), TransactionStatus::Committed, "{ctx}");
                assert_eq!(seen, post, "{ctx}");
                to_post += 1;
            } else {
                assert_eq!(sc.fx.tx_status(tx), TransactionStatus::RolledBack, "{ctx}");
                assert_eq!(seen, pre, "{ctx}");
                to_pre += 1;
            }
            assert_nothing_lost(&sc.fx, &pre_files, &ctx);
            assert!(
                sc.fx.leftovers().is_empty(),
                "{ctx}: {:?}",
                sc.fx.leftovers()
            );
            sc.fx.assert_protected_intact();
            let again = recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone()))
                .expect("recover again");
            assert_eq!(again, RecoveryReport::default(), "{ctx}");
            if committing {
                assert!(sc.fx.reconcile().is_empty(), "{ctx}: reconcile not clean");
            }
        }
        println!(
            "fault-injection (crash) {kind:?}: {n} cases = {to_pre} recovered to pre-state + {to_post} rolled forward"
        );
    }
}
