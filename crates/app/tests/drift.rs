//! Drift resolution through the app, plus the disabled-mod scan rule and forgetting damaged mods.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use common::*;
use gmm_app::{ErrorCode, InventoryFilterDto, NullSink};
use gmm_domain::{Classification, ModState, Sha256, TransactionKind};
use gmm_engine::Vault;

const NOTHING: &str = "Nothing to resolve: the files already match.";

fn class_of(e: &Env, rel: &str) -> Option<Classification> {
    e.app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                classifications: vec![],
                search: Some(rel.to_owned()),
                limit: 100,
                offset: 0,
            },
        )
        .expect("inventory")
        .items
        .iter()
        .find(|i| i.rel_path.eq_ignore_ascii_case(rel))
        .map(|i| i.classification)
}

fn vault_objects(e: &Env) -> usize {
    fn count(dir: &std::path::Path) -> usize {
        fs::read_dir(dir).map_or(0, |rd| {
            rd.filter_map(Result::ok)
                .map(|d| {
                    let p = d.path();
                    if p.is_dir() { count(&p) } else { 1 }
                })
                .sum()
        })
    }
    count(&e.dirs.vault_dir.join("objects"))
}

fn original(name: &str) -> Vec<u8> {
    gmm_testkit::fake_bytes(name, 256)
}

#[test]
fn restore_puts_the_stored_files_back_and_keeps_the_users_version() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    assert_eq!(vault_objects(&e), 2);
    e.install
        .write("trainer.asi", b"edited by hand")
        .expect("write");
    e.install.remove("trainer.ini").expect("remove");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &m.id), ModState::Drifted);

    let restored = e.app.restore_mod(&m.id, &NullSink).expect("restore");
    assert_eq!(restored.state, ModState::Enabled);
    assert_eq!(
        e.install.read("trainer.asi").expect("read"),
        original("trainer.asi")
    );
    assert_eq!(
        e.install.read("trainer.ini").expect("read"),
        original("trainer.ini")
    );
    assert_eq!(
        vault_objects(&e),
        3,
        "the edited version was saved to the vault"
    );
    assert_eq!(class_of(&e, "trainer.asi"), Some(Classification::Managed));

    let tx = e
        .app
        .list_transactions(&e.id, 1, 0)
        .expect("history")
        .items
        .remove(0);
    assert_eq!(tx.kind, TransactionKind::Restore);
    assert_eq!(tx.description, "Restored mod “Trainer”");

    let again = err(e.app.restore_mod(&m.id, &NullSink));
    assert_eq!(again.code, ErrorCode::InvalidInput);
    assert_eq!(again.message, NOTHING);
    assert_eq!(err(e.app.readopt_mod(&m.id, &NullSink)).message, NOTHING);
}

#[test]
fn restore_finds_drift_the_database_does_not_know_yet() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    e.install.write("trainer.ini", b"edited").expect("write");
    assert_eq!(state_of(&e, &m.id), ModState::Enabled, "not reconciled yet");
    e.app.restore_mod(&m.id, &NullSink).expect("restore");
    assert_eq!(
        e.install.read("trainer.ini").expect("read"),
        original("trainer.ini")
    );
}

#[test]
fn readopt_accepts_the_current_files_without_touching_the_game_folder() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    e.install
        .write("trainer.asi", b"my own build")
        .expect("write");
    e.install.remove("trainer.ini").expect("remove");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    let before = snapshot(&e);

    let m2 = e.app.readopt_mod(&m.id, &NullSink).expect("readopt");
    assert_eq!(m2.state, ModState::Enabled);
    assert_eq!(m2.entry_count, 1, "the missing file is dropped");
    assert!(
        before.diff(&snapshot(&e)).is_empty(),
        "game folder untouched"
    );
    assert_eq!(class_of(&e, "trainer.asi"), Some(Classification::Managed));
    let tx = e
        .app
        .list_transactions(&e.id, 1, 0)
        .expect("history")
        .items
        .remove(0);
    assert_eq!(tx.kind, TransactionKind::Adopt);
    assert_eq!(tx.description, "Re-adopted mod “Trainer”");

    // The accepted version is now the stored one: disable and enable bring it back.
    set(&e, &m.id, false);
    assert!(!e.install.exists("trainer.asi"));
    set(&e, &m.id, true);
    assert_eq!(
        e.install.read("trainer.asi").expect("read"),
        b"my own build"
    );
    assert!(!e.install.exists("trainer.ini"));
}

