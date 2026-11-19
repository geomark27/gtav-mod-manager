//! Adopt / enable / disable / conflicts / drift / integrity (tests 1 to 6 and 10).
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{Fixture, M1_FILES, rp};
use gmm_domain::{Category, ModId, ModState, RelPath, StepKind, TransactionStatus};
use gmm_engine::{
    DriftKind, Error, execute, plan_adopt, plan_disable, plan_enable, plan_set_states, reconcile,
};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, Fs, FsOp, OpMatch, StdFs};
use gmm_testkit::fake_bytes;

fn rels(files: &[&str]) -> Vec<RelPath> {
    files.iter().map(|f| rp(f)).collect()
}

fn adopt_plan(fx: &Fixture, name: &str, files: &[&str]) -> gmm_engine::Result<gmm_engine::Plan> {
    let fs = StdFs;
    plan_adopt(
        &fx.ctx(&fs),
        ModId::new(),
        name,
        Category::Vehicles,
        None,
        &rels(files),
        false,
    )
}

// ---- 1. adopt -------------------------------------------------------------------------------

#[test]
fn adopt_enables_mod_captures_vault_and_leaves_game_unchanged() {
    let fx = Fixture::new();
    let before = fx.snapshot();
    let id = fx.adopt("M1", M1_FILES);
    assert_eq!(
        fx.snapshot(),
        before,
        "adoption must not modify the game folder"
    );
    assert_eq!(fx.mod_state(id), ModState::Enabled);
    let entries = fx.db.list_mod_entries(id).expect("entries");
    assert_eq!(entries.len(), M1_FILES.len());
    let fs = StdFs;
    for e in &entries {
        assert!(fx.vault.verify(&fs, &e.sha256).expect("verify"));
        assert!(fx.db.get_vault_object(&e.sha256).expect("db").is_some());
        assert_eq!(e.size, 300);
    }
    assert!(fx.leftovers().is_empty());
    fx.assert_protected_intact();
}

#[test]
fn adopt_refuses_protected_files() {
    let fx = Fixture::new();
    for f in ["common.rpf", "COMMON.RPF", "update/update.rpf", "GTA5.exe"] {
        let err = adopt_plan(&fx, "Bad", &[f]).expect_err("must refuse");
        assert!(
            matches!(err, Error::OwnershipUncertain { .. }),
            "{f}: {err}"
        );
    }
}

#[test]
fn adopt_refuses_files_of_an_enabled_mod_and_missing_files() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    let err = adopt_plan(&fx, "Other", &["Trainer.asi"]).expect_err("owned");
    assert!(matches!(err, Error::PathConflict { .. }));
    fx.disable(m1);
    let err = adopt_plan(&fx, "Other", &["Trainer.asi"]).expect_err("missing");
    assert!(matches!(err, Error::NotFound { .. }));
}

#[test]
fn adopt_validates_input() {
    let fx = Fixture::new();
    assert!(matches!(
        adopt_plan(&fx, "  ", &["Trainer.asi"]),
        Err(Error::Domain(_))
    ));
    assert!(matches!(
        adopt_plan(&fx, "X", &[]),
        Err(Error::InvalidInput { .. })
    ));
    assert!(matches!(
        adopt_plan(&fx, "X", &["Trainer.asi", "TRAINER.ASI"]),
        Err(Error::InvalidInput { .. })
    ));
    fx.install
        .write("scripts/a.dll.gmm-tmp-0123abcd", b"x")
        .expect("write");
    assert!(matches!(
        adopt_plan(&fx, "X", &["scripts/a.dll.gmm-tmp-0123abcd"]),
        Err(Error::InvalidInput { .. })
    ));
    fx.adopt("Taken", &["scripts/keep.txt"]);
    assert!(matches!(
        adopt_plan(&fx, "Taken", &["Trainer.asi"]),
        Err(Error::InvalidInput { .. })
    ));
}

