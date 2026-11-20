//! Phase 2 services: ZIP import, dependencies, vault report and garbage collection,
//! and file-version metadata of entries.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use common::*;
use gmm_app::{
    AdoptRequest, App, AppOptions, ConfirmImportRequest, ErrorCode, ImportPreviewDto,
    ImportTargetClass, NullSink, RegisterInstallationRequest,
};
use gmm_domain::{Category, Edition, ModState, TransactionId, TransactionKind, TransactionStatus};
use gmm_scanner::{DiscoveryHints, Hive, OsProbe};
use gmm_store::{Database, TransactionRow};
use gmm_testkit::FakeInstall;
use tempfile::TempDir;

fn make_zip(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let path = dir.join(name);
    let mut zw = zip::ZipWriter::new(fs::File::create(&path).expect("create zip"));
    for (n, bytes) in files {
        zw.start_file(*n, zip::write::SimpleFileOptions::default())
            .expect("start file");
        zw.write_all(bytes).expect("write entry");
    }
    zw.finish().expect("finish zip");
    path
}

fn zip_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A mod with a wrapper folder: the suggested root is `NewMod`.
fn newmod_zip(e: &Env) -> PathBuf {
    make_zip(
        e._tmp.path(),
        "New Mod v1.zip",
        &[
            ("NewMod/scripts/newmod.asi", b"asi-bytes"),
            ("NewMod/scripts/newmod.ini", b"ini-bytes"),
            ("NewMod/readme.txt", b"hi"),
        ],
    )
}

fn stage(e: &Env, zip: &Path) -> ImportPreviewDto {
    e.app
        .stage_import(&e.id, &zip_str(zip), &NullSink)
        .expect("stage")
}

fn confirm_req(p: &ImportPreviewDto, name: &str, enable_now: bool) -> ConfirmImportRequest {
    ConfirmImportRequest {
        staging_id: p.staging_id.clone(),
        root: p.suggested_root.clone(),
        name: name.to_owned(),
        category: Category::ScriptsGameplay,
        notes: Some("  from a zip  ".into()),
        enable_now,
    }
}

