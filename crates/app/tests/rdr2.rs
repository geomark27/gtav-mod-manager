//! Red Dead Redemption 2: the same use cases on an installation of the second game.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{
    AdoptDetectedRequest, CheckStatus, DetectedAdoptItem, ErrorCode, NullSink, ReadinessCheckId,
};
use gmm_domain::{Edition, Game, ModState};
use gmm_testkit::FakeInstall;

const HOOK: [&str; 1] = ["ScriptHookRDR2.dll"];
const LML: [&str; 2] = ["lml/BetterHorses/install.xml", "lml/BetterHorses/horse.ymt"];

fn rdr2_env() -> Env {
    env_with(
        FakeInstall::builder(Edition::Rdr2)
            .with_base_files()
            .file(HOOK[0], b"hook".to_vec())
            .file("dinput8.dll", b"loader".to_vec())
            .file("NativeTrainer.asi", b"trainer".to_vec())
            .file(LML[0], b"<xml/>".to_vec())
            .file(LML[1], b"ymt".to_vec())
            // A GTA V file means nothing here.
            .file("ScriptHookV.dll", b"gta hook".to_vec())
            .build()
            .expect("install"),
    )
}

#[test]
fn registers_as_red_dead_redemption_2() {
    let e = rdr2_env();
    let list = e.app.list_installations().expect("list");
    assert_eq!(list.len(), 1);
    let dto = &list[0];
    assert_eq!(dto.game, Game::Rdr2);
    assert_eq!(dto.edition, Edition::Rdr2);
    assert!(
        dto.display_name.starts_with("Red Dead Redemption 2"),
        "{}",
        dto.display_name
    );
}

#[test]
fn detects_red_dead_mods_and_never_gta_v_ones() {
    let e = rdr2_env();
    let d = e.app.detect_mods(&e.id).expect("detect");
    let found: Vec<(&str, Option<&str>)> = d
        .proposals
        .iter()
        .map(|p| (p.name.as_str(), p.signature_id.as_deref()))
        .collect();
    assert_eq!(
        found,
        [
            ("ASI Loader", Some("rdr2-asi-loader")),
            ("Native Trainer", Some("rdr2-native-trainer")),
            ("Script Hook RDR2", Some("rdr2-script-hook")),
            ("BetterHorses", None),
            ("ScriptHookV", None),
        ]
    );
    let gta = d
        .proposals
        .iter()
        .find(|p| p.name == "ScriptHookV")
        .expect("grouped");
    assert!(gta.includes_unknown, "a GTA V file is unknown here");
    assert!(d.ambiguous.is_empty());
}

#[test]
fn adopting_everything_links_the_trainer_to_script_hook_rdr2() {
    let e = rdr2_env();
    let before = snapshot(&e);
    let d = e.app.detect_mods(&e.id).expect("detect");
    let result = e
        .app
        .adopt_detected(
            AdoptDetectedRequest {
                installation_id: e.id.clone(),
                items: d
                    .proposals
                    .iter()
                    .map(|p| DetectedAdoptItem {
                        signature_id: p.signature_id.clone(),
                        name: p.name.clone(),
                        category: p.category,
                        rel_paths: p.files.iter().map(|f| f.rel_path.clone()).collect(),
                    })
                    .collect(),
                confirm_unknown: true,
            },
            &NullSink,
        )
        .expect("adopt");
    assert!(result.skipped.is_empty(), "{:?}", result.skipped);
    assert_eq!(result.adopted.len(), 5);
    assert_eq!(result.dependencies_linked, 2);
    assert_eq!(
        snapshot(&e).diff(&before),
        vec![],
        "the game folder is untouched"
    );
}

#[test]
fn enable_and_disable_restore_the_exact_files() {
    let e = rdr2_env();
    let before = snapshot(&e);
    let m = adopt(&e, &LML, "Better Horses");
    assert_eq!(
        snapshot(&e).diff(&before),
        vec![],
        "adopting changes nothing"
    );

    let off = set(&e, &m.id, false);
    assert_eq!(off.state, ModState::Disabled);
    for f in LML {
        assert!(!e.install.exists(f), "{f} still in the game folder");
    }
    assert!(e.install.exists(HOOK[0]), "other files stay");

    set(&e, &m.id, true);
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
    assert_eq!(snapshot(&e).diff(&before), vec![], "back to the same bytes");
}

#[test]
fn a_running_red_dead_redemption_2_blocks_changes() {
    let e = rdr2_env();
    let m = adopt(&e, &HOOK, "Script Hook RDR2");
    let before = snapshot(&e);
    *e.os.0.lock().expect("lock") = vec!["RDR2.exe".into()];

    let blocked = err(e.app.set_mod_enabled(&m.id, false, &NullSink));
    assert_eq!(blocked.code, ErrorCode::GameRunning);
    assert_eq!(blocked.details.as_deref(), Some("RDR2.exe"));
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
}

#[test]
fn a_running_gta_v_does_not_block_red_dead_redemption_2() {
    let e = rdr2_env();
    let m = adopt(&e, &HOOK, "Script Hook RDR2");
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into(), "PlayGTAV.exe".into()];
    assert!(
        e.app
            .guard_status(&e.id)
            .expect("guard")
            .game_running
            .is_empty()
    );
    assert_eq!(set(&e, &m.id, false).state, ModState::Disabled);

    // The shared Rockstar launcher still blocks it.
    *e.os.0.lock().expect("lock") = vec!["RockstarService.exe".into()];
    let blocked = err(e.app.set_mod_enabled(&m.id, true, &NullSink));
    assert_eq!(blocked.code, ErrorCode::GameRunning);
    assert_eq!(
        blocked.message,
        "Red Dead Redemption 2 or one of its launchers is running."
    );
}

#[test]
fn online_check_names_the_game_and_its_own_processes() {
    let e = rdr2_env();
    adopt(&e, &LML, "Better Horses");
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    let r = e.app.prepare_for_online(&e.id, &NullSink).expect("prepare");
    assert_eq!(r.game, Game::Rdr2);
    assert!(r.vanilla_applied);
    let status = |id: ReadinessCheckId| {
        r.checks
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.status)
            .expect("check")
    };
    assert_eq!(status(ReadinessCheckId::GameClosed), CheckStatus::Pass);
    assert_eq!(status(ReadinessCheckId::ManagedMods), CheckStatus::Pass);
    // Script Hook RDR2, its ASI loader and the trainer are not managed: mod files remain.
    let mod_files = r
        .checks
        .iter()
        .find(|c| c.id == ReadinessCheckId::ModFiles)
        .expect("mod files");
    assert_eq!(mod_files.status, CheckStatus::Fail);
    assert!(
        mod_files.items.iter().any(|i| i == "ScriptHookRDR2.dll"),
        "{:?}",
        mod_files.items
    );

    *e.os.0.lock().expect("lock") = vec!["RDR2.exe".into()];
    let r = e
        .app
        .check_online_readiness(&e.id, &NullSink)
        .expect("check");
    let closed = r
        .checks
        .iter()
        .find(|c| c.id == ReadinessCheckId::GameClosed)
        .expect("game closed");
    assert_eq!(closed.status, CheckStatus::Fail);
    assert_eq!(closed.items, ["RDR2.exe"]);
}
