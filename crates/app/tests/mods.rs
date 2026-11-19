//! Mod services against a real SQLite file, fake installs, a fake OS and a fake space probe.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use gmm_app::{
    AdoptRequest, App, AppDirs, AppError, AppOptions, ErrorCode, GuardNeeds, ModDto, NullSink,
    OperationKind, ProgressDto, ProgressSink, RegisterInstallationRequest, SpaceProbe,
};
use gmm_domain::{
    Category, Classification, Edition, Installation, ModId, ModState, RelPath, StepKind,
    TransactionId, TransactionKind, TransactionStatus,
};
use gmm_engine::{EngineContext, Vault, execute, plan_adopt, plan_disable};
use gmm_platform::{FaultMode, FaultPlan, FaultyFs, OpMatch, StdFs};
use gmm_scanner::{DiscoveryHints, Hive, OsProbe};
use gmm_store::{Database, TransactionRow};
use gmm_testkit::{FakeInstall, Snapshot};
use tempfile::TempDir;

/// OS probe whose process list can change during a test.
#[derive(Default)]
struct ToggleOs(Mutex<Vec<String>>);

impl OsProbe for ToggleOs {
    fn registry_string(&self, _hive: Hive, _key: &str, _value: &str) -> Option<String> {
        None
    }
    fn file_version(&self, _path: &Path) -> Option<String> {
        None
    }
    fn running_process_names(&self) -> Vec<String> {
        self.0.lock().expect("lock").clone()
    }
}

/// Space probe whose answer can change during a test.
struct FakeSpace(Mutex<Option<u64>>);

impl SpaceProbe for FakeSpace {
    fn free_bytes(&self, _path: &Path) -> Option<u64> {
        *self.0.lock().expect("lock")
    }
}

/// Records every progress event.
#[derive(Default)]
struct Recorder(Mutex<Vec<ProgressDto>>);

impl ProgressSink for Recorder {
    fn emit(&self, p: &ProgressDto) {
        self.0.lock().expect("lock").push(p.clone());
    }
}

struct Env {
    _tmp: TempDir,
    dirs: AppDirs,
    app: Arc<App>,
    os: Arc<ToggleOs>,
    space: Arc<FakeSpace>,
    install: FakeInstall,
    id: String,
}

const TRAINER: [&str; 2] = ["trainer.asi", "trainer.ini"];
const ALPHA: [&str; 2] = ["scripts/alpha.dll", "scripts/alpha.ini"];

fn start(dirs: &AppDirs, os: &Arc<ToggleOs>, space: &Arc<FakeSpace>) -> App {
    App::init_with(
        dirs.clone(),
        AppOptions {
            os: os.clone(),
            hints: DiscoveryHints::default(),
            space: space.clone(),
        },
    )
    .expect("init")
}

fn env() -> Env {
    let tmp = TempDir::new().expect("tempdir");
    let base = dunce::canonicalize(tmp.path()).expect("canonical");
    let dirs = AppDirs::at(&base.join("data"));
    let os = Arc::new(ToggleOs::default());
    let space = Arc::new(FakeSpace(Mutex::new(Some(u64::MAX))));
    let app = start(&dirs, &os, &space);
    let install = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .asi_mod("trainer")
        .scripts_mod("alpha")
        .file("notes/readme.txt", b"hello".to_vec())
        .build()
        .expect("install");
    let dto = app
        .register_installation(RegisterInstallationRequest {
            root_path: install.root().to_string_lossy().into_owned(),
            display_name: None,
            platform: None,
        })
        .expect("register");
    app.scan_installation(&dto.id, &NullSink).expect("scan");
    Env {
        _tmp: tmp,
        dirs,
        app: Arc::new(app),
        os,
        space,
        install,
        id: dto.id,
    }
}

fn err<T>(r: Result<T, AppError>) -> AppError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

fn rp(s: &str) -> RelPath {
    RelPath::parse(s).expect("rel path")
}

fn paths(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_owned()).collect()
}

fn request(e: &Env, files: &[&str], name: &str, category: Category) -> AdoptRequest {
    AdoptRequest {
        installation_id: e.id.clone(),
        rel_paths: paths(files),
        name: name.to_owned(),
        category,
        notes: None,
        confirm_unknown: true,
    }
}