fn staging_entries(e: &Env) -> usize {
    match fs::read_dir(e.dirs.data_dir.join("staging")) {
        Ok(rd) => rd.count(),
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------- import

#[test]
fn import_happy_path_then_enable_places_files() {
    let e = env();
    let before = snapshot(&e);
    let p = stage(&e, &newmod_zip(&e));
    assert_eq!(p.archive_name, "New Mod v1.zip");
    assert_eq!(p.suggested_name, "New Mod v1");
    assert_eq!(p.suggested_root.as_deref(), Some("NewMod"));
    assert_eq!(p.root.as_deref(), Some("NewMod"));
    assert_eq!(p.files.len(), 3);
    let targets: Vec<&str> = p.mapped.iter().map(|m| m.target_path.as_str()).collect();
    assert_eq!(
        targets,
        ["readme.txt", "scripts/newmod.asi", "scripts/newmod.ini"]
    );
    assert!(p.mapped.iter().all(|m| !m.conflict && !m.protected));
    assert!(
        p.mapped
            .iter()
            .all(|m| m.classification == ImportTargetClass::Absent)
    );
    assert_eq!(p.suggested_category, Category::ScriptsGameplay);
    assert_eq!(staging_entries(&e), 1);

    let m = e
        .app
        .confirm_import(confirm_req(&p, "New Mod", false), &NullSink)
        .expect("confirm");
    assert_eq!(m.state, ModState::Disabled);
    assert_eq!(m.entry_count, 3);
    assert_eq!(m.notes.as_deref(), Some("from a zip"));
    // Provenance: the archive name and hash.
    let zip_sha =
        gmm_platform::sha256_file(&gmm_platform::StdFs::new(), &newmod_zip(&e)).expect("zip hash");
    assert_eq!(m.source.kind, gmm_domain::ModSourceKind::ZipImport);
    assert_eq!(m.source.detail.as_deref(), Some("New Mod v1.zip"));
    assert_eq!(m.source.sha256.as_deref(), Some(zip_sha.as_str()));
    assert_eq!(m.source.version, None);
    assert_eq!(staging_entries(&e), 0, "staging deleted after import");
    assert_eq!(snapshot(&e).diff(&before), vec![], "game folder untouched");
    let history = e.app.list_transactions(&e.id, 1, 0).expect("history");
    assert_eq!(history.items[0].description, "Adopted mod “New Mod”");

    // A second confirm of the same staging id is refused.
    assert_eq!(
        err(e
            .app
            .confirm_import(confirm_req(&p, "Other", false), &NullSink))
        .code,
        ErrorCode::NotFound
    );

    let enabled = set(&e, &m.id, true);
    assert_eq!(enabled.state, ModState::Enabled);
    assert_eq!(
        fs::read(e.install.root().join("scripts/newmod.asi")).expect("placed"),
        b"asi-bytes"
    );
    assert!(e.install.root().join("readme.txt").is_file());
}

#[test]
fn import_with_enable_now_enables_the_mod() {
    let e = env();
    let p = stage(&e, &newmod_zip(&e));
    let m = e
        .app
        .confirm_import(confirm_req(&p, "New Mod", true), &NullSink)
        .expect("confirm");
    assert_eq!(m.state, ModState::Enabled);
    assert!(e.install.root().join("scripts/newmod.ini").is_file());
}

#[test]
fn remap_recomputes_the_mapping() {
    let e = env();
    let p = stage(&e, &newmod_zip(&e));
    let r = e
        .app
        .remap_import(&p.staging_id, Some("NewMod/scripts"))
        .expect("remap");
    assert_eq!(r.staging_id, p.staging_id);
    assert_eq!(r.suggested_root.as_deref(), Some("NewMod"));
    assert_eq!(r.root.as_deref(), Some("NewMod/scripts"));
    let targets: Vec<&str> = r.mapped.iter().map(|m| m.target_path.as_str()).collect();
    assert_eq!(targets, ["newmod.asi", "newmod.ini"]);

    let r = e.app.remap_import(&p.staging_id, None).expect("remap root");
    assert_eq!(r.mapped.len(), 3);
    assert_eq!(r.mapped[0].target_path, "NewMod/readme.txt");

    let r = e
        .app
        .remap_import(&p.staging_id, Some("Elsewhere"))
        .expect("remap empty");
    assert!(r.mapped.is_empty());
    let mut req = confirm_req(&p, "X", false);
    req.root = Some("Elsewhere".into());
    assert_eq!(
        err(e.app.confirm_import(req, &NullSink)).code,
        ErrorCode::InvalidInput
    );

    assert_eq!(
        err(e.app.remap_import(&p.staging_id, Some("../up"))).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e.app.remap_import("nope", None)).code,
        ErrorCode::NotFound
    );
}

#[test]
fn protected_targets_are_refused() {
    let e = env();
    let zip = make_zip(
        e._tmp.path(),
        "evil.zip",
        &[("Pack/GTA5.exe", b"nope"), ("Pack/ok.asi", b"ok")],
    );
    let p = stage(&e, &zip);
    let exe = p
        .mapped
        .iter()
        .find(|m| m.target_path == "GTA5.exe")
        .expect("exe mapped");
    assert!(exe.protected && exe.conflict);
    assert_eq!(exe.classification, ImportTargetClass::Original);
    let before = snapshot(&e);
    let e1 = err(e
        .app
        .confirm_import(confirm_req(&p, "Evil", false), &NullSink));
    assert_eq!(e1.code, ErrorCode::OwnershipUncertain);
    assert!(e1.details.expect("details").contains("GTA5.exe"));
    assert_eq!(snapshot(&e).diff(&before), vec![]);
    assert!(e.app.list_mods(&e.id).expect("mods").is_empty());
    // The staged archive is still there so the user can pick another root.
    assert!(e.app.remap_import(&p.staging_id, Some("Pack/ok")).is_ok());
}

#[test]
fn conflict_on_enable_now_leaves_the_mod_disabled() {
    let e = env();
    // `trainer.asi` already sits in the game folder as an unmanaged file.
    let zip = make_zip(
        e._tmp.path(),
        "clash.zip",
        &[("trainer.asi", b"different"), ("trainer_extra.ini", b"x")],
    );
    let p = stage(&e, &zip);
    let clash = p
        .mapped
        .iter()
        .find(|m| m.target_path == "trainer.asi")
        .expect("mapped");
    assert!(clash.conflict && !clash.protected);
    assert_eq!(clash.classification, ImportTargetClass::Candidate);

    let before = snapshot(&e);
    let failure = err(e
        .app
        .confirm_import(confirm_req(&p, "Clash", true), &NullSink));
    assert_eq!(failure.code, ErrorCode::PathConflict);
    assert_eq!(
        failure.hint.as_deref(),
        Some("The mod was imported but not enabled.")
    );
    assert_eq!(snapshot(&e).diff(&before), vec![], "game folder untouched");
    let mods = e.app.list_mods(&e.id).expect("mods");
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].state, ModState::Disabled);
    assert_eq!(staging_entries(&e), 0);
}

