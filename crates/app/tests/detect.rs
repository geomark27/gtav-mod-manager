//! Detected mods: proposals from the last scan, bulk adoption, provenance and dependencies.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{
    AdoptDetectedRequest, DetectedAdoptItem, DetectedModDto, DetectedModsDto, DetectionConfidence,
    ErrorCode, NullSink, RequirementStatus,
};
use gmm_domain::{Category, Edition, ModSourceKind, ModState};
use gmm_testkit::FakeInstall;

fn rampage_install(edition: Edition) -> FakeInstall {
    FakeInstall::builder(edition)
        .with_base_files()
        .script_hook()
        .file("ScriptHookVDotNet.asi", b"shvdn".to_vec())
        .file("ScriptHookVDotNet3.dll", b"shvdn3".to_vec())
        .file("scripts/RampageTrainer.dll", b"rampage".to_vec())
        .file("scripts/Rampage/config.ini", b"cfg".to_vec())
        .asi_mod("FastTravel")
        .build()
        .expect("install")
}

fn detect(e: &Env) -> DetectedModsDto {
    e.app.detect_mods(&e.id).expect("detect")
}

fn proposal<'a>(d: &'a DetectedModsDto, name: &str) -> &'a DetectedModDto {
    d.proposals
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no proposal {name}"))
}

fn as_item(p: &DetectedModDto) -> DetectedAdoptItem {
    DetectedAdoptItem {
        signature_id: p.signature_id.clone(),
        name: p.name.clone(),
        category: p.category,
        rel_paths: p.files.iter().map(|f| f.rel_path.clone()).collect(),
    }
}

fn adopt_all(e: &Env, d: &DetectedModsDto) -> gmm_app::AdoptDetectedResultDto {
    e.app
        .adopt_detected(
            AdoptDetectedRequest {
                installation_id: e.id.clone(),
                items: d.proposals.iter().map(as_item).collect(),
                confirm_unknown: true,
            },
            &NullSink,
        )
        .expect("adopt detected")
}

#[test]
fn proposals_name_known_mods_and_group_the_rest() {
    let e = env_with(rampage_install(Edition::Enhanced));
    let d = detect(&e);
    assert!(d.scanned_at.is_some());
    let names: Vec<&str> = d.proposals.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "ASI Loader",
            "Rampage Trainer",
            "Script Hook V",
            "Script Hook V .NET",
            "FastTravel"
        ]
    );
    let rampage = proposal(&d, "Rampage Trainer");
    assert_eq!(rampage.source_kind, ModSourceKind::Recognized);
    assert_eq!(rampage.confidence, DetectionConfidence::Medium);
    assert_eq!(rampage.files.len(), 2);
    assert_eq!(rampage.requires.len(), 1);
    assert_eq!(rampage.requires[0].name, "Script Hook V .NET");
    assert_eq!(rampage.requires[0].status, RequirementStatus::Detected);
    assert!(rampage.edition_supported);
    let fast = proposal(&d, "FastTravel");
    assert_eq!(fast.source_kind, ModSourceKind::Grouped);
    assert_eq!(fast.confidence, DetectionConfidence::Low);
    assert!(fast.includes_unknown, "FastTravel.ini has no mod pattern");
    assert_eq!(
        proposal(&d, "Script Hook V").confidence,
        DetectionConfidence::High
    );
    let json = serde_json::to_value(&d).expect("json");
    assert_eq!(json["proposals"][0]["sourceKind"], "recognized");
}

#[test]
fn adopt_all_creates_one_mod_per_proposal_with_provenance_and_dependencies() {
    let e = env_with(rampage_install(Edition::Legacy));
    let before = snapshot(&e);
    let txs = tx_count(&e);
    let d = detect(&e);
    let result = adopt_all(&e, &d);
    assert!(result.skipped.is_empty(), "{:?}", result.skipped);
    assert_eq!(result.adopted.len(), 5);
    assert_eq!(tx_count(&e), txs + 5, "one transaction per mod");
    assert_eq!(
        snapshot(&e).diff(&before),
        vec![],
        "the game folder is untouched"
    );
    // Requirements first.
    let order: Vec<&str> = result.adopted.iter().map(|m| m.name.as_str()).collect();
    let pos = |n: &str| order.iter().position(|x| *x == n).expect("adopted");
    assert!(pos("Script Hook V .NET") < pos("Rampage Trainer"));
    assert!(pos("Script Hook V") < pos("Script Hook V .NET"));

    let rampage = result
        .adopted
        .iter()
        .find(|m| m.name == "Rampage Trainer")
        .expect("rampage");
    assert_eq!(rampage.state, ModState::Enabled);
    assert_eq!(rampage.source.kind, ModSourceKind::Recognized);
    assert_eq!(rampage.source.detail.as_deref(), Some("rampage-trainer"));
    assert_eq!(rampage.depends_on_count, 1);
    let shvdn = result
        .adopted
        .iter()
        .find(|m| m.name == "Script Hook V .NET")
        .expect("shvdn");
    assert_eq!(
        shvdn.depends_on_count, 2,
        "Script Hook V and the ASI loader"
    );
    // SHVDN→SHV, SHVDN→ASI, Rampage→SHVDN.
    assert_eq!(result.dependencies_linked, 3);
    let fast = result
        .adopted
        .iter()
        .find(|m| m.name == "FastTravel")
        .expect("fast");
    assert_eq!(fast.source.kind, ModSourceKind::Grouped);

    let detail = e.app.get_mod(&rampage.id).expect("detail");
    assert_eq!(detail.depends_on[0].name, "Script Hook V .NET");

    // After the rescan nothing is left to propose.
    let d = detect(&e);
    assert!(d.proposals.is_empty(), "{:?}", d.proposals);
}