fn adopt(e: &Env, files: &[&str], name: &str) -> ModDto {
    e.app
        .adopt_mod(
            request(e, files, name, Category::ScriptsGameplay),
            &NullSink,
        )
        .expect("adopt")
}

fn class_of(e: &Env, rel: &str) -> Option<Classification> {
    let page = e
        .app
        .list_inventory(
            &e.id,
            gmm_app::InventoryFilterDto {
                classifications: vec![],
                search: Some(rel.to_owned()),
                limit: 100,
                offset: 0,
            },
        )
        .expect("inventory");
    page.items
        .iter()
        .find(|i| i.rel_path.eq_ignore_ascii_case(rel))
        .map(|i| i.classification)
}

fn snapshot(e: &Env) -> Snapshot {
    e.install.snapshot().expect("snapshot")
}

fn state_of(e: &Env, mod_id: &str) -> ModState {
    e.app.get_mod(mod_id).expect("mod").summary.state
}

fn vault_objects(e: &Env) -> usize {
    fn count(dir: &Path) -> usize {
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

#[test]
fn adopt_disable_enable_round_trip() {
    let e = env();
    let preview = e
        .app
        .preview_adopt(&e.id, &paths(&TRAINER))
        .expect("preview");
    assert_eq!(preview.files.len(), 2);
    assert!(preview.blocked.is_empty());
    assert_eq!(preview.suggested_name, "trainer");
    assert!(preview.total_size > 0);
    assert_eq!(class_of(&e, "trainer.asi"), Some(Classification::Candidate));
    let original = e.install.read("trainer.asi").expect("read");

    let rec = Recorder::default();
    let m = e
        .app
        .adopt_mod(
            request(&e, &TRAINER, "Trainer", Category::ScriptsGameplay),
            &rec,
        )
        .expect("adopt");
    assert_eq!(m.state, ModState::Enabled);
    assert_eq!(m.entry_count, 2);
    assert_eq!(m.total_size, preview.total_size);
    assert_eq!(vault_objects(&e), 2);
    for f in TRAINER {
        assert!(e.install.exists(f), "adopt leaves the files in place");
        assert_eq!(class_of(&e, f), Some(Classification::Managed), "{f}");
    }
    let events = rec.0.lock().expect("lock");
    assert!(events.last().expect("events").done);
    assert!(events.iter().all(|p| p.kind == OperationKind::Adopt));
    drop(events);

    let before_disable = snapshot(&e);
    let rec = Recorder::default();
    let m = e.app.set_mod_enabled(&m.id, false, &rec).expect("disable");
    assert_eq!(m.state, ModState::Disabled);
    assert!(
        rec.0
            .lock()
            .expect("lock")
            .iter()
            .all(|p| p.kind == OperationKind::Transaction)
    );
    for f in TRAINER {
        assert!(!e.install.exists(f), "{f} is gone");
        assert_eq!(class_of(&e, f), None, "inventory refreshed");
    }
    let changes = before_disable.diff(&snapshot(&e));
    assert_eq!(
        changes.len(),
        2,
        "only the two mod files changed: {changes:?}"
    );
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);

    let m = e
        .app
        .set_mod_enabled(&m.id, true, &NullSink)
        .expect("enable");
    assert_eq!(m.state, ModState::Enabled);
    assert_eq!(e.install.read("trainer.asi").expect("read"), original);
    assert_eq!(class_of(&e, "trainer.asi"), Some(Classification::Managed));
    assert!(before_disable.diff(&snapshot(&e)).is_empty());

    // Setting the state it already has is a no-op.
    let again = e
        .app
        .set_mod_enabled(&m.id, true, &NullSink)
        .expect("no-op");
    assert_eq!(again.state, ModState::Enabled);
    assert!(before_disable.diff(&snapshot(&e)).is_empty());
}

#[test]
fn list_and_get_mod_report_files_and_sizes() {
    let e = env();
    let m = adopt(&e, &ALPHA, "Alpha");
    let list = e.app.list_mods(&e.id).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0], m);
    assert!(list[0].conflicts_with.is_empty());
    let detail = e.app.get_mod(&m.id).expect("detail");
    assert_eq!(detail.summary, m);
    let rel: Vec<_> = detail.entries.iter().map(|x| x.rel_path.as_str()).collect();
    assert_eq!(rel, ALPHA);
    assert_eq!(
        detail.entries.iter().map(|x| x.size).sum::<u64>(),
        m.total_size
    );
    assert!(detail.entries.iter().all(|x| x.sha256.len() == 64));
    assert_eq!(
        err(e.app.get_mod("0192f0e8-0000-7000-8000-000000000001")).code,
        ErrorCode::NotFound
    );
    assert_eq!(err(e.app.get_mod("nope")).code, ErrorCode::InvalidInput);
}

