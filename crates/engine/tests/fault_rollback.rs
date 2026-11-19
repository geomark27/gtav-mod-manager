//! Test 7: rollback on error, with an I/O error injected at every mutating operation.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{KINDS, Scenario, observe};
use gmm_domain::TransactionStatus;
use gmm_engine::{Error, recover};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, OpMatch, StdFs};

#[test]
fn error_at_every_operation_rolls_back_to_the_exact_pre_state() {
    for kind in KINDS {
        // Fault-free dry run: measures N and the expected post-state.
        let dry = Scenario::new(kind);
        let pre = observe(&dry.fx);
        let counter = FaultyFs::new(StdFs);
        dry.run(&counter).expect("dry run");
        let n = counter.ops_count();
        let post = observe(&dry.fx);
        assert!(n >= 8, "{kind:?}: suspiciously few operations ({n})");
        assert_ne!(pre, post, "{kind:?}: the transaction must change something");

        let (mut rolled_back, mut roll_forward) = (0usize, 0usize);
        for nth in 1..=n {
            let sc = Scenario::new(kind);
            let before = sc.fx.tx_ids();
            let fs = FaultyFs::with_plan(
                StdFs,
                FaultPlan {
                    op: OpMatch::Any,
                    nth,
                    mode: FaultMode::Error,
                },
            );
            let err = sc
                .run(&fs)
                .expect_err("an injected error must fail the transaction");
            let tx = sc.fx.new_tx(&before).expect("a transaction was journaled");
            let ctx = format!("{kind:?} op {nth}/{n}");

            match err {
                // Errors after `committing` cannot be rolled back: recovery rolls forward.
                Error::CommitIncomplete { tx: t } => {
                    assert_eq!(t, tx, "{ctx}");
                    assert_eq!(sc.fx.tx_status(tx), TransactionStatus::Committing, "{ctx}");
                    let root = sc.fx.install.root().to_path_buf();
                    let report = recover(&StdFs, &sc.fx.db, &sc.fx.vault, &|_| Some(root.clone()))
                        .expect("recover");
                    assert_eq!(report.committed, vec![tx], "{ctx}");
                    assert_eq!(sc.fx.tx_status(tx), TransactionStatus::Committed, "{ctx}");
                    assert_eq!(observe(&sc.fx), post, "{ctx}");
                    roll_forward += 1;
                }
                other => {
                    assert!(
                        matches!(other, Error::Fs(_)),
                        "{ctx}: unexpected error {other}"
                    );
                    assert_eq!(sc.fx.tx_status(tx), TransactionStatus::RolledBack, "{ctx}");
                    assert_eq!(observe(&sc.fx), pre, "{ctx}: not restored");
                    rolled_back += 1;
                }
            }
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