#[test]
fn duplicate_name_is_refused_and_staging_is_kept() {
    let e = env();
    adopt(&e, &TRAINER, "Taken");
    let p = stage(&e, &newmod_zip(&e));
    assert_eq!(
        err(e
            .app
            .confirm_import(confirm_req(&p, "Taken", false), &NullSink))
        .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e
            .app
            .confirm_import(confirm_req(&p, "  ", false), &NullSink))
        .code,
        ErrorCode::InvalidInput
    );
    assert!(e.app.remap_import(&p.staging_id, None).is_ok());
}

#[test]
fn discard_and_restage_delete_staging_folders() {
    let e = env();
    let zip = newmod_zip(&e);
    let first = stage(&e, &zip);
    assert_eq!(staging_entries(&e), 1);
    let second = stage(&e, &zip);
    assert_ne!(first.staging_id, second.staging_id);
    assert_eq!(staging_entries(&e), 1, "the previous staging was removed");
    assert_eq!(
        err(e.app.remap_import(&first.staging_id, None)).code,
        ErrorCode::NotFound
    );
    // Discarding an unknown id changes nothing.
    e.app.discard_import(&first.staging_id).expect("noop");
    assert_eq!(staging_entries(&e), 1);
    e.app.discard_import(&second.staging_id).expect("discard");
    assert_eq!(staging_entries(&e), 0);
    assert_eq!(
        err(e.app.remap_import(&second.staging_id, None)).code,
        ErrorCode::NotFound
    );
}

#[test]
fn bad_archives_are_rejected_without_leftovers() {
    let e = env();
    let not_zip = e._tmp.path().join("not.zip");
    fs::write(&not_zip, b"this is not a zip").expect("write");
    let f = err(e.app.stage_import(&e.id, &zip_str(&not_zip), &NullSink));
    assert_eq!(f.code, ErrorCode::InvalidInput);
    let empty = make_zip(e._tmp.path(), "empty.zip", &[]);
    assert_eq!(
        err(e.app.stage_import(&e.id, &zip_str(&empty), &NullSink)).code,
        ErrorCode::InvalidInput
    );
    let missing = e._tmp.path().join("missing.zip");
    assert_eq!(
        err(e.app.stage_import(&e.id, &zip_str(&missing), &NullSink)).code,
        ErrorCode::Io
    );
    assert_eq!(staging_entries(&e), 0);
}

/// Cancels the running operation as soon as the first event arrives.
struct CancelSink<'a>(&'a App);

