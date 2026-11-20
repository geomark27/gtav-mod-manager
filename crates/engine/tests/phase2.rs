//! Phase 2: imported (disabled) mods, dependencies and vault garbage collection.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use std::path::PathBuf;

use common::{Fixture, rp};
use gmm_domain::{Category, Edition, ModId, ModState, RelPath, Sha256, TransactionId};
use gmm_engine::{
    AdoptPayload, Error, GcReport, dependency_closure, import_to_vault, plan_disable, plan_enable,
    plan_register_imported, plan_set_states, vault_gc, vault_gc_candidates,
};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, Fs, OpMatch, StdFs};
use gmm_testkit::base_files;
use tempfile::TempDir;

/// Writes `files` under a staging directory outside the game folder.
fn stage(files: &[(&str, &[u8])]) -> (TempDir, Vec<(RelPath, PathBuf)>) {
    let dir = TempDir::new().expect("staging dir");
    let mut out = Vec::new();
    for (i, (target, data)) in files.iter().enumerate() {
        let p = dir.path().join(format!("staged-{i}.bin"));
        std::fs::write(&p, data).expect("write staged");
        out.push((rp(target), p));
    }
    (dir, out)
}

/// Imports `files` into the vault and registers them as a disabled mod.
fn register(fx: &Fixture, name: &str, files: &[(&str, &[u8])]) -> ModId {
    let (_dir, staged) = stage(files);
    let entries =
        import_to_vault(&StdFs, &fx.vault, &staged, TransactionId::new()).expect("import");
    let id = ModId::new();
    fx.run(|ctx| plan_register_imported(ctx, id, name, Category::Vehicles, None, &entries));
    id
}

#[test]
fn imported_mod_registers_disabled_without_touching_the_game_then_enables() {
    let fx = Fixture::new();
    let before = fx.snapshot();
    let hashes = fx.game_hashes();
    let id = register(
        &fx,
        "Imported",
        &[
            ("scripts/imp/new.asi", b"asi bytes"),
            ("plugins/deep/x.ini", b"ini bytes"),
        ],
    );
    assert_eq!(fx.mod_state(id), ModState::Disabled);
    assert_eq!(fx.snapshot(), before, "game folder unchanged");
    assert_eq!(fx.game_hashes(), hashes);
    assert_eq!(fx.db.list_mod_entries(id).expect("entries").len(), 2);
    for e in fx.db.list_mod_entries(id).expect("entries") {
        assert!(fx.vault.verify(&StdFs, &e.sha256).expect("verify"));
        assert!(fx.db.get_vault_object(&e.sha256).expect("db").is_some());
    }
    assert!(fx.leftovers().is_empty());

    fx.enable(id);
    assert_eq!(fx.mod_state(id), ModState::Enabled);
    assert_eq!(
        fx.install.read("scripts/imp/new.asi").expect("read"),
        b"asi bytes"
    );
    assert_eq!(
        fx.install.read("plugins/deep/x.ini").expect("read"),
        b"ini bytes"
    );
    assert!(fx.reconcile().is_empty());
    fx.assert_protected_intact();
    fx.disable(id);
    assert_eq!(fx.game_hashes(), hashes);
}

#[test]
fn enabling_an_imported_mod_checks_conflicts() {
    let fx = Fixture::new();
    // Trainer.asi already exists in the game folder (unmanaged).
    let id = register(&fx, "Imported", &[("Trainer.asi", b"other")]);
    let fs = StdFs;
    let err = plan_enable(&fx.ctx(&fs), &[id]).expect_err("conflict");
    assert!(matches!(err, Error::PathConflict { .. }), "{err}");
}

#[test]
fn register_validates_input() {
    let fx = Fixture::new();
    let fs = StdFs;
    let ctx = fx.ctx(&fs);
    let (_d, staged) = stage(&[("scripts/a.asi", b"a"), ("scripts/b.asi", b"b")]);
    let entries = import_to_vault(&fs, &fx.vault, &staged, TransactionId::new()).expect("import");
    let plan = |name: &str, e: &[(RelPath, Sha256, u64)]| {
        plan_register_imported(&ctx, ModId::new(), name, Category::Vehicles, None, e)
    };

    // Protected original file.
    let protected = rp(base_files(Edition::Legacy)[0]);
    let bad = vec![(protected, entries[0].1.clone(), entries[0].2)];
    assert!(matches!(
        plan("P", &bad),
        Err(Error::OwnershipUncertain { .. })
    ));
    // Empty, duplicate keys, bad name, engine artifact, missing vault object.
    assert!(matches!(plan("E", &[]), Err(Error::InvalidInput { .. })));
    let dup = vec![
        (rp("scripts/a.asi"), entries[0].1.clone(), entries[0].2),
        (rp("Scripts/A.asi"), entries[1].1.clone(), entries[1].2),
    ];
    assert!(matches!(plan("D", &dup), Err(Error::InvalidInput { .. })));
    assert!(matches!(plan("  ", &entries), Err(Error::Domain(_))));
    let missing = vec![(
        rp("scripts/c.asi"),
        Sha256::parse(&"ab".repeat(32)).expect("sha"),
        1,
    )];
    assert!(matches!(
        plan("V", &missing),
        Err(Error::VaultCorrupt { .. })
    ));

    // Duplicate name.
    fx.run(|c| plan_register_imported(c, ModId::new(), "Same", Category::Vehicles, None, &entries));
    assert!(matches!(
        plan("Same", &entries),
        Err(Error::InvalidInput { .. })
    ));
}