#[test]
fn guards_apply_to_restore_but_readopt_needs_no_write_access() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    e.install.write("trainer.asi", b"edited").expect("write");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    let before = snapshot(&e);

    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    assert_eq!(
        err(e.app.restore_mod(&m.id, &NullSink)).code,
        ErrorCode::GameRunning
    );
    assert_eq!(
        err(e.app.readopt_mod(&m.id, &NullSink)).code,
        ErrorCode::GameRunning
    );
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Drifted);
    e.os.0.lock().expect("lock").clear();

    // Read-only folder check uses Unix permissions; the Windows equivalent is covered by the
    // manual M7 checklist (docs/windows-validation.md §4.3).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        struct Restore<'a>(&'a std::path::Path);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
            }
        }
        let _restore = Restore(e.install.root());
        fs::set_permissions(e.install.root(), fs::Permissions::from_mode(0o555)).expect("chmod");
        let probe = e.install.root().join("probe-check");
        if fs::write(&probe, b"x").is_ok() {
            let _ = fs::remove_file(&probe);
            return; // privileges that ignore permissions: nothing more to test
        }
        assert_eq!(
            err(e.app.restore_mod(&m.id, &NullSink)).code,
            ErrorCode::InstallNotWritable
        );
        assert_eq!(state_of(&e, &m.id), ModState::Drifted);
        let ok = e
            .app
            .readopt_mod(&m.id, &NullSink)
            .expect("readopt on read-only folder");
        assert_eq!(ok.state, ModState::Enabled);
    }
}

#[test]
fn unknown_mod_and_bad_ids_are_rejected() {
    let e = env();
    let missing = gmm_domain::ModId::new().to_string();
    assert_eq!(
        err(e.app.restore_mod(&missing, &NullSink)).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        err(e.app.readopt_mod(&missing, &NullSink)).code,
        ErrorCode::NotFound
    );
    assert!(e.app.restore_mod("nope", &NullSink).is_err());
}

#[test]
fn a_file_at_a_disabled_mods_path_is_not_managed() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    set(&e, &m.id, false);
    assert_eq!(class_of(&e, "trainer.asi"), None);

    e.install
        .write("trainer.asi", b"a different file")
        .expect("write");
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
    let class = class_of(&e, "trainer.asi").expect("listed");
    assert!(
        !matches!(class, Classification::Managed | Classification::Drifted),
        "got {class:?}"
    );
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);

    let before = snapshot(&e);
    let blocked = err(e.app.set_mod_enabled(&m.id, true, &NullSink));
    assert_eq!(blocked.code, ErrorCode::PathConflict);
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);
    assert_eq!(
        e.install.read("trainer.asi").expect("read"),
        b"a different file"
    );
}

#[test]
fn a_damaged_mod_can_be_forgotten_but_not_resolved() {
    let e = env();
    let m = adopt(&e, &ALPHA, "Alpha");
    set(&e, &m.id, false);
    let sha = e.app.get_mod(&m.id).expect("mod").entries[0].sha256.clone();
    let vault = Vault::new(&e.dirs.vault_dir);
    let object = vault.object_path(&Sha256::parse(&sha).expect("sha"));
    // Vault objects are read-only: replace the object with damaged bytes.
    fs::remove_file(&object).expect("remove the read-only object");
    fs::write(&object, b"damaged").expect("corrupt");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &m.id), ModState::Error);

    assert_eq!(
        err(e.app.restore_mod(&m.id, &NullSink)).code,
        ErrorCode::HashMismatch
    );
    assert_eq!(
        err(e.app.readopt_mod(&m.id, &NullSink)).code,
        ErrorCode::HashMismatch
    );
    e.app.forget_mod(&m.id).expect("forget a damaged mod");
    assert!(e.app.list_mods(&e.id).expect("list").is_empty());
}
