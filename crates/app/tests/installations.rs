//! Installation use cases against a real SQLite file, fake installs and a fake OS.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gmm_app::{App, AppDirs, AppOptions, ErrorCode, RegisterInstallationRequest};
use gmm_domain::{DistributionPlatform, Edition, InstallationStatus};
use gmm_scanner::DiscoveryHints;
use gmm_scanner::testing::FakeOs;
use gmm_testkit::{FakeInstall, base_files, fake_bytes};
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    app: App,
    base: PathBuf,
}

fn env_with(hints: impl FnOnce(&Path) -> DiscoveryHints) -> Env {
    let tmp = TempDir::new().expect("tempdir");
    let base = dunce::canonicalize(tmp.path()).expect("canonical");
    let app = App::init_with(
        AppDirs::at(&base.join("data")),
        AppOptions {
            os: Arc::new(FakeOs::new()),
            hints: hints(&base),
            ..AppOptions::default()
        },
    )
    .expect("init");
    Env {
        _tmp: tmp,
        app,
        base,
    }
}

fn env() -> Env {
    env_with(|_| DiscoveryHints::default())
}

fn legacy() -> FakeInstall {
    FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .build()
        .expect("install")
}

fn enhanced() -> FakeInstall {
    FakeInstall::builder(Edition::Enhanced)
        .with_base_files()
        .build()
        .expect("install")
}

fn req(path: &Path) -> RegisterInstallationRequest {
    RegisterInstallationRequest {
        root_path: path.to_string_lossy().into_owned(),
        display_name: None,
        platform: None,
    }
}

/// Writes the base files of `edition` into `dir`.
fn populate(dir: &Path, edition: Edition) {
    for rel in base_files(edition) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(p, fake_bytes(rel, 64)).expect("write");
    }
}

/// Builds a fake Steam client with one library holding a Legacy install.
fn steam_env() -> (Env, PathBuf) {
    let tmp_root = std::cell::RefCell::new(PathBuf::new());
    let e = env_with(|base| {
        let steam = base.join("Steam");
        let game = steam.join("steamapps/common/Grand Theft Auto V");
        populate(&game, Edition::Legacy);
        fs::write(
            steam.join("steamapps/appmanifest_271590.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\"271590\"\n\t\"installdir\"\t\"Grand Theft Auto V\"\n}\n",
        )
        .expect("manifest");
        let p = steam.to_string_lossy().replace('\\', "\\\\");
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\"{p}\"\n\t}}\n}}\n"),
        )
        .expect("vdf");
        *tmp_root.borrow_mut() = game;
        DiscoveryHints {
            steam_roots: vec![steam],
            epic_manifest_dirs: vec![],
        }
    });
    let game = tmp_root.into_inner();
    (e, game)
}

#[test]
fn discovery_marks_already_registered_after_registering() {
    let (e, game) = steam_env();
    let found = e.app.discover_installations().expect("discover");
    assert_eq!(found.len(), 1);
    let d = &found[0];
    assert_eq!(d.platform, DistributionPlatform::Steam);
    assert_eq!(d.edition, Some(Edition::Legacy));
    assert_eq!(d.status, InstallationStatus::Ok);
    assert!(d.registrable && d.writable && !d.already_registered);
    assert!(d.problems.is_empty() && d.missing_files.is_empty());

    e.app
        .register_installation(req(Path::new(&d.root_path)))
        .expect("register");
    let found = e.app.discover_installations().expect("discover");
    assert!(found[0].already_registered);
    assert!(found[0].registrable);
    assert!(game.join("GTA5.exe").exists());
}

#[test]
fn discovery_reports_incomplete_candidates_with_problems() {
    let (e, game) = steam_env();
    fs::remove_file(game.join("common.rpf")).expect("remove");
    let found = e.app.discover_installations().expect("discover");
    let d = &found[0];
    assert_eq!(d.status, InstallationStatus::Incomplete);
    assert!(!d.registrable);
    assert_eq!(d.missing_files, vec!["common.rpf".to_owned()]);
    assert_eq!(d.problems, vec!["Some required game files are missing."]);
}

#[test]
fn registers_legacy_and_enhanced_with_default_names() {
    let e = env();
    let l = legacy();
    let n = enhanced();
    let a = e.app.register_installation(req(l.root())).expect("legacy");
    let b = e
        .app
        .register_installation(req(n.root()))
        .expect("enhanced");
    assert_eq!(a.edition, Edition::Legacy);
    assert_eq!(b.edition, Edition::Enhanced);
    assert_eq!(a.display_name, "GTA V Legacy (Unknown)");
    assert_eq!(b.display_name, "GTA V Enhanced (Unknown)");
    assert_eq!(a.status, InstallationStatus::Ok);
    assert_eq!(a.baseline, gmm_domain::BaselineTrust::None);
    assert_eq!(e.app.list_installations().expect("list").len(), 2);
}