#[test]
fn game_running_blocks_everything_without_changes() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let before = snapshot(&e);
    *e.os.0.lock().expect("lock") = vec!["gta5.exe".into(), "notepad.exe".into()];

    let status = e.app.guard_status(&e.id).expect("status");
    assert_eq!(status.game_running, ["GTA5.exe"]);
    let blocked = err(e.app.set_mod_enabled(&m.id, false, &NullSink));
    assert_eq!(blocked.code, ErrorCode::GameRunning);
    assert_eq!(blocked.details.as_deref(), Some("GTA5.exe"));
    assert_eq!(
        blocked.hint.as_deref(),
        Some("Close the game and its launchers, then try again.")
    );
    let adopt_err = err(e.app.adopt_mod(
        request(&e, &ALPHA, "Alpha", Category::ScriptsGameplay),
        &NullSink,
    ));
    assert_eq!(adopt_err.code, ErrorCode::GameRunning);
    assert_eq!(
        err(e
            .app
            .set_category_enabled(&e.id, Category::ScriptsGameplay, false, &NullSink))
        .code,
        ErrorCode::GameRunning
    );
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
    assert_eq!(e.app.list_mods(&e.id).expect("list").len(), 1);

    e.os.0.lock().expect("lock").clear();
    assert!(e.app.set_mod_enabled(&m.id, false, &NullSink).is_ok());
}

#[test]
fn insufficient_space_blocks_enable_and_adopt() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    e.app
        .set_mod_enabled(&m.id, false, &NullSink)
        .expect("disable");
    let before = snapshot(&e);

    // Plenty for the files, but not for the 512 MiB margin.
    *e.space.0.lock().expect("lock") = Some(100 * 1024 * 1024);
    let blocked = err(e.app.set_mod_enabled(&m.id, true, &NullSink));
    assert_eq!(blocked.code, ErrorCode::InsufficientSpace);
    assert!(blocked.details.is_some());
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);
    assert!(before.diff(&snapshot(&e)).is_empty());

    let objects = vault_objects(&e);
    let blocked = err(e.app.adopt_mod(
        request(&e, &ALPHA, "Alpha", Category::ScriptsGameplay),
        &NullSink,
    ));
    assert_eq!(blocked.code, ErrorCode::InsufficientSpace);
    assert_eq!(vault_objects(&e), objects);
    assert_eq!(e.app.list_mods(&e.id).expect("list").len(), 1);

    // Disabling needs no space; an unknown amount of space does not block.
    *e.space.0.lock().expect("lock") = None;
    assert!(e.app.set_mod_enabled(&m.id, true, &NullSink).is_ok());
    assert!(e.app.set_mod_enabled(&m.id, false, &NullSink).is_ok());
}

#[test]
fn check_guards_reports_space_directly() {
    let e = env();
    *e.space.0.lock().expect("lock") = Some(600 * 1024 * 1024);
    let ok = GuardNeeds {
        game_folder_writes: false,
        bytes_game: 80 * 1024 * 1024,
        bytes_vault: 0,
    };
    assert!(e.app.check_guards(&e.id, ok).is_ok());
    let too_much = GuardNeeds {
        bytes_game: 100 * 1024 * 1024,
        ..ok
    };
    assert_eq!(
        err(e.app.check_guards(&e.id, too_much)).code,
        ErrorCode::InsufficientSpace
    );
    let vault_too_much = GuardNeeds {
        bytes_game: 0,
        bytes_vault: 100 * 1024 * 1024,
        ..ok
    };
    assert_eq!(
        err(e.app.check_guards(&e.id, vault_too_much)).code,
        ErrorCode::InsufficientSpace
    );
}