#[cfg(unix)]
#[test]
fn adopt_refuses_symlinks_hard_links_and_symlinked_directories() {
    let fx = Fixture::new();
    let outside = tempfile::TempDir::new().expect("outside");
    std::fs::write(outside.path().join("real.dll"), b"x").expect("write");
    std::os::unix::fs::symlink(outside.path().join("real.dll"), fx.install.path("link.dll"))
        .expect("symlink");
    assert!(matches!(
        adopt_plan(&fx, "A", &["link.dll"]),
        Err(Error::OwnershipUncertain { .. })
    ));

    std::fs::hard_link(fx.install.path("Trainer.ini"), fx.install.path("hard.ini"))
        .expect("hard link");
    for f in ["Trainer.ini", "hard.ini"] {
        assert!(
            matches!(
                adopt_plan(&fx, "B", &[f]),
                Err(Error::OwnershipUncertain { .. })
            ),
            "{f}"
        );
    }

    std::os::unix::fs::symlink(outside.path(), fx.install.path("evil")).expect("dir symlink");
    assert!(matches!(
        adopt_plan(&fx, "C", &["evil/real.dll"]),
        Err(Error::PathOutsideRoot { .. })
    ));
}

// ---- 2. disable / enable round trip ---------------------------------------------------------

#[test]
fn disable_then_enable_restores_byte_identical_files() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    let enabled = fx.snapshot();

    fx.disable(m1);
    assert_eq!(fx.mod_state(m1), ModState::Disabled);
    for f in M1_FILES {
        assert!(!fx.install.exists(f), "{f} must be gone");
    }
    let fs = StdFs;
    for e in fx.db.list_mod_entries(m1).expect("entries") {
        assert!(fx.vault.verify(&fs, &e.sha256).expect("verify"));
    }
    assert!(fx.leftovers().is_empty());

    fx.enable(m1);
    assert_eq!(fx.mod_state(m1), ModState::Enabled);
    assert_eq!(fx.snapshot(), enabled, "byte-identical after re-enable");
    for f in M1_FILES {
        assert_eq!(fx.install.read(f).expect("read"), fake_bytes(f, 300));
    }
    assert!(fx.leftovers().is_empty());
    fx.assert_protected_intact();
}

#[test]
fn wrong_state_is_rejected() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    let fs = StdFs;
    assert!(matches!(
        plan_enable(&fx.ctx(&fs), &[m1]),
        Err(Error::InvalidState { .. })
    ));
    fx.disable(m1);
    assert!(matches!(
        plan_disable(&fx.ctx(&fs), &[m1]),
        Err(Error::InvalidState { .. })
    ));
    assert!(matches!(
        plan_enable(&fx.ctx(&fs), &[ModId::new()]),
        Err(Error::NotFound { .. })
    ));
    assert!(matches!(
        plan_enable(&fx.ctx(&fs), &[m1, m1]),
        Err(Error::InvalidInput { .. })
    ));
}

// ---- 3. nested directories ------------------------------------------------------------------

#[test]
fn enable_creates_missing_dirs_and_rollback_removes_only_those() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    fx.install.remove("scripts/sub").expect("remove dirs");
    let pre = fx.snapshot();

    let fs = StdFs;
    let plan = plan_enable(&fx.ctx(&fs), &[m1]).expect("plan");
    let dirs: Vec<&str> = plan
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::EnsureDir)
        .map(|s| s.rel_path.as_str())
        .collect();
    assert_eq!(
        dirs,
        ["scripts/sub", "scripts/sub/deep"],
        "outermost first, no existing dir"
    );

    // Fail on the very last rename: everything done so far, dirs included, must be undone.
    let last_rename = plan
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::PlaceFromVault)
        .count();
    let faulty = FaultyFs::with_plan(
        StdFs,
        FaultPlan {
            op: OpMatch::Op(FsOp::Rename),
            nth: last_rename,
            mode: FaultMode::Error,
        },
    );
    let err = execute(&fx.ctx(&faulty), plan).expect_err("must fail");
    assert!(matches!(err, Error::Fs(_)));
    assert_eq!(fx.snapshot(), pre);
    assert!(fx.install.exists("scripts/keep.txt"));
    assert!(!fx.install.exists("scripts/sub"));
    assert_eq!(fx.mod_state(m1), ModState::Disabled);

    fx.enable(m1);
    assert!(fx.install.exists("scripts/sub/deep/x.dll"));
    // Disabling never removes directories, existing or created.
    fx.disable(m1);
    assert!(fx.install.exists("scripts/sub/deep"));
}

