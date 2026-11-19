//! Profile services: from current, apply, Vanilla, previews, guards and CRUD.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{
    ErrorCode, NullSink, ProfileEntryInput, VANILLA_PROFILE_ID, profile_name_suggestions,
};
use gmm_domain::{DesiredState, ModState, StepKind, TransactionKind, TransactionStatus};

fn last_tx(e: &Env) -> gmm_app::TransactionDto {
    e.app
        .list_transactions(&e.id, 1, 0)
        .expect("history")
        .items
        .remove(0)
}

#[test]
fn profile_from_current_then_apply_restores_the_saved_states_in_one_transaction() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    set(&e, &a.id, false);
    let profile = e
        .app
        .create_profile_from_current(&e.id, "  Visual Overhaul ")
        .expect("create");
    assert_eq!(profile.name, "Visual Overhaul");
    assert_eq!(profile.entry_count, 2);
    assert!(!profile.built_in);

    let detail = e.app.get_profile(&profile.id).expect("detail");
    let by_name = |n: &str| {
        detail
            .entries
            .iter()
            .find(|x| x.mod_name == n)
            .expect("entry")
            .desired
    };
    assert_eq!(by_name("Trainer"), DesiredState::Disabled);
    assert_eq!(by_name("Alpha"), DesiredState::Enabled);

    // Change everything, then apply.
    set(&e, &a.id, true);
    set(&e, &b.id, false);
    let before = tx_count(&e);
    let applied = e
        .app
        .apply_profile(&e.id, &profile.id, &NullSink)
        .expect("apply");
    assert_eq!(applied.len(), 2);
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);
    assert_eq!(state_of(&e, &b.id), ModState::Enabled);
    assert!(!e.install.exists("trainer.asi"));
    assert!(e.install.exists("scripts/alpha.dll"));
    assert_eq!(tx_count(&e), before + 1, "one transaction for both mods");
    let tx = last_tx(&e);
    assert_eq!(tx.kind, TransactionKind::Profile);
    assert_eq!(tx.status, TransactionStatus::Committed);
    assert_eq!(tx.description, "Applied profile");
    assert_eq!(tx.mod_names, ["Alpha", "Trainer"]);

    // Applying again changes nothing and journals nothing.
    e.app
        .apply_profile(&e.id, &profile.id, &NullSink)
        .expect("again");
    assert_eq!(tx_count(&e), before + 1);
}

#[test]
fn list_puts_vanilla_first_and_counts_entries() {
    let e = env();
    let list = e.app.list_profiles(&e.id).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, VANILLA_PROFILE_ID);
    assert_eq!(list[0].name, "Vanilla");
    assert!(list[0].built_in);
    assert_eq!(list[0].entry_count, 0);

    adopt(&e, &TRAINER, "Trainer");
    adopt(&e, &ALPHA, "Alpha");
    let p = e.app.create_profile_from_current(&e.id, "Zeta").expect("p");
    e.app
        .create_profile_from_current(&e.id, "alpha set")
        .expect("p2");
    let list = e.app.list_profiles(&e.id).expect("list");
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Vanilla", "alpha set", "Zeta"]);
    assert_eq!(list[0].entry_count, 2);
    assert_eq!(list[2].id, p.id);
    assert_eq!(list[2].entry_count, 2);
    let json = serde_json::to_value(&list[0]).expect("json");
    assert_eq!(json["builtIn"], true);
    assert!(json["entryCount"].is_number());
}

#[test]
fn drifted_and_damaged_mods_are_recorded_as_enabled() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    e.install.write("trainer.asi", b"edited").expect("write");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &a.id), ModState::Drifted);
    let p = e.app.create_profile_from_current(&e.id, "Now").expect("p");
    let d = e.app.get_profile(&p.id).expect("detail");
    assert_eq!(d.entries[0].desired, DesiredState::Enabled);
}

#[test]
fn vanilla_disables_every_mod_and_preview_matches_execution() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let c = adopt(&e, &BETA, "Beta");
    set(&e, &c.id, false);

    let preview = e
        .app
        .preview_apply_profile(&e.id, VANILLA_PROFILE_ID)
        .expect("preview");
    let mut affected: Vec<&str> = preview
        .mods_affected
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    affected.sort_unstable();
    assert_eq!(affected, ["Alpha", "Trainer"], "Beta is already disabled");
    assert!(
        preview
            .mods_affected
            .iter()
            .all(|m| m.from == ModState::Enabled && m.to == ModState::Disabled)
    );
    assert_eq!(preview.actions.len(), 4);
    assert!(
        preview
            .actions
            .iter()
            .all(|x| x.kind == StepKind::TombstoneGameFile)
    );

    let before_preview = snapshot(&e);
    let _ = e
        .app
        .preview_apply_profile(&e.id, VANILLA_PROFILE_ID)
        .expect("preview");
    assert!(before_preview.diff(&snapshot(&e)).is_empty(), "no writes");

    let applied = e
        .app
        .apply_profile(&e.id, VANILLA_PROFILE_ID, &NullSink)
        .expect("apply");
    assert_eq!(applied.len(), 3);
    assert!(applied.iter().all(|m| m.state == ModState::Disabled));
    for id in [&a.id, &b.id, &c.id] {
        assert_eq!(state_of(&e, id), ModState::Disabled);
    }
    for f in TRAINER.iter().chain(&ALPHA).chain(&BETA) {
        assert!(!e.install.exists(f), "{f}");
    }
    let tx = last_tx(&e);
    assert_eq!(tx.step_count as usize, preview.actions.len());
    assert_eq!(tx.mod_names, ["Alpha", "Trainer"]);

    // The plan for enabling shows the vault copies being placed back.
    let p = e
        .app
        .create_profile_from_current(&e.id, "Empty")
        .expect("p");
    e.app
        .update_profile_entries(
            &p.id,
            vec![ProfileEntryInput {
                mod_id: a.id.clone(),
                desired: DesiredState::Enabled,
            }],
        )
        .expect("entries");
    let preview = e.app.preview_apply_profile(&e.id, &p.id).expect("preview");
    assert_eq!(preview.mods_affected.len(), 1);
    assert_eq!(preview.mods_affected[0].to, ModState::Enabled);
    assert!(preview.bytes_to_copy > 0);
    assert!(
        preview
            .actions
            .iter()
            .any(|x| x.kind == StepKind::PlaceFromVault && x.mod_name == "Trainer")
    );
}

