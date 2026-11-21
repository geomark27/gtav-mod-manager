//! Red Dead Redemption 2: the same use cases on an installation of the second game.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{ErrorCode, NullSink};
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
fn gta_v_signatures_are_never_proposed() {
    let e = rdr2_env();
    let d = e.app.detect_mods(&e.id).expect("detect");
    assert!(
        d.proposals.iter().all(|p| p.signature_id.is_none()),
        "{:?}",
        d.proposals
    );
    assert!(d.ambiguous.is_empty());
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