// ---- 4. conflicts ---------------------------------------------------------------------------

#[test]
fn enable_fails_when_target_is_occupied_by_an_unmanaged_file() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    fx.install
        .write("scripts/y.ini", b"user file")
        .expect("write");
    let pre = fx.snapshot();
    let fs = StdFs;
    let err = plan_enable(&fx.ctx(&fs), &[m1]).expect_err("conflict");
    assert!(matches!(&err, Error::PathConflict { path } if path.as_str() == "scripts/y.ini"));
    assert_eq!(fx.snapshot(), pre);
}

#[test]
fn conflict_appearing_after_planning_rolls_back_without_touching_the_foreign_file() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    fx.install.remove("scripts/sub").expect("remove dirs");
    let fs = StdFs;
    let plan = plan_enable(&fx.ctx(&fs), &[m1]).expect("plan");
    fx.install
        .write("scripts/y.ini", b"appeared later")
        .expect("write");
    let err = execute(&fx.ctx(&fs), plan).expect_err("conflict");
    assert!(matches!(err, Error::PathConflict { .. }));
    assert_eq!(
        fx.install.read("scripts/y.ini").expect("read"),
        b"appeared later"
    );
    for f in ["Trainer.asi", "Trainer.ini", "scripts/sub"] {
        assert!(!fx.install.exists(f), "{f} must have been rolled back");
    }
    let tx = fx.tx_ids();
    assert_eq!(fx.tx_status(tx[0]), TransactionStatus::RolledBack);
    assert!(fx.leftovers().is_empty());
}

fn two_alternatives(fx: &Fixture) -> (ModId, ModId) {
    let a = fx.adopt("A", &["Trainer.asi"]);
    fx.disable(a);
    fx.install
        .write("Trainer.asi", &fake_bytes("alt", 99))
        .expect("write");
    let b = fx.adopt("B", &["Trainer.asi"]);
    (a, b)
}

#[test]
fn second_mod_with_the_same_path_cannot_be_enabled() {
    let fx = Fixture::new();
    let (a, b) = two_alternatives(&fx);
    assert_eq!(fx.mod_state(a), ModState::Disabled);
    let fs = StdFs;
    let err = plan_enable(&fx.ctx(&fs), &[a]).expect_err("conflict");
    assert!(matches!(err, Error::PathConflict { .. }));
    // After disabling B the path is free and the file is gone, so A can be enabled.
    fx.disable(b);
    fx.enable(a);
    assert_eq!(fx.mod_state(a), ModState::Enabled);
}

#[test]
fn two_mods_in_the_same_enable_plan_cannot_share_a_path() {
    let fx = Fixture::new();
    let (a, b) = two_alternatives(&fx);
    fx.disable(b);
    let fs = StdFs;
    let err = plan_enable(&fx.ctx(&fs), &[a, b]).expect_err("conflict");
    assert!(matches!(err, Error::PathConflict { .. }));
}

