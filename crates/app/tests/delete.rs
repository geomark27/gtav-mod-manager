//! Deleting a mod: files out of the game folder, rows gone, unused vault copies freed.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{ErrorCode, NullSink};
use gmm_domain::{Edition, ModState};
use gmm_testkit::FakeInstall;

fn delete(e: &Env, id: &str) -> gmm_app::DeleteModResultDto {
    e.app.delete_mod(id, &NullSink).expect("delete")
}

fn mod_count(e: &Env) -> usize {
    e.app.list_mods(&e.id).expect("list").len()
}

#[test]
fn an_enabled_mod_leaves_the_game_folder_and_the_vault() {
    let e = env();
    let trainer = adopt(&e, &TRAINER, "Trainer");
    adopt(&e, &BETA, "Beta");
    let before = snapshot(&e);

    let r = delete(&e, &trainer.id);
    assert_eq!(r.name, "Trainer");
    assert_eq!(r.files_removed, 2);
    assert!(r.vault_bytes_freed > 0);
    for f in TRAINER {
        assert!(!e.install.exists(f), "{f} still in the game folder");
    }
    let changed: Vec<String> = before
        .diff(&snapshot(&e))
        .into_iter()
        .map(|d| format!("{d:?}"))
        .collect();
    assert_eq!(
        changed.len(),
        2,
        "only the trainer files changed: {changed:?}"
    );
    assert!(
        changed.iter().all(|d| d.contains("trainer.")),
        "{changed:?}"
    );
    for f in BETA {
        assert!(e.install.exists(f), "{f} of another mod stays");
    }
    assert_eq!(mod_count(&e), 1);
    let vault = e.app.vault_report().expect("vault");
    assert!(vault.unreferenced.is_empty(), "{:?}", vault.unreferenced);
    assert_eq!(
        err(e.app.delete_mod(&trainer.id, &NullSink)).code,
        ErrorCode::NotFound
    );
}

#[test]
fn a_disabled_mod_is_deleted_without_touching_the_game_folder() {
    let e = env();
    let m = adopt(&e, &ALPHA, "Alpha");
    assert_eq!(set(&e, &m.id, false).state, ModState::Disabled);
    let before = snapshot(&e);

    let r = delete(&e, &m.id);
    assert_eq!(r.files_removed, 0);
    assert!(r.vault_bytes_freed > 0);
    assert_eq!(snapshot(&e).diff(&before), vec![]);
    assert_eq!(mod_count(&e), 0);
    assert!(e.app.vault_report().expect("vault").unreferenced.is_empty());
}

#[test]
fn a_required_mod_is_refused_until_its_dependents_are_gone() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &BETA, "Beta");
    e.app.add_dependency(&a.id, &b.id).expect("dependency");
    let before = snapshot(&e);

    let refused = err(e.app.delete_mod(&b.id, &NullSink));
    assert_eq!(refused.code, ErrorCode::DependencyBlocked);
    assert_eq!(refused.message, "Other mods require \"Beta\".");
    assert_eq!(refused.details.as_deref(), Some("Trainer"));
    assert_eq!(snapshot(&e).diff(&before), vec![]);
    assert_eq!(mod_count(&e), 2);

    delete(&e, &a.id);
    delete(&e, &b.id);
    assert_eq!(mod_count(&e), 0);
}

#[test]
fn a_running_game_blocks_without_changes() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let before = snapshot(&e);
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];

    let blocked = err(e.app.delete_mod(&m.id, &NullSink));
    assert_eq!(blocked.code, ErrorCode::GameRunning);
    assert_eq!(snapshot(&e).diff(&before), vec![]);
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
}

#[test]
fn a_drifted_mod_is_refused() {
    let e = env();
    let m = adopt(&e, &ALPHA, "Alpha");
    e.install
        .write("scripts/alpha.dll", b"edited by hand")
        .expect("write");
    let refused = err(e.app.delete_mod(&m.id, &NullSink));
    assert_eq!(refused.code, ErrorCode::DriftDetected);
    assert_eq!(
        e.install.read("scripts/alpha.dll").expect("read"),
        b"edited by hand",
        "the edited file is kept"
    );
    assert_eq!(mod_count(&e), 1);
}

#[test]
fn a_stored_copy_another_mod_uses_is_kept() {
    let e = env_with(
        FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .file("scripts/one/same.dll", b"identical bytes".to_vec())
            .file("scripts/two/same.dll", b"identical bytes".to_vec())
            .build()
            .expect("install"),
    );
    let one = adopt(&e, &["scripts/one/same.dll"], "One");
    let two = adopt(&e, &["scripts/two/same.dll"], "Two");

    let r = delete(&e, &one.id);
    assert_eq!(r.vault_bytes_freed, 0, "Two still uses the same content");
    set(&e, &two.id, false);
    set(&e, &two.id, true);
    assert_eq!(
        e.install.read("scripts/two/same.dll").expect("read"),
        b"identical bytes"
    );

    let r = delete(&e, &two.id);
    assert_eq!(r.vault_bytes_freed, "identical bytes".len() as u64);
}

#[test]
fn a_red_dead_mod_imported_into_gta_v_can_be_deleted() {
    // The owner's case: RDR2 files added to a GTA V installation by mistake.
    let e = env_with(
        FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .file("lml/BetterHorses/install.xml", b"<xml/>".to_vec())
            .file("lml/BetterHorses/horse.ymt", b"ymt".to_vec())
            .build()
            .expect("install"),
    );
    let files = ["lml/BetterHorses/install.xml", "lml/BetterHorses/horse.ymt"];
    let m = adopt(&e, &files, "Better Horses");
    let r = delete(&e, &m.id);
    assert_eq!(r.files_removed, 2);
    for f in files {
        assert!(!e.install.exists(f), "{f}");
    }
    assert_eq!(mod_count(&e), 0);
}