#[cfg(unix)]
#[test]
fn not_writable_blocks_enable_and_disable_but_not_adopt() {
    use std::os::unix::fs::PermissionsExt;

    struct Restore<'a>(&'a Path);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
        }
    }

    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let _restore = Restore(e.install.root());
    fs::set_permissions(e.install.root(), fs::Permissions::from_mode(0o555)).expect("chmod");
    let probe = e.install.root().join("probe-check");
    if fs::write(&probe, b"x").is_ok() {
        let _ = fs::remove_file(&probe);
        return; // running with privileges that ignore permissions: nothing to test
    }

    assert!(!e.app.guard_status(&e.id).expect("status").writable);
    assert_eq!(
        err(e.app.set_mod_enabled(&m.id, false, &NullSink)).code,
        ErrorCode::InstallNotWritable
    );
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
    // Adoption only copies into the vault.
    let a = e
        .app
        .adopt_mod(
            request(&e, &ALPHA, "Alpha", Category::ScriptsGameplay),
            &NullSink,
        )
        .expect("adopt works on a read-only game folder");
    assert_eq!(a.state, ModState::Enabled);
}

#[test]
fn unknown_files_need_confirmation() {
    let e = env();
    let preview = e
        .app
        .preview_adopt(&e.id, &paths(&TRAINER))
        .expect("preview");
    assert!(
        preview.includes_unknown,
        "trainer.ini is not a known mod file"
    );
    let mut req = request(&e, &TRAINER, "Trainer", Category::ScriptsGameplay);
    req.confirm_unknown = false;
    let refused = err(e.app.adopt_mod(req, &NullSink));
    assert_eq!(refused.code, ErrorCode::InvalidInput);
    assert!(e.app.list_mods(&e.id).expect("list").is_empty());
    assert_eq!(vault_objects(&e), 0);

    // A selection with only recognized files needs no confirmation.
    let only_known = e
        .app
        .preview_adopt(&e.id, &paths(&["trainer.asi"]))
        .expect("p");
    assert!(!only_known.includes_unknown);
    let mut req = request(&e, &["trainer.asi"], "Only asi", Category::ScriptsGameplay);
    req.confirm_unknown = false;
    assert!(e.app.adopt_mod(req, &NullSink).is_ok());
}

#[test]
fn blocked_files_have_plain_reasons_and_stop_the_adoption() {
    let e = env();
    let m = adopt(&e, &["trainer.asi"], "Trainer");
    let sel = paths(&[
        "GTA5.exe",
        "trainer.asi",
        "scripts/missing.dll",
        "../evil.dll",
        "notes/readme.txt",
        "notes/readme.txt",
    ]);
    let p = e.app.preview_adopt(&e.id, &sel).expect("preview");
    let reason = |path: &str| {
        p.blocked
            .iter()
            .find(|b| b.rel_path == path)
            .map(|b| b.reason.as_str())
    };
    assert_eq!(reason("GTA5.exe"), Some("Part of the game"));
    assert_eq!(reason("trainer.asi"), Some("Already managed"));
    assert_eq!(
        reason("scripts/missing.dll"),
        Some("Not found in the last scan — scan again")
    );
    assert!(reason("../evil.dll").is_some());
    assert_eq!(p.files.len(), 1, "the duplicate selection collapses");
    assert_eq!(p.files[0].rel_path, "notes/readme.txt");
    assert_eq!(p.files[0].classification, Classification::Unknown);
    assert!(p.includes_unknown);
    assert_eq!(p.suggested_category, Category::Uncategorized);

    let before = snapshot(&e);
    let refused = err(e.app.adopt_mod(
        request(
            &e,
            &["notes/readme.txt", "GTA5.exe"],
            "Bad",
            Category::Uncategorized,
        ),
        &NullSink,
    ));
    assert_eq!(refused.code, ErrorCode::InvalidInput);
    assert!(
        refused
            .details
            .as_deref()
            .is_some_and(|d| d.contains("GTA5.exe"))
    );
    assert_eq!(e.app.list_mods(&e.id).expect("list").len(), 1);
    assert!(before.diff(&snapshot(&e)).is_empty());
    let _ = m;
}