#[test]
fn platform_is_inferred_or_taken_from_request() {
    let e = env();
    let tmp = TempDir::new().expect("tmp");
    let steam_dir = tmp.path().join("steamapps/common/GTA V");
    populate(&steam_dir, Edition::Legacy);
    let a = e.app.register_installation(req(&steam_dir)).expect("steam");
    assert_eq!(a.platform, DistributionPlatform::Steam);
    assert_eq!(a.display_name, "GTA V Legacy (Steam)");

    let epic_dir = tmp.path().join("Epic Games/GTAV");
    populate(&epic_dir, Edition::Enhanced);
    let b = e.app.register_installation(req(&epic_dir)).expect("epic");
    assert_eq!(b.platform, DistributionPlatform::Epic);
    assert_eq!(b.display_name, "GTA V Enhanced (Epic)");

    let other = legacy();
    let mut r = req(other.root());
    r.platform = Some(DistributionPlatform::Rockstar);
    r.display_name = Some("  My game  ".into());
    let c = e.app.register_installation(r).expect("rockstar");
    assert_eq!(c.platform, DistributionPlatform::Rockstar);
    assert_eq!(c.display_name, "My game");
}

#[test]
fn refuses_missing_incomplete_and_ambiguous_paths() {
    let e = env();
    let err = e
        .app
        .register_installation(req(&e.base.join("nope")))
        .expect_err("missing");
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.message, "The folder does not exist.");

    let inc = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .without("common.rpf")
        .build()
        .expect("install");
    let err = e
        .app
        .register_installation(req(inc.root()))
        .expect_err("incomplete");
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(
        err.details
            .as_deref()
            .is_some_and(|d| d.contains("required game files"))
    );

    let amb = legacy();
    fs::write(amb.root().join("GTA5_Enhanced.exe"), b"x").expect("write");
    let err = e
        .app
        .register_installation(req(amb.root()))
        .expect_err("ambiguous");
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(
        err.details
            .as_deref()
            .is_some_and(|d| d.contains("ambiguous"))
    );

    let empty = TempDir::new().expect("tmp");
    let err = e
        .app
        .register_installation(req(empty.path()))
        .expect_err("empty");
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(e.app.list_installations().expect("list").is_empty());
}

#[test]
fn duplicate_registration_is_refused() {
    let e = env();
    let l = legacy();
    e.app.register_installation(req(l.root())).expect("first");
    let err = e.app.register_installation(req(l.root())).expect_err("dup");
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.message, "This installation is already registered.");
    assert_eq!(e.app.list_installations().expect("list").len(), 1);
}

#[cfg(unix)]
#[test]
fn root_symlink_is_resolved_and_duplicates_are_detected() {
    let e = env();
    let l = legacy();
    let link = e.base.join("link");
    std::os::unix::fs::symlink(l.root(), &link).expect("symlink");
    let dto = e.app.register_installation(req(&link)).expect("via link");
    let canonical = dunce::canonicalize(l.root()).expect("canonical");
    assert_eq!(Path::new(&dto.root_path), canonical);
    let err = e.app.register_installation(req(l.root())).expect_err("dup");
    assert_eq!(err.message, "This installation is already registered.");
}

#[test]
fn first_installation_is_selected_second_is_not() {
    let e = env();
    assert!(e.app.get_selected_installation().expect("sel").is_none());
    let l = legacy();
    let n = enhanced();
    let a = e.app.register_installation(req(l.root())).expect("a");
    let selected = e.app.get_selected_installation().expect("sel");
    assert_eq!(selected.map(|s| s.id), Some(a.id.clone()));
    let b = e.app.register_installation(req(n.root())).expect("b");
    let selected = e.app.get_selected_installation().expect("sel");
    assert_eq!(selected.map(|s| s.id), Some(a.id));
    e.app.select_installation(&b.id).expect("select");
    let selected = e.app.get_selected_installation().expect("sel");
    assert_eq!(selected.map(|s| s.id), Some(b.id));
}

#[test]
fn select_rejects_unknown_and_malformed_ids() {
    let e = env();
    let err = e
        .app
        .select_installation("0192f0e8-0000-7000-8000-000000000000")
        .expect_err("unknown");
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = e.app.select_installation("nonsense").expect_err("bad");
    assert_eq!(err.code, ErrorCode::InvalidInput);
}

