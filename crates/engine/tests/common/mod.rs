//! Shared fixtures for the engine integration tests.
#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use gmm_domain::{
    BaselineTrust, Category, DistributionPlatform, Edition, Installation, InstallationId,
    InstallationStatus, ModId, ModState, RelPath, TransactionId, TransactionStatus, parse_artifact,
    root_key,
};
use gmm_engine::{
    EngineContext, Plan, Vault, execute, plan_adopt, plan_disable, plan_enable, plan_readopt,
    plan_restore, plan_set_states, reconcile,
};
use gmm_platform::{Fs, StdFs};
use gmm_store::Database;
use gmm_testkit::{FakeInstall, Snapshot, base_files, fake_bytes};
use tempfile::TempDir;

pub fn rp(s: &str) -> RelPath {
    RelPath::parse(s).expect("valid rel path")
}

/// Files of the main test mod (nested, multi-file).
pub const M1_FILES: &[&str] = &[
    "Trainer.asi",
    "Trainer.ini",
    "scripts/sub/deep/x.dll",
    "scripts/y.ini",
];

/// A fake installation, an app directory (database + vault) and a registered installation row.
pub struct Fixture {
    pub install: FakeInstall,
    pub app: TempDir,
    pub db: Database,
    pub vault: Vault,
    pub inst: InstallationId,
    pub is_protected: Box<dyn Fn(&RelPath) -> bool + Send + Sync>,
    base_snapshot: Vec<(String, String)>,
}