#[cfg(unix)]
#[test]
fn links_are_blocked() {
    let e = env();
    fs::hard_link(
        e.install.path("scripts/alpha.dll"),
        e.install.path("scripts/alpha2.dll"),
    )
    .expect("hard link");
    std::os::unix::fs::symlink(
        e.install.path("notes/readme.txt"),
        e.install.path("notes/link.txt"),
    )
    .expect("symlink");
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
    let p = e
        .app
        .preview_adopt(
            &e.id,
            &paths(&["scripts/alpha2.dll", "scripts/alpha.dll", "notes/link.txt"]),
        )
        .expect("preview");
    let reason = |path: &str| {
        p.blocked
            .iter()
            .find(|b| b.rel_path == path)
            .map(|b| b.reason.as_str())
    };
    assert_eq!(reason("scripts/alpha2.dll"), Some("Hard-linked file"));
    assert_eq!(reason("scripts/alpha.dll"), Some("Hard-linked file"));
    // Whether a symlink shows up as a link depends on the walker; it must never be adoptable.
    assert!(p.files.iter().all(|f| f.rel_path != "notes/link.txt"));
}

#[test]
fn suggestions_use_folder_and_category() {
    let e = env();
    let p = e.app.preview_adopt(&e.id, &paths(&ALPHA)).expect("preview");
    assert_eq!(p.suggested_name, "alpha");
    assert_eq!(p.suggested_category, Category::ScriptsGameplay);
    assert_eq!(
        err(e
            .app
            .preview_adopt("0192f0e8-0000-7000-8000-000000000001", &[]))
        .code,
        ErrorCode::NotFound
    );
}

#[test]
fn drift_blocks_and_reconcile_reports_it() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    e.install
        .write("trainer.asi", b"changed by someone else")
        .expect("write");
    let before = snapshot(&e);
    let blocked = err(e.app.set_mod_enabled(&m.id, false, &NullSink));
    assert_eq!(blocked.code, ErrorCode::DriftDetected);
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Drifted);

    let drift = e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].rel_path, "trainer.asi");
    assert_eq!(drift[0].mod_name, "Trainer");
    assert_eq!(
        serde_json::to_value(drift[0].kind).expect("json"),
        serde_json::json!("modified")
    );

    // Back to the original content: the drift resolves by itself.
    e.install
        .write("trainer.asi", &gmm_testkit::fake_bytes("trainer.asi", 256))
        .expect("restore");
    assert!(
        e.app
            .reconcile_installation(&e.id)
            .expect("reconcile")
            .is_empty()
    );
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);

    e.install.remove("trainer.ini").expect("remove");
    let drift = e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(
        serde_json::to_value(drift[0].kind).expect("json"),
        serde_json::json!("missing")
    );
}