#[test]
fn set_states_swaps_alternatives_in_one_transaction() {
    let fx = Fixture::new();
    let (a, b) = two_alternatives(&fx);
    let b_content = fx.install.read("Trainer.asi").expect("read");

    fx.set_states(&[(a, true), (b, false)]);
    assert_eq!(fx.mod_state(a), ModState::Enabled);
    assert_eq!(fx.mod_state(b), ModState::Disabled);
    assert_ne!(fx.install.read("Trainer.asi").expect("read"), b_content);

    // Mods already in the desired state are skipped: nothing left to do.
    let fs = StdFs;
    let plan = plan_set_states(&fx.ctx(&fs), &[(a, true), (b, false)]).expect("plan");
    assert!(plan.steps.is_empty());
    assert!(matches!(
        execute(&fx.ctx(&fs), plan),
        Err(Error::InvalidInput { .. })
    ));

    fx.set_states(&[(a, false), (b, true)]);
    assert_eq!(fx.install.read("Trainer.asi").expect("read"), b_content);
    assert!(fx.leftovers().is_empty());
}

// ---- 5. drift -------------------------------------------------------------------------------

#[test]
fn modified_file_blocks_disable_and_reconcile_tracks_drift() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.install
        .write("Trainer.ini", b"edited by the user")
        .expect("write");
    let pre = fx.snapshot();
    let fs = StdFs;

    let err = plan_disable(&fx.ctx(&fs), &[m1]).expect_err("drift");
    assert!(matches!(&err, Error::Drift { path } if path.as_str() == "Trainer.ini"));
    assert_eq!(fx.snapshot(), pre);

    let items = reconcile(&fs, &fx.db, &fx.vault, fx.install.root(), fx.inst).expect("reconcile");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DriftKind::Modified);
    assert_eq!(items[0].mod_id, m1);
    assert_eq!(fx.mod_state(m1), ModState::Drifted);
    assert_eq!(fx.snapshot(), pre, "reconcile never modifies files");
    assert!(matches!(
        plan_disable(&fx.ctx(&fs), &[m1]),
        Err(Error::InvalidState { .. })
    ));

    fx.install.remove("Trainer.ini").expect("remove");
    let items = reconcile(&fs, &fx.db, &fx.vault, fx.install.root(), fx.inst).expect("reconcile");
    assert_eq!(items[0].kind, DriftKind::Missing);

    fx.install
        .write("Trainer.ini", &fake_bytes("Trainer.ini", 300))
        .expect("restore");
    let items = reconcile(&fs, &fx.db, &fx.vault, fx.install.root(), fx.inst).expect("reconcile");
    assert!(items.is_empty());
    assert_eq!(fx.mod_state(m1), ModState::Enabled);
}

#[test]
fn drift_appearing_after_planning_rolls_back_and_keeps_the_user_edit() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    let fs = StdFs;
    let plan = plan_disable(&fx.ctx(&fs), &[m1]).expect("plan");
    fx.install
        .write("scripts/y.ini", b"edited late")
        .expect("write");
    let err = execute(&fx.ctx(&fs), plan).expect_err("drift");
    assert!(matches!(err, Error::Drift { .. }));
    assert_eq!(
        fx.install.read("scripts/y.ini").expect("read"),
        b"edited late"
    );
    for f in ["Trainer.asi", "Trainer.ini", "scripts/sub/deep/x.dll"] {
        assert!(fx.install.exists(f), "{f} must be restored");
    }
    assert_eq!(fx.mod_state(m1), ModState::Enabled);
    assert_eq!(fx.tx_status(fx.tx_ids()[0]), TransactionStatus::RolledBack);
    assert!(fx.leftovers().is_empty());
}

// ---- 6. integrity ---------------------------------------------------------------------------

fn corrupt_vault_object(fx: &Fixture, id: ModId, rel: &str) {
    let entry = fx
        .db
        .list_mod_entries(id)
        .expect("entries")
        .into_iter()
        .find(|e| e.rel_path.as_str() == rel)
        .expect("entry");
    let object = fx.vault.object_path(&entry.sha256);
    StdFs.set_readonly(&object, false).expect("make writable");
    std::fs::write(&object, b"corrupt").expect("corrupt");
}