#[test]
fn mods_outside_the_profile_stay_as_they_are() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let p = e
        .app
        .create_profile_from_current(&e.id, "Only trainer")
        .expect("p");
    let late = adopt(&e, &ALPHA, "Alpha");
    set(&e, &a.id, false);
    let applied = e.app.apply_profile(&e.id, &p.id, &NullSink).expect("apply");
    assert_eq!(applied.len(), 1);
    assert_eq!(state_of(&e, &a.id), ModState::Enabled);
    assert_eq!(
        state_of(&e, &late.id),
        ModState::Enabled,
        "not in the profile"
    );
}

#[test]
fn guards_block_apply_and_leave_everything_unchanged() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let p = e
        .app
        .create_profile_from_current(&e.id, "All on")
        .expect("p");
    set(&e, &a.id, false);

    let before = snapshot(&e);
    let txs = tx_count(&e);
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    let blocked = err(e.app.apply_profile(&e.id, &p.id, &NullSink));
    assert_eq!(blocked.code, ErrorCode::GameRunning);
    assert_eq!(
        err(e.app.apply_profile(&e.id, VANILLA_PROFILE_ID, &NullSink)).code,
        ErrorCode::GameRunning
    );
    e.os.0.lock().expect("lock").clear();
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(tx_count(&e), txs);
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);

    // A drifted mod in the profile blocks the whole apply, with no partial change.
    e.install
        .write("scripts/alpha.dll", b"edited")
        .expect("write");
    let before = snapshot(&e);
    let blocked = err(e.app.apply_profile(&e.id, &p.id, &NullSink));
    assert_eq!(blocked.code, ErrorCode::DriftDetected);
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &a.id), ModState::Disabled, "Trainer untouched");
    assert_eq!(state_of(&e, &b.id), ModState::Drifted);
    assert_eq!(tx_count(&e), txs);
    assert_eq!(
        err(e.app.preview_apply_profile(&e.id, &p.id)).code,
        ErrorCode::DriftDetected
    );
}

#[test]
fn profile_bookkeeping_rules() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    assert_eq!(profile_name_suggestions().len(), 5);
    assert_eq!(profile_name_suggestions()[0], "Visual Overhaul");

    let p = e.app.create_profile_from_current(&e.id, "One").expect("p");
    assert_eq!(
        err(e.app.create_profile_from_current(&e.id, "one")).code,
        ErrorCode::InvalidInput,
        "names are unique ignoring case"
    );
    assert_eq!(
        err(e.app.create_profile_from_current(&e.id, "VANILLA")).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e.app.create_profile_from_current(&e.id, "  ")).code,
        ErrorCode::InvalidInput
    );

    let renamed = e.app.rename_profile(&p.id, "Two").expect("rename");
    assert_eq!(renamed.name, "Two");

    let d = e
        .app
        .update_profile_entries(
            &p.id,
            vec![ProfileEntryInput {
                mod_id: b.id.clone(),
                desired: DesiredState::Disabled,
            }],
        )
        .expect("update");
    assert_eq!(d.summary.entry_count, 1);
    assert_eq!(d.entries[0].mod_name, "Alpha");
    let dup = vec![
        ProfileEntryInput {
            mod_id: a.id.clone(),
            desired: DesiredState::Enabled,
        },
        ProfileEntryInput {
            mod_id: a.id.clone(),
            desired: DesiredState::Disabled,
        },
    ];
    assert_eq!(
        err(e.app.update_profile_entries(&p.id, dup)).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(e.app.get_profile(&p.id).expect("same").entries.len(), 1);

    // Vanilla is built in.
    for r in [
        e.app.rename_profile(VANILLA_PROFILE_ID, "x").map(|_| ()),
        e.app.delete_profile(VANILLA_PROFILE_ID),
        e.app.get_profile(VANILLA_PROFILE_ID).map(|_| ()),
    ] {
        assert_eq!(err(r).code, ErrorCode::InvalidInput);
    }

    // Deleting a profile does not touch the mods; a profile of another id is not found.
    e.app.delete_profile(&p.id).expect("delete");
    assert_eq!(err(e.app.get_profile(&p.id)).code, ErrorCode::NotFound);
    assert_eq!(err(e.app.delete_profile(&p.id)).code, ErrorCode::NotFound);
    assert_eq!(
        err(e.app.apply_profile(&e.id, &p.id, &NullSink)).code,
        ErrorCode::NotFound
    );
    assert_eq!(state_of(&e, &a.id), ModState::Enabled);
    assert_eq!(e.app.list_profiles(&e.id).expect("list").len(), 1);
}
