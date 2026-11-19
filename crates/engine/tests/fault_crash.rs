//! Tests 8 and 9: exhaustive crash recovery and unavailable roots.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{KINDS, Kind, Scenario, observe};
use gmm_domain::TransactionStatus;
use gmm_engine::{Error, recover};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, OpMatch, StdFs};

fn crash_plan(nth: usize) -> FaultPlan {
    FaultPlan {
        op: OpMatch::Any,
        nth,
        mode: FaultMode::Crash,
    }
}

#[test]
fn crash_at_every_operation_recovers_to_pre_or_post_state() {
    for kind in KINDS {
        let dry = Scenario::new(kind);
        let pre = observe(&dry.fx);
        let counter = FaultyFs::new(StdFs);
        dry.run(&counter).expect("dry run");
        let n = counter.ops_count();
        let post = observe(&dry.fx);
        assert!(n >= 8, "{kind:?}");

        let (mut to_pre, mut to_post) = (0usize, 0usize);
        for nth in 1..=n {
            let mut sc = Scenario::new(kind);
            let before = sc.fx.tx_ids();
            let ctx = format!("{kind:?} crash at op {nth}/{n}");
            let fs = FaultyFs::with_plan(StdFs, crash_plan(nth));
            let err = sc.run(&fs).expect_err("a crash must stop the transaction");
            assert!(matches!(err, Error::Crashed), "{ctx}: {err}");
            let tx = sc.fx.new_tx(&before).expect("transaction journaled");

            // Power loss: no rollback and no further journal writes after the crash.
            let status = sc.fx.tx_status(tx);
            assert!(
                matches!(
                    status,
                    TransactionStatus::Open | TransactionStatus::Committing
                ),
                "{ctx}: status {status}"
            );
            let committing = status == TransactionStatus::Committing;
            drop(fs);

            // New process: reopen the database, plain file system, run recovery.
            sc.fx.reopen_db();
            let root = sc.fx.install.root().to_path_buf();
            let report =
                recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone())).expect("recover");
            assert!(
                report.needs_attention.is_empty() && report.skipped.is_empty(),
                "{ctx}"
            );
            assert_eq!(
                report.committed.len() + report.rolled_back.len(),
                1,
                "{ctx}"
            );

            let final_status = sc.fx.tx_status(tx);
            assert!(final_status.is_terminal(), "{ctx}: {final_status}");
            let seen = observe(&sc.fx);
            if committing {
                assert_eq!(final_status, TransactionStatus::Committed, "{ctx}");
                assert_eq!(seen, post, "{ctx}: roll-forward must reach the post-state");
                to_post += 1;
            } else {
                assert_eq!(final_status, TransactionStatus::RolledBack, "{ctx}");
                assert_eq!(seen, pre, "{ctx}: rollback must restore the pre-state");
                to_pre += 1;
            }
            assert!(
                sc.fx.leftovers().is_empty(),
                "{ctx}: {:?}",
                sc.fx.leftovers()
            );
            sc.fx.assert_protected_intact();

            // Committed adoptions keep verified vault objects.
            if kind == Kind::Adopt && committing {
                let m = sc.fx.db.list_mods(sc.fx.inst).expect("mods");
                assert_eq!(m.len(), 1, "{ctx}");
                for e in sc.fx.db.list_mod_entries(m[0].id).expect("entries") {
                    assert!(
                        sc.fx.vault.verify(&StdFs, &e.sha256).expect("verify"),
                        "{ctx}"
                    );
                }
            }

            // Recovery is idempotent.
            let again = recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone()))
                .expect("recover again");
            assert_eq!(again, gmm_engine::RecoveryReport::default(), "{ctx}");
        }
        println!(
            "fault-injection (crash) {kind:?}: {n} cases = {to_pre} recovered to pre-state + {to_post} rolled forward"
        );
    }
}

#[test]
fn recovery_skips_installations_whose_root_is_unavailable() {
    let mut sc = Scenario::new(Kind::Enable);
    let pre = observe(&sc.fx);
    let before = sc.fx.tx_ids();
    let fs = FaultyFs::with_plan(StdFs, crash_plan(4));
    assert!(matches!(sc.run(&fs), Err(Error::Crashed)));
    let tx = sc.fx.new_tx(&before).expect("tx");
    let crashed = observe(&sc.fx);
    drop(fs);
    sc.fx.reopen_db();

    let report = recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| None).expect("recover");
    assert_eq!(report.skipped, vec![tx]);
    assert!(report.rolled_back.is_empty() && report.committed.is_empty());
    assert_eq!(
        sc.fx.tx_status(tx),
        TransactionStatus::Open,
        "left untouched"
    );
    assert_eq!(observe(&sc.fx), crashed, "no file touched");

    // New work on that installation stays blocked until the transaction is recovered.
    assert!(matches!(sc.run(&StdFs), Err(Error::InvalidState { .. })));

    let root = sc.fx.install.root().to_path_buf();
    let report =
        recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone())).expect("recover");
    assert_eq!(report.rolled_back, vec![tx]);
    assert_eq!(observe(&sc.fx), pre);
}
