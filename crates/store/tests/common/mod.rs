//! Shared helpers for journal and mods tests.
#![allow(dead_code, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use gmm_domain::{
    BaselineTrust, Category, DistributionPlatform, Edition, InstallMechanism, Installation,
    InstallationId, InstallationStatus, Mod, ModEntry, ModEntryId, ModId, ModState, RelPath,
    Sha256, TransactionId, TransactionKind, TransactionStatus, root_key,
};
use gmm_store::{AdoptRecord, CommitEffects, Database, TransactionRow};

pub fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(1_800_000_000_000 + secs * 1000)
        .unwrap()
}

pub fn sha(n: u8) -> Sha256 {
    Sha256::from_bytes([n; 32])
}

pub fn rp(s: &str) -> RelPath {
    RelPath::parse(s).unwrap()
}

pub fn add_installation(db: &Database, root: &str) -> InstallationId {
    let id = InstallationId::new();
    db.insert_installation(&Installation {
        id,
        edition: Edition::Legacy,
        platform: DistributionPlatform::Steam,
        display_name: "T".into(),
        root_path: PathBuf::from(root),
        root_key: root_key(Path::new(root)),
        exe_version: None,
        status: InstallationStatus::Ok,
        baseline: BaselineTrust::None,
        created_at: ts(0),
        updated_at: ts(0),
    })
    .unwrap();
    id
}

/// A file-backed test database (so tests can count rows through a second connection).
pub struct Env {
    db: Database,
    path: PathBuf,
    _dir: tempfile::TempDir,
}

impl std::ops::Deref for Env {
    type Target = Database;
    fn deref(&self) -> &Database {
        &self.db
    }
}

pub fn setup() -> (Env, InstallationId) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let db = Database::open(&path).unwrap();
    let id = add_installation(&db, "C:/Games/Test");
    (
        Env {
            db,
            path,
            _dir: dir,
        },
        id,
    )
}

pub fn tx_row(inst: InstallationId, kind: TransactionKind, n: i64) -> TransactionRow {
    TransactionRow {
        id: TransactionId::new(),
        installation_id: inst,
        kind,
        status: TransactionStatus::Open,
        summary_json: "{}".into(),
        error: None,
        created_at: ts(n),
        finished_at: None,
    }
}

pub fn new_tx(db: &Database, inst: InstallationId, kind: TransactionKind, n: i64) -> TransactionId {
    let row = tx_row(inst, kind, n);
    let steps = if kind == TransactionKind::Adopt {
        vec![
            gmm_store::NewStep {
                seq: 0,
                kind: gmm_domain::StepKind::CaptureToVault,
                mod_id: None,
                rel_path: rp("a/b.dll"),
                sha256: Some(sha(1)),
            },
            gmm_store::NewStep {
                seq: 1,
                kind: gmm_domain::StepKind::EnsureDir,
                mod_id: None,
                rel_path: rp("a"),
                sha256: None,
            },
        ]
    } else {
        vec![]
    };
    db.create_transaction(&row, &steps, ts(n)).unwrap();
    row.id
}

pub fn new_mod(inst: InstallationId, name: &str, state: ModState) -> Mod {
    Mod {
        id: ModId::new(),
        installation_id: inst,
        name: name.into(),
        category: Category::Vehicles,
        state,
        mechanism: InstallMechanism::LooseFiles,
        notes: None,
        created_at: ts(0),
        updated_at: ts(0),
    }
}

pub fn adopt(m: &Mod, files: &[(&str, u8)]) -> AdoptRecord {
    AdoptRecord {
        r#mod: m.clone(),
        entries: files
            .iter()
            .map(|(p, h)| ModEntry {
                id: ModEntryId::new(),
                mod_id: m.id,
                rel_path: rp(p),
                sha256: sha(*h),
                size: 10,
            })
            .collect(),
        included_unknown: false,
        manifest_json: "[]".into(),
        confirmed_at: ts(0),
    }
}

/// Adopts a mod through a full open -> committing -> committed cycle.
pub fn adopt_mod(
    db: &Database,
    inst: InstallationId,
    name: &str,
    state: ModState,
    hash: u8,
    n: i64,
) -> Mod {
    adopt_mod_files(db, inst, name, state, &[(&format!("f{hash}.dll"), hash)], n)
}

pub fn adopt_mod_files(
    db: &Database,
    inst: InstallationId,
    name: &str,
    state: ModState,
    files: &[(&str, u8)],
    n: i64,
) -> Mod {
    let tx = new_tx(db, inst, TransactionKind::Enable, n);
    db.set_transaction_status(tx, TransactionStatus::Committing, None, ts(n))
        .unwrap();
    let m = new_mod(inst, name, state);
    db.commit_transaction(
        tx,
        &CommitEffects {
            adopt: Some(adopt(&m, files)),
            vault_objects: files.iter().map(|(_, h)| (sha(*h), 10)).collect(),
            mod_states: vec![],
            ..CommitEffects::default()
        },
        ts(n),
    )
    .unwrap();
    m
}

pub fn db_path(env: &Env) -> &Path {
    &env.path
}

pub fn count(env: &Env, table: &str) -> i64 {
    let conn = rusqlite::Connection::open(&env.path).unwrap();
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
