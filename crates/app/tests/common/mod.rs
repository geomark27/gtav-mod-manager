//! Shared scaffolding: a real SQLite file, a fake installation, a fake OS and space probe.
#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use gmm_app::{
    AdoptRequest, App, AppDirs, AppError, AppOptions, ModDto, NullSink,
    RegisterInstallationRequest, SpaceProbe,
};
use gmm_domain::{Category, Edition, ModState};
use gmm_scanner::{DiscoveryHints, Hive, OsProbe};
use gmm_testkit::{FakeInstall, Snapshot};
use tempfile::TempDir;

/// OS probe whose process list can change during a test.
#[derive(Default)]
pub struct ToggleOs(pub Mutex<Vec<String>>);

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

/// Space probe with plenty of room.
pub struct FakeSpace;

impl SpaceProbe for FakeSpace {
    fn free_bytes(&self, _path: &Path) -> Option<u64> {
        Some(u64::MAX)
    }
}

pub struct Env {
    pub _tmp: TempDir,
    pub dirs: AppDirs,
    pub app: App,
    pub os: Arc<ToggleOs>,
    pub install: FakeInstall,
    pub id: String,
}

pub const TRAINER: [&str; 2] = ["trainer.asi", "trainer.ini"];
pub const ALPHA: [&str; 2] = ["scripts/alpha.dll", "scripts/alpha.ini"];
pub const BETA: [&str; 2] = ["beta.asi", "beta.ini"];

pub fn env() -> Env {
    let tmp = TempDir::new().expect("tempdir");
    let base = dunce::canonicalize(tmp.path()).expect("canonical");
    let dirs = AppDirs::at(&base.join("data"));
    let os = Arc::new(ToggleOs::default());
    let app = App::init_with(
        dirs.clone(),
        AppOptions {
            os: os.clone(),
            hints: DiscoveryHints::default(),
            space: Arc::new(FakeSpace),
        },
    )
    .expect("init");
    let install = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .asi_mod("trainer")
        .asi_mod("beta")
        .scripts_mod("alpha")
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
        app,
        os,
        install,
        id: dto.id,
    }
}

pub fn err<T>(r: Result<T, AppError>) -> AppError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

pub fn adopt(e: &Env, files: &[&str], name: &str) -> ModDto {
    e.app
        .adopt_mod(
            AdoptRequest {
                installation_id: e.id.clone(),
                rel_paths: files.iter().map(|s| (*s).to_owned()).collect(),
                name: name.to_owned(),
                category: Category::ScriptsGameplay,
                notes: None,
                confirm_unknown: true,
            },
            &NullSink,
        )
        .expect("adopt")
}

pub fn set(e: &Env, mod_id: &str, enabled: bool) -> ModDto {
    e.app
        .set_mod_enabled(mod_id, enabled, &NullSink)
        .expect("set state")
}

pub fn state_of(e: &Env, mod_id: &str) -> ModState {
    e.app.get_mod(mod_id).expect("mod").summary.state
}

pub fn snapshot(e: &Env) -> Snapshot {
    e.install.snapshot().expect("snapshot")
}

pub fn tx_count(e: &Env) -> u64 {
    e.app.list_transactions(&e.id, 1, 0).expect("history").total
}