#[test]
fn category_switch_previews_and_applies_all_or_nothing() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let other = e
        .app
        .adopt_mod(
            request(&e, &["notes/readme.txt"], "Notes", Category::Vehicles),
            &NullSink,
        )
        .expect("adopt notes");

    let off = e
        .app
        .preview_set_category(&e.id, Category::ScriptsGameplay, false)
        .expect("preview off");
    assert_eq!(off.mods_affected.len(), 2);
    assert!(off.mods_affected.iter().all(|m| {
        m.from == ModState::Enabled && m.to == ModState::Disabled && !m.name.is_empty()
    }));
    assert_eq!(off.actions.len(), 4);
    assert!(
        off.actions
            .iter()
            .all(|a| a.kind == StepKind::TombstoneGameFile)
    );
    assert_eq!(off.bytes_to_copy, 0);
    let noop = e
        .app
        .preview_set_category(&e.id, Category::ScriptsGameplay, true)
        .expect("preview on");
    assert!(noop.mods_affected.is_empty() && noop.actions.is_empty());

    let before = snapshot(&e);
    let rec = Recorder::default();
    let changed = e
        .app
        .set_category_enabled(&e.id, Category::ScriptsGameplay, false, &rec)
        .expect("switch off");
    assert_eq!(changed.len(), 2);
    assert!(changed.iter().all(|m| m.state == ModState::Disabled));
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);
    assert_eq!(
        state_of(&e, &other.id),
        ModState::Enabled,
        "other category untouched"
    );
    assert!(e.install.exists("notes/readme.txt"));
    assert!(!e.install.exists("scripts/alpha.dll"));
    assert!(rec.0.lock().expect("lock").iter().any(|p| p.done));

    let on = e
        .app
        .preview_set_category(&e.id, Category::ScriptsGameplay, true)
        .expect("preview on");
    assert_eq!(on.mods_affected.len(), 2);
    assert!(on.bytes_to_copy > 0);
    assert!(
        on.actions
            .iter()
            .any(|a| a.kind == StepKind::PlaceFromVault)
    );

    // A foreign file blocks one mod: nothing at all is placed.
    e.install
        .write("scripts/alpha.ini", b"foreign")
        .expect("write");
    let mid = snapshot(&e);
    let blocked =
        err(e
            .app
            .set_category_enabled(&e.id, Category::ScriptsGameplay, true, &NullSink));
    assert_eq!(blocked.code, ErrorCode::PathConflict);
    assert!(mid.diff(&snapshot(&e)).is_empty(), "all-or-nothing");
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);
    assert_eq!(state_of(&e, &b.id), ModState::Disabled);
    assert_eq!(
        err(e
            .app
            .preview_set_category(&e.id, Category::ScriptsGameplay, true))
        .code,
        ErrorCode::PathConflict
    );

    e.install.remove("scripts/alpha.ini").expect("remove");
    e.app
        .set_category_enabled(&e.id, Category::ScriptsGameplay, true, &NullSink)
        .expect("switch on");
    assert!(
        before.diff(&snapshot(&e)).is_empty(),
        "byte-identical again"
    );
    assert!(
        e.app
            .set_category_enabled(&e.id, Category::SoundsMusic, true, &NullSink)
            .expect("empty category")
            .is_empty()
    );
}

#[test]
fn category_switch_refuses_drifted_mods() {
    let e = env();
    adopt(&e, &TRAINER, "Trainer");
    adopt(&e, &ALPHA, "Alpha");
    e.install
        .write("scripts/alpha.dll", b"edited")
        .expect("write");
    let before = snapshot(&e);
    let blocked =
        err(e
            .app
            .set_category_enabled(&e.id, Category::ScriptsGameplay, false, &NullSink));
    assert_eq!(blocked.code, ErrorCode::DriftDetected);
    assert!(before.diff(&snapshot(&e)).is_empty());
}

#[test]
fn mods_sharing_a_path_list_each_other_as_conflicts() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer A");
    e.app
        .set_mod_enabled(&a.id, false, &NullSink)
        .expect("disable");
    // The app never adopts a file over a disabled mod's paths twice in one flow; craft the
    // second mod directly through the engine.
    e.install
        .write("trainer.asi", b"another trainer build")
        .expect("write");
    e.install
        .write("trainer.ini", b"another config")
        .expect("write");
    let e = offline(e, |db, inst, dirs| {
        let vault = Vault::new(&dirs.vault_dir);
        let fs = StdFs::new();
        let not_protected = |_: &RelPath| false;
        let ctx = EngineContext {
            fs: &fs,
            db,
            vault: &vault,
            root: &inst.root_path,
            installation_id: inst.id,
            is_protected: &not_protected,
        };
        let files: Vec<RelPath> = TRAINER.iter().map(|f| rp(f)).collect();
        let plan = plan_adopt(
            &ctx,
            ModId::new(),
            "Trainer B",
            Category::ScriptsGameplay,
            None,
            &files,
            true,
        )
        .expect("plan");
        execute(&ctx, plan).expect("adopt B");
    });

    let list = e.app.list_mods(&e.id).expect("list");
    let find = |name: &str| list.iter().find(|m| m.name == name).expect("mod");
    let (a, b) = (find("Trainer A"), find("Trainer B"));
    assert_eq!(a.conflicts_with.len(), 1);
    assert_eq!(a.conflicts_with[0].name, "Trainer B");
    assert_eq!(b.conflicts_with[0].mod_id, a.id);
    assert_eq!(
        e.app
            .get_mod(&a.id)
            .expect("detail")
            .summary
            .conflicts_with
            .len(),
        1
    );

    let blocked = err(e.app.set_mod_enabled(&a.id, true, &NullSink));
    assert_eq!(blocked.code, ErrorCode::PathConflict);
    assert_eq!(state_of(&e, &a.id), ModState::Disabled);
}