#[test]
fn managed_requirements_are_reported_and_linked() {
    let e = env_with(rampage_install(Edition::Legacy));
    // Script Hook V adopted by hand under another name: its files still satisfy the signature.
    adopt(&e, &["ScriptHookV.dll"], "My SHV");
    let d = detect(&e);
    let shvdn = proposal(&d, "Script Hook V .NET");
    let shv_req = shvdn
        .requires
        .iter()
        .find(|r| r.signature_id == "script-hook-v")
        .expect("req");
    assert_eq!(shv_req.status, RequirementStatus::Managed);
    assert_eq!(shv_req.mod_name.as_deref(), Some("My SHV"));
    let asi_req = shvdn
        .requires
        .iter()
        .find(|r| r.signature_id == "asi-loader")
        .expect("req");
    assert_eq!(asi_req.status, RequirementStatus::Detected);

    let r = e
        .app
        .adopt_detected(
            AdoptDetectedRequest {
                installation_id: e.id.clone(),
                items: vec![as_item(shvdn)],
                confirm_unknown: false,
            },
            &NullSink,
        )
        .expect("adopt");
    assert_eq!(r.adopted.len(), 1);
    assert_eq!(
        r.dependencies_linked, 1,
        "only the managed requirement is linked"
    );
}

#[test]
fn a_failing_proposal_is_skipped_and_the_others_are_adopted() {
    let e = env_with(rampage_install(Edition::Legacy));
    adopt(&e, &["FastTravel.asi"], "Script Hook V");
    let d = detect(&e);
    let items = vec![
        as_item(proposal(&d, "Script Hook V")),
        as_item(proposal(&d, "ASI Loader")),
    ];
    let r = e
        .app
        .adopt_detected(
            AdoptDetectedRequest {
                installation_id: e.id.clone(),
                items,
                confirm_unknown: false,
            },
            &NullSink,
        )
        .expect("adopt");
    assert_eq!(r.adopted.len(), 1);
    assert_eq!(r.adopted[0].name, "ASI Loader");
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].name, "Script Hook V");
    assert_eq!(r.skipped[0].error.code, ErrorCode::InvalidInput);
}

#[test]
fn requests_are_validated_before_anything_is_adopted() {
    let e = env_with(rampage_install(Edition::Legacy));
    let txs = tx_count(&e);
    let item = |name: &str, files: &[&str]| DetectedAdoptItem {
        signature_id: None,
        name: name.to_owned(),
        category: Category::ScriptsGameplay,
        rel_paths: files.iter().map(|f| (*f).to_owned()).collect(),
    };
    let req = |items: Vec<DetectedAdoptItem>| AdoptDetectedRequest {
        installation_id: e.id.clone(),
        items,
        confirm_unknown: true,
    };
    // Empty.
    assert_eq!(
        err(e.app.adopt_detected(req(vec![]), &NullSink)).code,
        ErrorCode::InvalidInput
    );
    // The same file in two mods.
    let f = err(e.app.adopt_detected(
        req(vec![
            item("A", &["ScriptHookV.dll"]),
            item("B", &["scripthookv.dll"]),
        ]),
        &NullSink,
    ));
    assert_eq!(f.code, ErrorCode::InvalidInput);
    // Unknown signature.
    let mut bad = item("C", &["ScriptHookV.dll"]);
    bad.signature_id = Some("nope".into());
    assert_eq!(
        err(e.app.adopt_detected(req(vec![bad]), &NullSink)).code,
        ErrorCode::InvalidInput
    );
    // Game running.
    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    assert_eq!(
        err(e
            .app
            .adopt_detected(req(vec![item("D", &["ScriptHookV.dll"])]), &NullSink))
        .code,
        ErrorCode::GameRunning
    );
    e.os.0.lock().expect("lock").clear();
    assert_eq!(tx_count(&e), txs, "nothing was adopted");
}

#[test]
fn unknown_files_need_the_confirmation() {
    let e = env_with(rampage_install(Edition::Legacy));
    let d = detect(&e);
    let fast = proposal(&d, "FastTravel");
    let r = e
        .app
        .adopt_detected(
            AdoptDetectedRequest {
                installation_id: e.id.clone(),
                items: vec![as_item(fast)],
                confirm_unknown: false,
            },
            &NullSink,
        )
        .expect("bulk result");
    assert!(r.adopted.is_empty());
    assert_eq!(r.skipped[0].error.code, ErrorCode::InvalidInput);
}

#[test]
fn mods_without_signatures_are_only_grouped_and_unknown_ids_fail() {
    let e = env();
    let d = detect(&e);
    assert!(d.scanned_at.is_some(), "env() scans");
    assert!(
        d.proposals
            .iter()
            .all(|p| p.source_kind == ModSourceKind::Grouped)
    );
    assert_eq!(
        err(e.app.detect_mods("00000000-0000-7000-8000-000000000000")).code,
        ErrorCode::NotFound
    );
}