impl gmm_app::ProgressSink for CancelSink<'_> {
    fn emit(&self, _p: &gmm_app::ProgressDto) {
        self.0.cancel_operation();
    }
}

#[test]
fn staging_can_be_cancelled() {
    let e = env();
    let zip = newmod_zip(&e);
    let f = err(e
        .app
        .stage_import(&e.id, &zip_str(&zip), &CancelSink(&e.app)));
    assert_eq!(f.code, ErrorCode::Cancelled);
    assert_eq!(staging_entries(&e), 0);
    // The slot is free again.
    assert!(e.app.stage_import(&e.id, &zip_str(&zip), &NullSink).is_ok());
}

#[test]
fn leftover_staging_folders_are_deleted_on_start() {
    let tmp = TempDir::new().expect("tempdir");
    let dirs = gmm_app::AppDirs::at(&tmp.path().join("data"));
    let leftover = dirs.data_dir.join("staging").join("stage-1-1");
    fs::create_dir_all(leftover.join("sub")).expect("mkdir");
    fs::write(leftover.join("sub/f.bin"), b"x").expect("write");
    let keep = dirs.data_dir.join("staging").join("not-ours.txt");
    fs::write(&keep, b"keep").expect("write");
    let app = App::init_with(
        dirs.clone(),
        AppOptions {
            os: Arc::new(ToggleOs::default()),
            hints: DiscoveryHints::default(),
            space: Arc::new(FakeSpace),
        },
    )
    .expect("init");
    assert!(!leftover.exists());
    assert!(
        keep.exists(),
        "only the app's own staging folders are removed"
    );
    drop(app);
}

// ---------------------------------------------------------- dependencies

/// Two mods, both disabled: `a` (trainer) and `b` (beta).
fn two_disabled(e: &Env) -> (String, String) {
    let a = adopt(e, &TRAINER, "A");
    let b = adopt(e, &BETA, "B");
    set(e, &a.id, false);
    set(e, &b.id, false);
    (a.id, b.id)
}

#[test]
fn dependency_blocks_without_closure_and_applies_with_it() {
    let e = env();
    let (a, b) = two_disabled(&e);
    let detail = e.app.add_dependency(&a, &b).expect("add");
    assert_eq!(detail.summary.depends_on_count, 1);
    assert_eq!(detail.depends_on.len(), 1);
    assert_eq!(detail.depends_on[0].mod_id, b);
    assert_eq!(detail.depends_on[0].name, "B");
    let b_detail = e.app.get_mod(&b).expect("b");
    assert_eq!(b_detail.required_by[0].name, "A");
    assert_eq!(b_detail.summary.depends_on_count, 0);
    let listed = e.app.list_mods(&e.id).expect("list");
    assert_eq!(
        listed
            .iter()
            .find(|m| m.id == a)
            .expect("a")
            .depends_on_count,
        1
    );

    let before = snapshot(&e);
    let f = err(e.app.set_mod_enabled(&a, true, &NullSink));
    assert_eq!(f.code, ErrorCode::DependencyBlocked);
    assert!(
        f.message.contains("\"A\"") && f.message.contains("\"B\""),
        "{}",
        f.message
    );
    assert_eq!(snapshot(&e).diff(&before), vec![]);

    let preview = e.app.preview_set_mod_enabled(&a, true).expect("preview");
    let mut affected: Vec<&str> = preview
        .mods_affected
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    affected.sort_unstable();
    assert_eq!(affected, ["A", "B"]);
    assert!(preview.actions.len() >= 4);
    assert_eq!(snapshot(&e).diff(&before), vec![], "preview writes nothing");

    let n = tx_count(&e);
    let m = e
        .app
        .set_mod_enabled_with(&a, true, true, &NullSink)
        .expect("with deps");
    assert_eq!(m.state, ModState::Enabled);
    assert_eq!(state_of(&e, &b), ModState::Enabled);
    assert_eq!(tx_count(&e), n + 1, "one transaction for the closure");
}