impl Fixture {
    pub fn new() -> Self {
        let mut b = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .file("scripts/keep.txt", b"unmanaged".to_vec());
        for f in M1_FILES {
            b = b.file(f, fake_bytes(f, 300));
        }
        let install = b.build().expect("fake install");
        let app = TempDir::new().expect("app dir");
        let db = Database::open(&app.path().join("gmm.sqlite3")).expect("db");
        let vault = Vault::new(app.path().join("vault"));
        let inst = InstallationId::new();
        db.insert_installation(&Installation {
            id: inst,
            edition: Edition::Legacy,
            platform: DistributionPlatform::Steam,
            display_name: "Test".into(),
            root_path: install.root().to_path_buf(),
            root_key: root_key(install.root()),
            exe_version: None,
            status: InstallationStatus::Ok,
            baseline: BaselineTrust::None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .expect("insert installation");
        let protected_keys: HashSet<String> = base_files(Edition::Legacy)
            .iter()
            .map(|f| rp(f).key())
            .collect();
        let is_protected = Box::new(move |p: &RelPath| protected_keys.contains(&p.key()));
        let base_snapshot = base_hashes(&install);
        Self {
            install,
            app,
            db,
            vault,
            inst,
            is_protected,
            base_snapshot,
        }
    }

    pub fn ctx<'a>(&'a self, fs: &'a dyn Fs) -> EngineContext<'a> {
        EngineContext {
            fs,
            db: &self.db,
            vault: &self.vault,
            root: self.install.root(),
            installation_id: self.inst,
            is_protected: &*self.is_protected,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.app.path().join("gmm.sqlite3")
    }

    /// Drops the connection and reopens the database file (a fresh process after a crash).
    pub fn reopen_db(&mut self) {
        self.db = Database::open_in_memory().expect("in-memory db");
        self.db = Database::open(&self.db_path()).expect("reopen db");
    }

    pub fn adopt(&self, name: &str, files: &[&str]) -> ModId {
        let fs = StdFs;
        let ctx = self.ctx(&fs);
        let id = ModId::new();
        let rels: Vec<RelPath> = files.iter().map(|f| rp(f)).collect();
        let plan = plan_adopt(&ctx, id, name, Category::Uncategorized, None, &rels, false)
            .expect("plan adopt");
        execute(&ctx, plan).expect("execute adopt");
        id
    }

    pub fn run(&self, f: impl FnOnce(&EngineContext<'_>) -> gmm_engine::Result<Plan>) {
        let fs = StdFs;
        let ctx = self.ctx(&fs);
        let plan = f(&ctx).expect("plan");
        execute(&ctx, plan).expect("execute");
    }

    pub fn disable(&self, id: ModId) {
        self.run(|c| plan_disable(c, &[id]));
    }

    pub fn enable(&self, id: ModId) {
        self.run(|c| plan_enable(c, &[id]));
    }

    pub fn set_states(&self, d: &[(ModId, bool)]) {
        self.run(|c| plan_set_states(c, d));
    }

    pub fn reconcile(&self) -> Vec<gmm_engine::DriftItem> {
        reconcile(
            &StdFs,
            &self.db,
            &self.vault,
            self.install.root(),
            self.inst,
        )
        .expect("reconcile")
    }

    /// `(rel path, sha256)` of every regular file in the game folder.
    pub fn game_hashes(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        walk_hashes(self.install.root(), self.install.root(), &mut out);
        out.sort();
        out
    }

    pub fn mod_state(&self, id: ModId) -> ModState {
        self.db.get_mod(id).expect("db").expect("mod exists").state
    }

    /// `(name, state)` of every mod, sorted by name.
    pub fn states(&self) -> Vec<(String, ModState)> {
        let mut v: Vec<_> = self
            .db
            .list_mods(self.inst)
            .expect("list mods")
            .into_iter()
            .map(|m| (m.name, m.state))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    pub fn snapshot(&self) -> Snapshot {
        self.install.snapshot().expect("snapshot")
    }

    pub fn tx_ids(&self) -> Vec<TransactionId> {
        self.db
            .list_transactions(self.inst, 1000, 0)
            .expect("list tx")
            .0
            .into_iter()
            .map(|t| t.id)
            .collect()
    }

    pub fn tx_status(&self, tx: TransactionId) -> TransactionStatus {
        self.db
            .load_transaction(tx)
            .expect("db")
            .expect("tx exists")
            .0
            .status
    }

    /// The transaction created after `before` (a previous `tx_ids()`), if any.
    pub fn new_tx(&self, before: &[TransactionId]) -> Option<TransactionId> {
        self.tx_ids().into_iter().find(|t| !before.contains(t))
    }

    /// Protected original files must be byte-identical to what the fixture created (I-01).
    pub fn assert_protected_intact(&self) {
        assert_eq!(base_hashes(&self.install), self.base_snapshot);
    }

    /// Engine artifacts (`*.gmm-tmp-*`, `*.gmm-tomb-*`) left in the game folder or the vault.
    pub fn leftovers(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_artifacts(self.install.root(), &mut out);
        collect_artifacts(&self.vault.root, &mut out);
        out
    }
}

fn base_hashes(install: &FakeInstall) -> Vec<(String, String)> {
    let fs = StdFs;
    base_files(install.edition())
        .iter()
        .map(|f| {
            let h = gmm_platform::sha256_file(&fs, &install.path(f)).expect("hash base file");
            ((*f).to_owned(), h)
        })
        .collect()
}

fn walk_hashes(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_hashes(root, &p, out);
        } else {
            let rel = p.strip_prefix(root).expect("under root");
            let h = gmm_platform::sha256_file(&StdFs, &p).expect("hash");
            out.push((rel.to_string_lossy().replace('\\', "/"), h));
        }
    }
}

fn collect_artifacts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_artifacts(&p, out);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| parse_artifact(n).is_some())
        {
            out.push(p);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fault-injection scenarios
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Adopt,
    Enable,
    Disable,
    Group,
    /// Drifted mod (one modified, two missing incl. a whole directory) restored from the vault.
    Restore,
    /// Drifted mod (two modified, one missing) re-adopted.
    Readopt,
}

pub const KINDS: [Kind; 4] = [Kind::Adopt, Kind::Enable, Kind::Disable, Kind::Group];
pub const DRIFT_KINDS: [Kind; 2] = [Kind::Restore, Kind::Readopt];

/// A prepared pre-state plus the transaction under test.
pub struct Scenario {
    pub fx: Fixture,
    pub kind: Kind,
    m1: ModId,
    m2: Option<ModId>,
}

/// Alternative M2: same `Trainer.asi` path as M1 but different content.
const M2_FILES: &[(&str, usize)] = &[("Trainer.asi", 411), ("scripts/other.dll", 123)];

impl Scenario {
    /// Builds the pre-state fault-free (setup transactions use a plain `StdFs`).
    pub fn new(kind: Kind) -> Self {
        let fx = Fixture::new();
        let (m1, m2) = match kind {
            Kind::Adopt => (ModId::new(), None),
            Kind::Disable => (fx.adopt("M1", M1_FILES), None),
            Kind::Enable => {
                let m1 = fx.adopt("M1", M1_FILES);
                fx.disable(m1);
                fx.install.remove("scripts/sub").expect("remove dirs");
                (m1, None)
            }
            Kind::Group => {
                let m1 = fx.adopt("M1", M1_FILES);
                fx.disable(m1);
                fx.install.remove("scripts/sub").expect("remove dirs");
                let names: Vec<&str> = M2_FILES.iter().map(|(f, _)| *f).collect();
                for (f, len) in M2_FILES {
                    fx.install
                        .write(f, &fake_bytes(&format!("alt/{f}"), *len))
                        .expect("write");
                }
                (m1, Some(fx.adopt("M2", &names)))
            }
            Kind::Restore | Kind::Readopt => {
                let m1 = fx.adopt("M1", M1_FILES);
                let w = |f: &str, n: usize| {
                    fx.install
                        .write(f, &fake_bytes(&format!("user/{f}"), n))
                        .expect("write");
                };
                if kind == Kind::Restore {
                    w("Trainer.asi", 555);
                    fx.install.remove("scripts/y.ini").expect("remove");
                    fx.install.remove("scripts/sub").expect("remove dir");
                } else {
                    w("Trainer.asi", 555);
                    w("Trainer.ini", 77);
                    fx.install.remove("scripts/y.ini").expect("remove");
                }
                fx.reconcile();
                assert_eq!(fx.mod_state(m1), ModState::Drifted);
                (m1, None)
            }
        };
        Self { fx, kind, m1, m2 }
    }

    /// Plans and executes the transaction under test through `fs`.
    pub fn run(&self, fs: &dyn Fs) -> gmm_engine::Result<TransactionId> {
        let ctx = self.fx.ctx(fs);
        let plan = match self.kind {
            Kind::Adopt => {
                let files: Vec<RelPath> = M1_FILES.iter().map(|f| rp(f)).collect();
                plan_adopt(
                    &ctx,
                    self.m1,
                    "Adopted",
                    Category::Vehicles,
                    None,
                    &files,
                    false,
                )?
            }
            Kind::Enable => plan_enable(&ctx, &[self.m1])?,
            Kind::Disable => plan_disable(&ctx, &[self.m1])?,
            Kind::Restore => plan_restore(&ctx, self.m1)?,
            Kind::Readopt => plan_readopt(&ctx, self.m1)?,
            Kind::Group => {
                let m2 = self.m2.expect("group scenario has M2");
                plan_set_states(&ctx, &[(self.m1, true), (m2, false)])?
            }
        };
        execute(&ctx, plan)
    }
}

/// Everything an assertion about "the installation is in state X" needs.
#[derive(Debug, PartialEq)]
pub struct Observed {
    pub snapshot: Snapshot,
    pub states: Vec<(String, ModState)>,
}

pub fn observe(fx: &Fixture) -> Observed {
    Observed {
        snapshot: fx.snapshot(),
        states: fx.states(),
    }
}