#[test]
fn stale_selected_id_is_cleared() {
    let e = env();
    let l = legacy();
    e.app.register_installation(req(l.root())).expect("a");
    // A second connection writes an id that does not exist.
    let other = gmm_store::Database::open(&e.base.join("data").join("gmm.sqlite3")).expect("open");
    other
        .set_setting(
            gmm_store::SETTING_SELECTED_INSTALLATION,
            "0192f0e8-0000-7000-8000-000000000000",
        )
        .expect("set");
    assert!(e.app.get_selected_installation().expect("sel").is_none());
    assert_eq!(
        other
            .get_setting(gmm_store::SETTING_SELECTED_INSTALLATION)
            .expect("get"),
        None
    );
    // A garbled value is also cleared.
    other
        .set_setting(gmm_store::SETTING_SELECTED_INSTALLATION, "garbage")
        .expect("set");
    assert!(e.app.get_selected_installation().expect("sel").is_none());
}

#[test]
fn refresh_detects_deleted_required_file_and_deleted_root() {
    let e = env();
    let l = legacy();
    let dto = e.app.register_installation(req(l.root())).expect("reg");
    let same = e.app.refresh_installation(&dto.id).expect("refresh");
    assert_eq!(same.status, InstallationStatus::Ok);

    l.remove("common.rpf").expect("remove");
    let r = e.app.refresh_installation(&dto.id).expect("refresh");
    assert_eq!(r.status, InstallationStatus::Incomplete);
    assert_eq!(r.id, dto.id);
    // Persisted.
    let listed = e.app.list_installations().expect("list");
    assert_eq!(listed[0].status, InstallationStatus::Incomplete);

    fs::write(l.root().join("common.rpf"), b"x").expect("restore");
    let r = e.app.refresh_installation(&dto.id).expect("refresh");
    assert_eq!(r.status, InstallationStatus::Ok);

    // Delete the whole root: remove every entry, then the directory itself.
    let root = l.root().to_path_buf();
    fs::remove_dir_all(&root).expect("rm root");
    let r = e.app.refresh_installation(&dto.id).expect("refresh");
    assert_eq!(r.status, InstallationStatus::Missing);
    assert_eq!(r.exe_version, None);
    // Dropping the FakeInstall of a missing dir must not matter for the assertions above.
}

#[test]
fn refresh_unknown_id_is_not_found() {
    let e = env();
    let err = e
        .app
        .refresh_installation("0192f0e8-0000-7000-8000-000000000000")
        .expect_err("nf");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn rename_validates_and_trims() {
    let e = env();
    let l = legacy();
    let dto = e.app.register_installation(req(l.root())).expect("reg");
    let r = e
        .app
        .rename_installation(&dto.id, "  Main game  ")
        .expect("rename");
    assert_eq!(r.display_name, "Main game");
    assert_eq!(
        e.app.list_installations().expect("list")[0].display_name,
        "Main game"
    );
    for bad in ["", "   ", &"x".repeat(81)] {
        let err = e.app.rename_installation(&dto.id, bad).expect_err("bad");
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }
    assert!(e.app.rename_installation(&dto.id, &"x".repeat(80)).is_ok());
    let err = e
        .app
        .rename_installation("0192f0e8-0000-7000-8000-000000000000", "A")
        .expect_err("nf");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn dto_json_shape_is_camel_case_with_enum_strings() {
    let e = env();
    let l = legacy();
    let dto = e.app.register_installation(req(l.root())).expect("reg");
    let v = serde_json::to_value(&dto).expect("json");
    let obj = v.as_object().expect("object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "baseline",
            "createdAt",
            "displayName",
            "edition",
            "exeVersion",
            "game",
            "id",
            "platform",
            "rootPath",
            "status",
            "updatedAt"
        ]
    );
    assert_eq!(v["edition"], "legacy");
    assert_eq!(v["game"], "gtav");
    assert_eq!(v["platform"], "unknown");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["baseline"], "none");
    assert!(v["exeVersion"].is_null());
    assert!(v["createdAt"].as_str().is_some_and(|t| t.ends_with('Z')));

    let (e2, _) = steam_env();
    let found = e2.app.discover_installations().expect("discover");
    let v = serde_json::to_value(&found[0]).expect("json");
    for k in [
        "rootPath",
        "platform",
        "source",
        "edition",
        "status",
        "missingFiles",
        "exeVersion",
        "writable",
        "alreadyRegistered",
        "registrable",
        "problems",
    ] {
        assert!(v.get(k).is_some(), "missing key {k}");
    }
    assert_eq!(v["source"], "steam:libraryfolders");

    let r: RegisterInstallationRequest = serde_json::from_value(serde_json::json!({
        "rootPath": "/x", "displayName": null, "platform": "epic"
    }))
    .expect("request");
    assert_eq!(r.platform, Some(DistributionPlatform::Epic));
}