#[test]
fn disabling_a_dependency_is_blocked_while_a_dependent_is_enabled() {
    let e = env();
    let (a, b) = two_disabled(&e);
    e.app.add_dependency(&a, &b).expect("add");
    e.app
        .set_mod_enabled_with(&a, true, true, &NullSink)
        .expect("enable both");

    let f = err(e.app.set_mod_enabled(&b, false, &NullSink));
    assert_eq!(f.code, ErrorCode::DependencyBlocked);
    assert!(f.message.contains("\"B\"") && f.message.contains("\"A\""));
    assert_eq!(state_of(&e, &b), ModState::Enabled);

    let preview = e.app.preview_set_mod_enabled(&b, false).expect("preview");
    assert_eq!(preview.mods_affected.len(), 2);

    let n = tx_count(&e);
    e.app
        .set_mod_enabled_with(&b, false, true, &NullSink)
        .expect("disable both");
    assert_eq!(state_of(&e, &a), ModState::Disabled);
    assert_eq!(state_of(&e, &b), ModState::Disabled);
    assert_eq!(tx_count(&e), n + 1);
}

#[test]
fn dependency_rules_are_validated() {
    let e = env();
    let (a, b) = two_disabled(&e);
    e.app.add_dependency(&a, &b).expect("add");
    assert_eq!(
        err(e.app.add_dependency(&b, &a)).code,
        ErrorCode::InvalidInput,
        "cycle"
    );
    assert_eq!(
        err(e.app.add_dependency(&a, &a)).code,
        ErrorCode::InvalidInput,
        "self"
    );
    assert_eq!(
        err(e.app.add_dependency(&a, &b)).code,
        ErrorCode::InvalidInput,
        "duplicate"
    );
    assert_eq!(
        err(e.app.add_dependency(&a, "not-an-id")).code,
        ErrorCode::InvalidInput
    );
    let removed = e.app.remove_dependency(&a, &b).expect("remove");
    assert!(removed.depends_on.is_empty());
    assert_eq!(removed.summary.depends_on_count, 0);
    assert_eq!(
        err(e.app.remove_dependency(&a, &b)).code,
        ErrorCode::NotFound
    );
    // Without the dependency the mod enables on its own again.
    assert_eq!(set(&e, &a, true).state, ModState::Enabled);
}

// ----------------------------------------------------------------- vault

#[test]
fn vault_report_and_gc_only_touch_unreferenced_objects() {
    let e = env();
    let p = stage(&e, &newmod_zip(&e));
    let m = e
        .app
        .confirm_import(confirm_req(&p, "New Mod", true), &NullSink)
        .expect("confirm");
    let other = adopt(&e, &BETA, "Beta");

    let report = e.app.vault_report().expect("report");
    assert_eq!(report.object_count, 5);
    assert!(report.total_bytes > 0);
    assert!(report.unreferenced.is_empty());
    assert_eq!(report.unreferenced_bytes, 0);
    assert!(report.stray_files.is_empty());

    // Forgetting the imported mod leaves its three objects without an owner.
    e.app.forget_mod(&m.id).expect("forget");
    let report = e.app.vault_report().expect("report");
    assert_eq!(report.unreferenced.len(), 3);
    assert_eq!(report.object_count, 5);
    assert_eq!(
        report.unreferenced_bytes,
        report.unreferenced.iter().map(|o| o.size).sum::<u64>()
    );

    // A referenced object is refused and nothing is deleted.
    let beta_sha = e.app.get_mod(&other.id).expect("beta").entries[0]
        .sha256
        .clone();
    assert_eq!(
        err(e.app.vault_gc(std::slice::from_ref(&beta_sha))).code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        err(e.app.vault_gc(&["zz".to_owned()])).code,
        ErrorCode::InvalidInput
    );

    let shas: Vec<String> = report
        .unreferenced
        .iter()
        .map(|o| o.sha256.clone())
        .collect();
    let freed = e.app.vault_gc(&shas).expect("gc");
    assert_eq!(freed, report.unreferenced_bytes);
    let after = e.app.vault_report().expect("report");
    assert!(after.unreferenced.is_empty());
    assert_eq!(after.object_count, 2);
    for sha in &shas {
        let object = e.dirs.vault_dir.join("objects").join(&sha[..2]).join(sha);
        assert!(!object.exists(), "object file removed");
    }
    // The referenced mod still enables.
    assert_eq!(state_of(&e, &other.id), ModState::Enabled);
    assert_eq!(e.app.vault_gc(&shas).expect("idempotent"), 0);
}