#[test]
fn import_rejects_missing_and_non_regular_sources() {
    let fx = Fixture::new();
    let dir = TempDir::new().expect("dir");
    let missing = vec![(rp("a.asi"), dir.path().join("nope"))];
    assert!(matches!(
        import_to_vault(&StdFs, &fx.vault, &missing, TransactionId::new()),
        Err(Error::NotFound { .. })
    ));
    let as_dir = vec![(rp("a.asi"), dir.path().to_path_buf())];
    assert!(matches!(
        import_to_vault(&StdFs, &fx.vault, &as_dir, TransactionId::new()),
        Err(Error::InvalidInput { .. })
    ));
}

#[test]
fn old_adopt_payload_without_initial_state_parses_as_enabled() {
    let json = format!(
        "{{\"mod_id\":\"{}\",\"name\":\"M\",\"category\":\"vehicles\",\"notes\":null,\
         \"entries\":[],\"included_unknown\":false}}",
        ModId::new()
    );
    let p: AdoptPayload = serde_json::from_str(&json).expect("parse");
    assert_eq!(p.initial_state, ModState::Enabled);
}

/// A (Trainer.asi, Trainer.ini) enabled; B (scripts/y.ini) enabled and requires A.
fn dependent_pair(fx: &Fixture) -> (ModId, ModId) {
    let a = fx.adopt("Loader", &["Trainer.asi", "Trainer.ini"]);
    let b = fx.adopt("Plugin", &["scripts/y.ini"]);
    fx.db.add_dependency(b, a, chrono::Utc::now()).expect("dep");
    (a, b)
}