#[test]
fn forget_rules() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let off = adopt(&e, &ALPHA, "Alpha");
    e.app
        .set_mod_enabled(&off.id, false, &NullSink)
        .expect("disable");

    let refused = err(e.app.forget_mod(&off.id));
    assert_eq!(refused.code, ErrorCode::InvalidInput);
    assert_eq!(
        refused.message,
        "Enable the mod first so its files are back in the game folder."
    );
    assert_eq!(e.app.list_mods(&e.id).expect("list").len(), 2);

    e.app.forget_mod(&m.id).expect("forget enabled");
    assert_eq!(e.app.list_mods(&e.id).expect("list").len(), 1);
    assert!(
        e.install.exists("trainer.asi"),
        "files stay in the game folder"
    );
    assert_eq!(class_of(&e, "trainer.asi"), Some(Classification::Candidate));
    assert_eq!(err(e.app.forget_mod(&m.id)).code, ErrorCode::NotFound);

    // A drifted mod can be forgotten too.
    e.app
        .set_mod_enabled(&off.id, true, &NullSink)
        .expect("enable");
    e.install
        .write("scripts/alpha.dll", b"edited")
        .expect("write");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &off.id), ModState::Drifted);
    e.app.forget_mod(&off.id).expect("forget drifted");
    assert!(e.app.list_mods(&e.id).expect("list").is_empty());
}

#[test]
fn update_mod_validates_and_reads_back() {
    let e = env();
    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    let updated = e
        .app
        .update_mod(
            &a.id,
            "  Trainer v2 ",
            Category::Vehicles,
            Some("  my notes ".into()),
        )
        .expect("update");
    assert_eq!(updated.name, "Trainer v2");
    assert_eq!(updated.category, Category::Vehicles);
    assert_eq!(updated.notes.as_deref(), Some("my notes"));
    assert_eq!(updated.state, ModState::Enabled);
    let cleared = e
        .app
        .update_mod(&a.id, "Trainer v2", Category::Vehicles, Some("   ".into()))
        .expect("update");
    assert_eq!(cleared.notes, None);

    assert_eq!(
        err(e.app.update_mod(&a.id, "Alpha", Category::Vehicles, None)).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e.app.update_mod(&a.id, "   ", Category::Vehicles, None)).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e.app.update_mod(
            "0192f0e8-0000-7000-8000-000000000001",
            "x",
            Category::Vehicles,
            None
        ))
        .code,
        ErrorCode::NotFound
    );
    // Duplicate names are refused at adoption too.
    let dup = err(e.app.adopt_mod(
        request(&e, &["notes/readme.txt"], "Alpha", Category::Uncategorized),
        &NullSink,
    ));
    assert_eq!(dup.code, ErrorCode::InvalidInput);
    let _ = b;
}

/// Closes the app, runs `f` against the database directly and starts a new app.
fn offline(e: Env, f: impl FnOnce(&Database, &Installation, &AppDirs)) -> Env {
    let Env {
        _tmp,
        dirs,
        app,
        os,
        space,
        install,
        id,
    } = e;
    drop(app);
    let db = Database::open(&dirs.db_path).expect("db");
    let inst = db
        .list_installations()
        .expect("installations")
        .into_iter()
        .next()
        .expect("installation");
    f(&db, &inst, &dirs);
    drop(db);
    Env {
        _tmp,
        app: Arc::new(start(&dirs, &os, &space)),
        dirs,
        os,
        space,
        install,
        id,
    }
}