#[test]
fn corrupt_vault_object_is_never_placed_or_trusted() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    corrupt_vault_object(&fx, m1, "scripts/y.ini");
    let pre = fx.snapshot();
    let fs = StdFs;
    let err = plan_enable(&fx.ctx(&fs), &[m1]).expect_err("corrupt");
    assert!(matches!(err, Error::VaultCorrupt { .. }));
    assert_eq!(fx.snapshot(), pre);

    // Reconcile reports it and flags the mod.
    let items = reconcile(&fs, &fx.db, &fx.vault, fx.install.root(), fx.inst).expect("reconcile");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DriftKind::VaultCorrupt);
    assert_eq!(fx.mod_state(m1), ModState::Error);
}

#[test]
fn enabled_mod_with_corrupt_vault_cannot_be_disabled() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    corrupt_vault_object(&fx, m1, "Trainer.asi");
    let pre = fx.snapshot();
    let fs = StdFs;
    let err = plan_disable(&fx.ctx(&fs), &[m1]).expect_err("corrupt");
    assert!(matches!(err, Error::VaultCorrupt { .. }));
    assert_eq!(
        fx.snapshot(),
        pre,
        "I-02: nothing is removed without a verified vault copy"
    );
}

#[test]
fn vault_corrupted_after_planning_is_caught_at_execution() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    let fs = StdFs;

    // Disable: the tombstone step re-verifies the vault object.
    let plan = plan_disable(&fx.ctx(&fs), &[m1]).expect("plan");
    corrupt_vault_object(&fx, m1, "scripts/sub/deep/x.dll");
    let pre = fx.snapshot();
    let err = execute(&fx.ctx(&fs), plan).expect_err("corrupt");
    assert!(matches!(err, Error::VaultCorrupt { .. }));
    assert_eq!(fx.snapshot(), pre);
    assert_eq!(fx.mod_state(m1), ModState::Enabled);

    // Enable: the corrupt object is never copied into the game folder.
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    let plan = plan_enable(&fx.ctx(&fs), &[m1]).expect("plan");
    corrupt_vault_object(&fx, m1, "scripts/y.ini");
    let pre = fx.snapshot();
    let err = execute(&fx.ctx(&fs), plan).expect_err("corrupt");
    assert!(matches!(err, Error::VaultCorrupt { .. }));
    assert_eq!(fx.snapshot(), pre);
    assert!(!fx.install.exists("scripts/y.ini"));
    assert!(fx.leftovers().is_empty());
}

// ---- 10. protected originals ----------------------------------------------------------------

#[test]
fn executor_refuses_steps_on_protected_paths_even_if_a_plan_contains_them() {
    let fx = Fixture::new();
    let fs = StdFs;
    let mut plan = adopt_plan(&fx, "Fine", &["Trainer.asi"]).expect("plan");
    let sha = gmm_domain::Sha256::parse(
        &gmm_platform::sha256_file(&fs, &fx.install.path("common.rpf")).expect("hash"),
    )
    .expect("sha");
    for kind in [
        StepKind::TombstoneGameFile,
        StepKind::PlaceFromVault,
        StepKind::CaptureToVault,
    ] {
        plan.steps[0].kind = kind;
        plan.steps[0].rel_path = rp("common.rpf");
        plan.steps[0].sha256 = Some(sha.clone());
        let before = fx.snapshot();
        let err = execute(&fx.ctx(&fs), plan.clone()).expect_err("protected");
        assert!(
            matches!(err, Error::OwnershipUncertain { .. }),
            "{kind}: {err}"
        );
        assert_eq!(fx.snapshot(), before);
        fx.assert_protected_intact();
    }
    assert!(fx.leftovers().is_empty());
}

#[test]
fn protected_files_survive_every_operation() {
    let fx = Fixture::new();
    let m1 = fx.adopt("M1", M1_FILES);
    fx.disable(m1);
    fx.enable(m1);
    fx.assert_protected_intact();
    assert!(fx.install.exists("GTA5.exe"));
}
