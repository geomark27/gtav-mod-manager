//! Online readiness: the check, "prepare for online", and what each check reports.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{
    CheckStatus, ErrorCode, NullSink, OnlineCheckDto, ReadinessCheckDto, ReadinessCheckId,
    ReadinessVerdict,
};
use gmm_domain::{BaselineTrust, Edition, ModState, TransactionId, tomb_name};
use gmm_testkit::FakeInstall;

fn install(extra: &[(&str, &[u8])]) -> FakeInstall {
    let mut b = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .script_hook()
        .asi_mod("trainer");
    for (path, data) in extra {
        b = b.file(path, data.to_vec());
    }
    b.build().expect("install")
}

fn get(r: &OnlineCheckDto, id: ReadinessCheckId) -> &ReadinessCheckDto {
    r.checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id:?}"))
}

fn check(e: &Env) -> OnlineCheckDto {
    e.app
        .check_online_readiness(&e.id, &NullSink)
        .expect("check")
}

fn prepare(e: &Env) -> OnlineCheckDto {
    e.app.prepare_for_online(&e.id, &NullSink).expect("prepare")
}

/// An installation whose trusted baseline was captured while it was clean; the mod files are
/// added afterwards (as a user installs mods after verifying the game).
fn clean_then_modded(extra: &[(&str, &[u8])]) -> Env {
    let e = env_with(
        FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .build()
            .expect("install"),
    );
    e.app
        .capture_baseline(&e.id, true, &NullSink)
        .expect("baseline");
    for f in [
        "ScriptHookV.dll",
        "dinput8.dll",
        "trainer.asi",
        "trainer.ini",
    ] {
        e.install.write(f, f.as_bytes()).expect("write");
    }
    for (path, data) in extra {
        e.install.write(path, data).expect("write");
    }
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
    e
}

#[test]
fn the_check_lists_what_is_left_and_never_touches_the_game_folder() {
    let e = env_with(install(&[]));
    adopt(&e, &["ScriptHookV.dll"], "Script Hook V");
    let before = snapshot(&e);
    let r = check(&e);
    assert_eq!(snapshot(&e).diff(&before), vec![], "read-only");
    assert_eq!(r.verdict, ReadinessVerdict::Blocked);
    assert!(r.scanned_at.is_some());
    assert!(!r.vanilla_applied);
    assert_eq!(
        get(&r, ReadinessCheckId::GameClosed).status,
        CheckStatus::Pass
    );
    assert_eq!(
        get(&r, ReadinessCheckId::Recovery).status,
        CheckStatus::Pass
    );
    let managed = get(&r, ReadinessCheckId::ManagedMods);
    assert_eq!(managed.status, CheckStatus::Fail);
    assert_eq!(managed.items, ["Script Hook V"]);
    let files = get(&r, ReadinessCheckId::ModFiles);
    assert_eq!(files.status, CheckStatus::Fail);
    assert!(
        files.items.contains(&"dinput8.dll".to_owned()),
        "{:?}",
        files.items
    );
    assert!(files.items.contains(&"trainer.asi".to_owned()));
    // No baseline: the original-file check cannot run.
    assert_eq!(r.baseline, BaselineTrust::None);
    assert_eq!(
        get(&r, ReadinessCheckId::Originals).status,
        CheckStatus::Warn
    );
    // Display order.
    let ids: Vec<ReadinessCheckId> = r.checks.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            ReadinessCheckId::GameClosed,
            ReadinessCheckId::Recovery,
            ReadinessCheckId::ManagedMods,
            ReadinessCheckId::ModFiles,
            ReadinessCheckId::UnknownFiles,
            ReadinessCheckId::Originals,
            ReadinessCheckId::Leftovers,
        ]
    );
    let json = serde_json::to_value(&r).expect("json");
    assert_eq!(json["verdict"], "blocked");
    assert_eq!(json["checks"][2]["id"], "managed_mods");
}

#[test]
fn prepare_disables_managed_mods_and_reports_unmanaged_ones() {
    let e = env_with(install(&[]));
    let shv = adopt(&e, &["ScriptHookV.dll", "dinput8.dll"], "Script Hook V");
    let r = prepare(&e);
    assert!(r.vanilla_applied);
    assert!(r.action_error.is_none());
    assert_eq!(state_of(&e, &shv.id), ModState::Disabled);
    assert!(!e.install.exists("ScriptHookV.dll"));
    assert!(
        e.install.exists("trainer.asi"),
        "unmanaged files are never removed"
    );
    assert_eq!(
        get(&r, ReadinessCheckId::ManagedMods).status,
        CheckStatus::Pass
    );
    let files = get(&r, ReadinessCheckId::ModFiles);
    assert_eq!(files.status, CheckStatus::Fail);
    assert_eq!(files.items, ["trainer.asi"]);
    assert!(
        e.app
            .vanilla_status(&e.id)
            .expect("status")
            .restore_point
            .is_some(),
        "the restore point brings the mods back"
    );
}