/// Runs `disable` with a simulated crash at the `nth` mutating operation while the app is
/// closed, then starts a new app. Returns the journal status the crash left behind.
fn crash_then_restart(e: Env, mod_id: &str, nth: usize) -> (Env, TransactionStatus) {
    let mut status = None;
    let e = offline(e, |db, inst, dirs| {
        let vault = Vault::new(&dirs.vault_dir);
        let fs = FaultyFs::with_plan(
            StdFs::new(),
            FaultPlan {
                op: OpMatch::Any,
                nth,
                mode: FaultMode::Crash,
            },
        );
        let not_protected = |_: &RelPath| false;
        let ctx = EngineContext {
            fs: &fs,
            db,
            vault: &vault,
            root: &inst.root_path,
            installation_id: inst.id,
            is_protected: &not_protected,
        };
        let plan = plan_disable(&ctx, &[mod_id.parse().expect("mod id")]).expect("plan");
        assert!(execute(&ctx, plan).expect_err("crash").is_crash());
        let left = db.transactions_needing_recovery().expect("pending");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].kind, TransactionKind::Disable);
        status = Some(left[0].status);
    });
    (e, status.expect("status"))
}

#[test]
fn startup_rolls_back_an_interrupted_open_transaction() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let before = snapshot(&e);
    let (e, status) = crash_then_restart(e, &m.id, 2);
    assert_eq!(status, TransactionStatus::Open);
    let s = e.app.status().expect("status");
    assert_eq!((s.recovery.rolled_back, s.recovery.committed), (1, 0));
    assert_eq!((s.recovery.needs_attention, s.recovery.skipped), (0, 0));
    assert!(
        before.diff(&snapshot(&e)).is_empty(),
        "the disk is back to the pre-state"
    );
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
    assert!(!e.app.guard_status(&e.id).expect("guards").recovery_required);
    assert!(e.app.set_mod_enabled(&m.id, false, &NullSink).is_ok());
    let json = serde_json::to_value(&s).expect("json");
    assert_eq!(json["recovery"]["rolledBack"], 1);
}

#[test]
fn startup_rolls_forward_a_committing_transaction() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let (e, status) = crash_then_restart(e, &m.id, 3);
    assert_eq!(status, TransactionStatus::Committing);
    let s = e.app.status().expect("status");
    assert_eq!((s.recovery.rolled_back, s.recovery.committed), (0, 1));
    assert_eq!(state_of(&e, &m.id), ModState::Disabled);
    assert!(!e.install.exists("trainer.asi") && !e.install.exists("trainer.ini"));
    assert!(e.app.set_mod_enabled(&m.id, true, &NullSink).is_ok());
}

#[test]
fn unresolved_transaction_blocks_with_recovery_required() {
    let e = env();
    let m = adopt(&e, &TRAINER, "Trainer");
    let e = offline(e, |db, inst, _| {
        let tx = TransactionId::new();
        let now = Utc::now();
        db.create_transaction(
            &TransactionRow {
                id: tx,
                installation_id: inst.id,
                kind: TransactionKind::Disable,
                status: TransactionStatus::Open,
                summary_json: "{}".into(),
                error: None,
                created_at: now,
                finished_at: None,
            },
            &[],
            now,
        )
        .expect("create");
        db.set_transaction_status(tx, TransactionStatus::RollingBack, None, now)
            .expect("rolling back");
        db.set_transaction_status(tx, TransactionStatus::NeedsAttention, Some("test"), now)
            .expect("needs attention");
    });

    assert!(e.app.guard_status(&e.id).expect("guards").recovery_required);
    let before = snapshot(&e);
    for result in [
        e.app.set_mod_enabled(&m.id, false, &NullSink).map(|_| ()),
        e.app
            .adopt_mod(
                request(&e, &ALPHA, "Alpha", Category::ScriptsGameplay),
                &NullSink,
            )
            .map(|_| ()),
    ] {
        assert_eq!(err(result).code, ErrorCode::RecoveryRequired);
    }
    assert_eq!(
        err(e.app.check_guards(&e.id, GuardNeeds::default())).code,
        ErrorCode::RecoveryRequired
    );
    assert!(before.diff(&snapshot(&e)).is_empty());
    assert_eq!(state_of(&e, &m.id), ModState::Enabled);
}
