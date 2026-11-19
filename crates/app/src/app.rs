//! The application root object.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use gmm_domain::{InstallationId, InstallationStatus};
use gmm_engine::{RecoveryReport, Vault, recover};
use gmm_platform::{AppDirs, StdFs};
use gmm_scanner::{Catalogs, DiscoveryHints, OsProbe, SystemOs};
use gmm_store::Database;

use crate::error::{AppError, ErrorCode};
use crate::guards::{SpaceProbe, SystemSpace};
use crate::lock::InstanceLock;
use crate::operation::RunningOp;

/// Everything about the environment that tests replace.
#[derive(Clone)]
pub struct AppOptions {
    /// Operating system queries (registry, processes, version resources).
    pub os: Arc<dyn OsProbe>,
    /// Where installation discovery looks.
    pub hints: DiscoveryHints,
    /// Free disk space reporting.
    pub space: Arc<dyn SpaceProbe>,
}

impl Default for AppOptions {
    /// The real system and the default Windows discovery locations.
    fn default() -> Self {
        Self {
            os: Arc::new(SystemOs),
            hints: DiscoveryHints::windows_defaults(),
            space: Arc::new(SystemSpace),
        }
    }
}

impl fmt::Debug for AppOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppOptions")
            .field("hints", &self.hints)
            .finish_non_exhaustive()
    }
}

/// Owns the data directories, the single-instance lock and the database.
pub struct App {
    pub(crate) dirs: AppDirs,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) db: Mutex<Database>,
    pub(crate) os: Arc<dyn OsProbe>,
    pub(crate) space: Arc<dyn SpaceProbe>,
    pub(crate) catalogs: Catalogs,
    pub(crate) hints: DiscoveryHints,
    /// What startup recovery did.
    pub(crate) recovery: RecoveryReport,
    /// The single slot for heavy operations (scan, baseline capture, mod transactions).
    pub(crate) op: Mutex<Option<RunningOp>>,
    /// The archive staged for import (one at a time), see `import.rs`.
    pub(crate) staged: Mutex<Option<crate::import::StagedImport>>,
    // Declared last so the lock is released after the database is closed.
    _lock: InstanceLock,
}

impl fmt::Debug for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("dirs", &self.dirs)
            .field("started_at", &self.started_at)
            .field("hints", &self.hints)
            .finish_non_exhaustive()
    }
}

impl App {
    /// Creates the directories, takes the instance lock, opens the database and recovers
    /// interrupted transactions, using the real operating system.
    pub fn init(dirs: AppDirs) -> Result<Self, AppError> {
        Self::init_with(dirs, AppOptions::default())
    }

    /// Like [`App::init`] with explicit options (used by tests).
    ///
    /// Recovery runs right after the database is opened and before anything else can act;
    /// a database failure during recovery fails the start with `DATABASE`.
    pub fn init_with(dirs: AppDirs, options: AppOptions) -> Result<Self, AppError> {
        dirs.ensure_created(&StdFs)?;
        let lock = InstanceLock::acquire(&dirs.data_dir)?;
        let db = Database::open(&dirs.db_path)?;
        let recovery = recover_at_startup(&db, &dirs)?;
        crate::import::remove_leftover_staging(&dirs);
        let catalogs = Catalogs::load()?;
        let schema_version = db.schema_version()?;
        tracing::info!(
            data_dir = %dirs.data_dir.display(),
            vault_dir = %dirs.vault_dir.display(),
            logs_dir = %dirs.logs_dir.display(),
            db_path = %dirs.db_path.display(),
            schema_version,
            "application initialized"
        );
        Ok(Self {
            dirs,
            started_at: Utc::now(),
            db: Mutex::new(db),
            os: options.os,
            space: options.space,
            catalogs,
            hints: options.hints,
            recovery,
            op: Mutex::new(None),
            staged: Mutex::new(None),
            _lock: lock,
        })
    }

    pub(crate) fn lock_db(&self) -> Result<MutexGuard<'_, Database>, AppError> {
        self.db.lock().map_err(|_| {
            AppError::new(ErrorCode::Internal, "The database handle is unavailable.")
                .with_details("database mutex poisoned")
        })
    }
}

/// Runs the engine's recovery for every unfinished transaction. Installations whose folder
/// is missing are reported as unavailable (their transactions are skipped).
fn recover_at_startup(db: &Database, dirs: &AppDirs) -> Result<RecoveryReport, AppError> {
    let roots: HashMap<InstallationId, PathBuf> = db
        .list_installations()?
        .into_iter()
        .filter(|i| i.status != InstallationStatus::Missing)
        .map(|i| (i.id, i.root_path))
        .collect();
    let vault = Vault::new(&dirs.vault_dir);
    let report = recover(&StdFs::new(), db, &vault, &|id| roots.get(&id).cloned())?;
    tracing::info!(
        rolled_back = report.rolled_back.len(),
        committed = report.committed.len(),
        needs_attention = report.needs_attention.len(),
        skipped = report.skipped.len(),
        "startup recovery finished"
    );
    if !report.needs_attention.is_empty() || !report.skipped.is_empty() {
        tracing::warn!(
            needs_attention = ?report.needs_attention,
            skipped = ?report.skipped,
            "some transactions still need attention"
        );
    }
    Ok(report)
}