#[test]
fn everything_managed_and_a_trusted_baseline_gives_a_clear_result() {
    let e = clean_then_modded(&[]);
    adopt(&e, &["ScriptHookV.dll", "dinput8.dll"], "Script Hook V");
    adopt(&e, &["trainer.asi", "trainer.ini"], "Trainer");
    let r = prepare(&e);
    assert_eq!(r.baseline, BaselineTrust::Trusted);
    assert!(
        r.checks.iter().all(|c| c.status == CheckStatus::Pass),
        "{:?}",
        r.checks
    );
    assert_eq!(r.verdict, ReadinessVerdict::Clear);
    // Restoring brings everything back.
    e.app
        .restore_from_vanilla(&e.id, &NullSink)
        .expect("restore");
    assert!(e.install.exists("trainer.asi") && e.install.exists("ScriptHookV.dll"));
}

#[test]
fn a_modified_original_fails_and_is_never_repaired() {
    let e = clean_then_modded(&[]);
    e.install
        .write("update/update.rpf", b"modded")
        .expect("write");
    let r = prepare(&e);
    let originals = get(&r, ReadinessCheckId::Originals);
    assert_eq!(originals.status, CheckStatus::Fail);
    assert_eq!(originals.items, ["update/update.rpf"]);
    assert_eq!(
        e.install.read("update/update.rpf").expect("read"),
        b"modded",
        "originals are only reported"
    );
}

#[test]
fn unknown_files_and_leftovers_only_warn() {
    let tomb = tomb_name("old.asi", TransactionId::new());
    let e = clean_then_modded(&[("notes.txt", b"hello"), (&tomb, b"x")]);
    adopt(&e, &["ScriptHookV.dll", "dinput8.dll"], "Script Hook V");
    adopt(&e, &["trainer.asi", "trainer.ini"], "Trainer");
    let r = prepare(&e);
    assert_eq!(get(&r, ReadinessCheckId::UnknownFiles).items, ["notes.txt"]);
    assert_eq!(
        get(&r, ReadinessCheckId::UnknownFiles).status,
        CheckStatus::Warn
    );
    assert_eq!(
        get(&r, ReadinessCheckId::Leftovers).status,
        CheckStatus::Warn
    );
    assert_eq!(get(&r, ReadinessCheckId::Leftovers).count, 1);
    assert_eq!(r.verdict, ReadinessVerdict::Review);
}

#[test]
fn a_running_game_fails_the_check_and_prepare_reports_why_nothing_changed() {
    let e = env_with(install(&[]));
    let shv = adopt(&e, &["ScriptHookV.dll"], "Script Hook V");
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    let r = prepare(&e);
    assert!(!r.vanilla_applied);
    assert_eq!(
        r.action_error.as_ref().map(|a| a.code),
        Some(ErrorCode::GameRunning)
    );
    let game = get(&r, ReadinessCheckId::GameClosed);
    assert_eq!(game.status, CheckStatus::Fail);
    assert_eq!(game.items, ["GTA5.exe"]);
    assert_eq!(state_of(&e, &shv.id), ModState::Enabled);
    assert!(e.install.exists("ScriptHookV.dll"));
    e.os.0.lock().expect("lock").clear();
}

#[test]
fn a_drifted_mod_blocks_prepare_and_is_named() {
    let e = env_with(install(&[]));
    adopt(&e, &["trainer.asi", "trainer.ini"], "Trainer");
    e.install.write("trainer.ini", b"edited").expect("write");
    let r = prepare(&e);
    assert_eq!(
        r.action_error.as_ref().map(|a| a.code),
        Some(ErrorCode::DriftDetected)
    );
    assert_eq!(
        get(&r, ReadinessCheckId::ManagedMods).items,
        ["Trainer (drifted)"]
    );
    assert!(
        get(&r, ReadinessCheckId::ModFiles)
            .items
            .contains(&"trainer.ini".to_owned())
    );
}

#[test]
fn unknown_installation_is_not_found() {
    let e = env();
    let missing = "00000000-0000-7000-8000-000000000000";
    assert_eq!(
        err(e.app.check_online_readiness(missing, &NullSink)).code,
        ErrorCode::NotFound
    );
}

#[test]
fn mod_files_in_a_baseline_captured_with_mods_are_still_reported() {
    // The user captured a "trusted" baseline while the mods were installed.
    let e = env_with(install(&[]));
    e.app
        .capture_baseline(&e.id, true, &NullSink)
        .expect("baseline");
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
    let r = check(&e);
    let files = get(&r, ReadinessCheckId::ModFiles);
    assert_eq!(files.status, CheckStatus::Fail);
    for f in ["dinput8.dll", "ScriptHookV.dll", "trainer.asi"] {
        assert!(
            files.items.contains(&format!("{f} (in the baseline)")),
            "{f}: {:?}",
            files.items
        );
    }
    // trainer.ini has no mod pattern: it is not reported.
    assert!(!files.items.iter().any(|i| i.starts_with("trainer.ini")));
}