#[test]
fn dependency_rules_for_enable_disable_and_set_states() {
    let fx = Fixture::new();
    let (a, b) = dependent_pair(&fx);
    let fs = StdFs;
    let ctx = fx.ctx(&fs);

    match plan_disable(&ctx, &[a]) {
        Err(Error::DependentEnabled { r#mod, dependent }) => {
            assert_eq!((r#mod.as_str(), dependent.as_str()), ("Loader", "Plugin"));
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(matches!(
        plan_set_states(&ctx, &[(a, false)]),
        Err(Error::DependentEnabled { .. })
    ));

    // Valid combined plan: dependent first or together, both orders.
    fx.set_states(&[(a, false), (b, false)]);
    assert_eq!(fx.mod_state(a), ModState::Disabled);
    assert_eq!(fx.mod_state(b), ModState::Disabled);

    match plan_enable(&ctx, &[b]) {
        Err(Error::DependencyDisabled { r#mod, dependency }) => {
            assert_eq!((r#mod.as_str(), dependency.as_str()), ("Plugin", "Loader"));
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(matches!(
        plan_set_states(&ctx, &[(b, true)]),
        Err(Error::DependencyDisabled { .. })
    ));
    assert!(matches!(
        plan_set_states(&ctx, &[(b, true), (a, false)]),
        Err(Error::DependencyDisabled { .. })
    ));

    // Enabling both together is fine; so is enabling the dependency first.
    fx.set_states(&[(b, true), (a, true)]);
    assert_eq!(fx.mod_state(a), ModState::Enabled);
    assert_eq!(fx.mod_state(b), ModState::Enabled);
    fx.set_states(&[(b, false)]);
    fx.disable(a);
    fx.enable(a);
    fx.enable(b);
    assert!(fx.reconcile().is_empty());
}

#[test]
fn plan_enable_of_both_in_one_call_is_valid() {
    let fx = Fixture::new();
    let (a, b) = dependent_pair(&fx);
    fx.set_states(&[(a, false), (b, false)]);
    fx.run(|c| plan_enable(c, &[a, b]));
    assert_eq!(fx.mod_state(b), ModState::Enabled);
}

#[test]
fn closure_follows_dependencies_or_dependents() {
    let fx = Fixture::new();
    let (a, b) = dependent_pair(&fx);
    let c = fx.adopt("Addon", &["scripts/sub/deep/x.dll"]);
    let d = fx.adopt("Solo", &["scripts/keep.txt"]);
    fx.db.add_dependency(c, b, chrono::Utc::now()).expect("dep");
    let sorted = |mut v: Vec<ModId>| {
        v.sort();
        v
    };
    assert_eq!(
        sorted(dependency_closure(&fx.db, fx.inst, &[c], true).expect("c")),
        sorted(vec![a, b, c])
    );
    assert_eq!(
        sorted(dependency_closure(&fx.db, fx.inst, &[a], false).expect("a")),
        sorted(vec![a, b, c])
    );
    assert_eq!(
        sorted(dependency_closure(&fx.db, fx.inst, &[a], true).expect("a")),
        vec![a]
    );
    assert_eq!(
        sorted(dependency_closure(&fx.db, fx.inst, &[d], true).expect("d")),
        vec![d]
    );
    let ordered = dependency_closure(&fx.db, fx.inst, &[c, c], true).expect("dup");
    assert_eq!(ordered[0], c);
    assert_eq!(ordered.len(), 3);
}

/// Two orphan vault objects plus one referenced object and one stray file.
fn gc_fixture() -> (Fixture, Vec<(Sha256, u64)>, PathBuf) {
    let fx = Fixture::new();
    fx.adopt("M1", &["Trainer.asi"]);
    let (_d, staged) = stage(&[("o/one.bin", b"orphan one"), ("o/two.bin", b"orphan two!!")]);
    let imported =
        import_to_vault(&StdFs, &fx.vault, &staged, TransactionId::new()).expect("import");
    let orphans: Vec<(Sha256, u64)> = imported.into_iter().map(|(_, s, n)| (s, n)).collect();
    for (sha, size) in &orphans {
        fx.db
            .upsert_vault_object(sha, *size, chrono::Utc::now())
            .expect("register orphan");
    }
    let stray = fx.vault.root.join("objects").join("zz").join("junk.bin");
    std::fs::create_dir_all(stray.parent().expect("parent")).expect("mkdir");
    std::fs::write(&stray, b"not ours").expect("stray");
    (fx, orphans, stray)
}

#[test]
fn gc_deletes_only_unreferenced_objects_and_reports_strays() {
    let (fx, orphans, stray) = gc_fixture();
    let fs = StdFs;
    let report = vault_gc_candidates(&fs, &fx.db, &fx.vault).expect("candidates");
    let mut want = orphans.clone();
    want.sort();
    assert_eq!(report.unreferenced, want);
    assert_eq!(report.stray_files, vec![stray.clone()]);

    let referenced = fx
        .db
        .list_mod_entries(fx.db.list_mods(fx.inst).expect("mods")[0].id)
        .expect("entries")[0]
        .sha256
        .clone();
    assert!(matches!(
        vault_gc(&fs, &fx.db, &fx.vault, std::slice::from_ref(&referenced)),
        Err(Error::InvalidState { .. })
    ));
    assert!(fx.vault.verify(&fs, &referenced).expect("verify"));

    let shas: Vec<Sha256> = orphans.iter().map(|(s, _)| s.clone()).collect();
    let freed = vault_gc(&fs, &fx.db, &fx.vault, &shas).expect("gc");
    assert_eq!(freed, orphans.iter().map(|(_, n)| n).sum::<u64>());
    for sha in &shas {
        assert!(!fs.exists(&fx.vault.object_path(sha)).expect("exists"));
        assert!(fx.db.get_vault_object(sha).expect("db").is_none());
    }
    assert!(fx.vault.verify(&fs, &referenced).expect("verify"));
    assert!(stray.exists(), "strays are never deleted");
    let after = vault_gc_candidates(&fs, &fx.db, &fx.vault).expect("candidates");
    assert_eq!(
        after,
        GcReport {
            unreferenced: vec![],
            stray_files: vec![stray]
        }
    );
    // Idempotent: unknown digests are skipped.
    assert_eq!(vault_gc(&fs, &fx.db, &fx.vault, &shas).expect("again"), 0);
    fx.assert_protected_intact();
}

#[test]
fn gc_keeps_objects_of_unresolved_transactions_and_imported_mods() {
    let fx = Fixture::new();
    let id = register(&fx, "Imported", &[("scripts/imp/new.asi", b"asi bytes")]);
    let fs = StdFs;
    assert!(
        vault_gc_candidates(&fs, &fx.db, &fx.vault)
            .expect("candidates")
            .unreferenced
            .is_empty()
    );
    // Delete-then-orphan: forgetting the mod makes its objects collectable.
    fx.db.delete_mod(id).expect("delete mod");
    assert_eq!(
        vault_gc_candidates(&fs, &fx.db, &fx.vault)
            .expect("candidates")
            .unreferenced
            .len(),
        1
    );
}

#[test]
fn gc_crash_at_every_operation_stays_consistent() {
    let dry = gc_fixture();
    let counter = FaultyFs::new(StdFs);
    let shas: Vec<Sha256> = dry.1.iter().map(|(s, _)| s.clone()).collect();
    vault_gc(&counter, &dry.0.db, &dry.0.vault, &shas).expect("dry run");
    let n = counter.ops_count();
    assert!(n >= 4, "{n}");

    for nth in 1..=n {
        let (fx, orphans, stray) = gc_fixture();
        let shas: Vec<Sha256> = orphans.iter().map(|(s, _)| s.clone()).collect();
        let fs = FaultyFs::with_plan(
            StdFs,
            FaultPlan {
                op: OpMatch::Any,
                nth,
                mode: FaultMode::Crash,
            },
        );
        let err = vault_gc(&fs, &fx.db, &fx.vault, &shas).expect_err("crash");
        assert!(matches!(err, Error::Crashed), "op {nth}: {err}");
        drop(fs);

        // Consistent: every object is either intact (row + file) or a row without a file.
        let report = vault_gc_candidates(&StdFs, &fx.db, &fx.vault).expect("candidates");
        for (sha, _) in &orphans {
            let row = fx.db.get_vault_object(sha).expect("db").is_some();
            let file = StdFs.exists(&fx.vault.object_path(sha)).expect("exists");
            assert!(row || !file, "op {nth}: file without row for {sha}");
            if row {
                assert!(
                    report.unreferenced.iter().any(|(s, _)| s == sha),
                    "op {nth}: leftover row not reported"
                );
            }
        }
        assert_eq!(report.stray_files, vec![stray.clone()], "op {nth}");

        // A second run finishes the job, including rows whose file is already gone.
        vault_gc(&StdFs, &fx.db, &fx.vault, &shas).expect("second gc");
        for (sha, _) in &orphans {
            assert!(
                fx.db.get_vault_object(sha).expect("db").is_none(),
                "op {nth}"
            );
            assert!(!StdFs.exists(&fx.vault.object_path(sha)).expect("exists"));
        }
        assert!(
            vault_gc_candidates(&StdFs, &fx.db, &fx.vault)
                .expect("candidates")
                .unreferenced
                .is_empty()
        );
        fx.assert_protected_intact();
    }
    println!("fault-injection (crash) vault_gc: {n} cases");
}

#[test]
fn provenance_travels_with_the_plan_and_old_journals_read_as_manual() {
    use gmm_domain::{ModSource, ModSourceKind};

    let fx = Fixture::new();
    let (_dir, staged) = stage(&[("scripts/p.asi", b"p bytes")]);
    let entries =
        import_to_vault(&StdFs, &fx.vault, &staged, TransactionId::new()).expect("import");
    let id = ModId::new();
    let source = ModSource::zip_import("p.zip", Some(Sha256::from_bytes([7; 32])));
    let wanted = source.clone();
    fx.run(move |ctx| {
        plan_register_imported(ctx, id, "P", Category::Vehicles, None, &entries)
            .map(|p| p.with_source(source))
    });
    let m = fx.db.get_mod(id).expect("db").expect("mod");
    assert_eq!(m.source, wanted);
    assert_eq!(m.source.kind, ModSourceKind::ZipImport);

    // with_source on a plan that creates no mod changes nothing.
    let fs = StdFs;
    let ctx = fx.ctx(&fs);
    let plan = plan_enable(&ctx, &[id]).expect("plan");
    let same = plan.clone().with_source(ModSource::manual());
    assert_eq!(same.summary, plan.summary);

    // An adoption payload journaled before provenance existed reads back as manual.
    let old = r#"{"mod_id":"00000000-0000-7000-8000-000000000001","name":"Old",
        "category":"vehicles","notes":null,"entries":[],"included_unknown":false}"#;
    let payload: AdoptPayload = serde_json::from_str(old).expect("old payload");
    assert_eq!(payload.source, ModSource::manual());
    assert_eq!(payload.initial_state, ModState::Enabled);
}