#[test]
fn vault_gc_is_blocked_by_an_unresolved_transaction() {
    let e = env();
    let Env {
        _tmp,
        dirs,
        app,
        os,
        install,
        id,
    } = e;
    drop(app);
    {
        let db = Database::open(&dirs.db_path).expect("db");
        let inst = db
            .list_installations()
            .expect("installations")
            .into_iter()
            .next()
            .expect("installation");
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
    }
    let app = App::init_with(
        dirs,
        AppOptions {
            os,
            hints: DiscoveryHints::default(),
            space: Arc::new(FakeSpace),
        },
    )
    .expect("restart");
    let f = err(app.vault_gc(&[]));
    assert_eq!(f.code, ErrorCode::RecoveryRequired);
    drop((install, id, _tmp));
}

// ------------------------------------------------------- entry metadata

struct VersionOs;

impl OsProbe for VersionOs {
    fn registry_string(&self, _hive: Hive, _key: &str, _value: &str) -> Option<String> {
        None
    }
    fn file_version(&self, path: &Path) -> Option<String> {
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some(format!("1.2.3.4 {name}"))
    }
    fn running_process_names(&self) -> Vec<String> {
        Vec::new()
    }
}

#[test]
fn entries_of_enabled_mods_carry_a_file_version() {
    let tmp = TempDir::new().expect("tempdir");
    let base = dunce::canonicalize(tmp.path()).expect("canonical");
    let app = App::init_with(
        gmm_app::AppDirs::at(&base.join("data")),
        AppOptions {
            os: Arc::new(VersionOs),
            hints: DiscoveryHints::default(),
            space: Arc::new(FakeSpace),
        },
    )
    .expect("init");
    let install = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .asi_mod("trainer")
        .build()
        .expect("install");
    let inst = app
        .register_installation(RegisterInstallationRequest {
            root_path: install.root().to_string_lossy().into_owned(),
            display_name: None,
            platform: None,
        })
        .expect("register");
    app.scan_installation(&inst.id, &NullSink).expect("scan");
    let m = app
        .adopt_mod(
            AdoptRequest {
                installation_id: inst.id.clone(),
                rel_paths: TRAINER.iter().map(|s| (*s).to_owned()).collect(),
                name: "Trainer".into(),
                category: Category::ScriptsGameplay,
                notes: None,
                confirm_unknown: true,
            },
            &NullSink,
        )
        .expect("adopt");

    let version_of = |detail: &gmm_app::ModDetailDto, path: &str| {
        detail
            .entries
            .iter()
            .find(|e| e.rel_path == path)
            .expect("entry")
            .file_version
            .clone()
    };
    let enabled = app.get_mod(&m.id).expect("detail");
    assert_eq!(
        version_of(&enabled, "trainer.asi").as_deref(),
        Some("1.2.3.4 trainer.asi")
    );
    assert_eq!(
        version_of(&enabled, "trainer.ini"),
        None,
        "not a PE file type"
    );

    app.set_mod_enabled(&m.id, false, &NullSink)
        .expect("disable");
    let disabled = app.get_mod(&m.id).expect("detail");
    assert!(disabled.entries.iter().all(|e| e.file_version.is_none()));
}
