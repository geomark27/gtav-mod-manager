//! Installation use cases: detect, register, list, refresh, rename and select.
//!
//! The database mutex is held only around database calls, never during file-system work.

use std::path::Path;
use std::str::FromStr;

use chrono::{SecondsFormat, Utc};
use gmm_domain::{
    BaselineTrust, DistributionPlatform, Edition, Game, Installation, InstallationId,
    InstallationStatus, root_key,
};
use gmm_platform::StdFs;
use gmm_scanner::{ValidationProblem, ValidationReport, discover, validate};
use gmm_store::SETTING_SELECTED_INSTALLATION;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};

const MAX_NAME_CHARS: usize = 80;

/// A registered installation as shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallationDto {
    /// Identifier (UUID).
    pub id: String,
    /// Game, derived from the edition.
    pub game: Game,
    /// Game edition.
    pub edition: Edition,
    /// Distribution platform.
    pub platform: DistributionPlatform,
    /// Name shown in the UI.
    pub display_name: String,
    /// Canonical root folder.
    pub root_path: String,
    /// Executable version, when known.
    pub exe_version: Option<String>,
    /// Folder health.
    pub status: InstallationStatus,
    /// Baseline trust level.
    pub baseline: BaselineTrust,
    /// Creation time, RFC 3339 UTC.
    pub created_at: String,
    /// Last update time, RFC 3339 UTC.
    pub updated_at: String,
}

impl From<&Installation> for InstallationDto {
    fn from(i: &Installation) -> Self {
        Self {
            id: i.id.to_string(),
            game: i.edition.game(),
            edition: i.edition,
            platform: i.platform,
            display_name: i.display_name.clone(),
            root_path: i.root_path.to_string_lossy().into_owned(),
            exe_version: i.exe_version.clone(),
            status: i.status,
            baseline: i.baseline,
            created_at: i.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            updated_at: i.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        }
    }
}

/// A folder found by detection, with its validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiscoveredInstallationDto {
    /// Canonical root folder.
    pub root_path: String,
    /// Store it was found through.
    pub platform: DistributionPlatform,
    /// Where it was found, for example `steam:libraryfolders`.
    pub source: String,
    /// Detected edition, when known.
    pub edition: Option<Edition>,
    /// Validation status.
    pub status: InstallationStatus,
    /// Required files that are missing.
    pub missing_files: Vec<String>,
    /// Executable version, when known.
    pub exe_version: Option<String>,
    /// Whether the folder accepted the write probe.
    pub writable: bool,
    /// Whether this folder is already registered.
    pub already_registered: bool,
    /// Whether it can be registered (known edition, status `ok` or `not_writable`).
    pub registrable: bool,
    /// Short user-readable sentences, one per validation problem.
    pub problems: Vec<String>,
}

/// Request of `register_installation`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterInstallationRequest {
    /// Folder to register.
    pub root_path: String,
    /// Name to show; a default is generated when `null` or blank.
    pub display_name: Option<String>,
    /// Platform; inferred from the path when `null`.
    pub platform: Option<DistributionPlatform>,
}

fn problem_text(p: ValidationProblem) -> &'static str {
    match p {
        ValidationProblem::RootNotFound => "The folder does not exist.",
        ValidationProblem::NotADirectory => "The path is not a folder.",
        ValidationProblem::NoExecutable => {
            "No GTA V executable (GTA5.exe or GTA5_Enhanced.exe) was found in this folder."
        }
        ValidationProblem::AmbiguousEdition => {
            "Both the Legacy and Enhanced executables are present, so the edition is ambiguous."
        }
        ValidationProblem::MissingRequiredFiles => "Some required game files are missing.",
        ValidationProblem::NotWritable => "The folder is read-only for this app.",
    }
}

fn problems(report: &ValidationReport) -> Vec<String> {
    report
        .problems
        .iter()
        .map(|p| problem_text(*p).to_owned())
        .collect()
}

fn is_registrable(report: &ValidationReport) -> bool {
    report.edition.is_some()
        && matches!(
            report.status,
            InstallationStatus::Ok | InstallationStatus::NotWritable
        )
}

fn infer_platform(root: &Path) -> DistributionPlatform {
    let p = root.to_string_lossy().to_lowercase().replace('\\', "/");
    if p.contains("/steamapps/common/") {
        DistributionPlatform::Steam
    } else if p.contains("epic games") {
        DistributionPlatform::Epic
    } else {
        DistributionPlatform::Unknown
    }
}

fn platform_label(p: DistributionPlatform) -> &'static str {
    match p {
        DistributionPlatform::Steam => "Steam",
        DistributionPlatform::Epic => "Epic",
        DistributionPlatform::Rockstar => "Rockstar",
        DistributionPlatform::Unknown => "Unknown",
    }
}

pub(crate) fn parse_id(id: &str) -> Result<InstallationId, AppError> {
    Ok(InstallationId::from_str(id)?)
}

pub(crate) fn not_found() -> AppError {
    AppError::new(ErrorCode::NotFound, "The installation was not found.")
}

impl App {
    pub(crate) fn validate_root(&self, root: &Path) -> ValidationReport {
        validate(&StdFs::new(), &*self.os, &self.catalogs, root)
    }

    /// Finds candidate installations and validates each one.
    pub fn discover_installations(&self) -> Result<Vec<DiscoveredInstallationDto>, AppError> {
        let candidates = discover(&StdFs::new(), &*self.os, &self.catalogs, &self.hints);
        let registered: Vec<String> = {
            let db = self.lock_db()?;
            db.list_installations()?
                .into_iter()
                .map(|i| i.root_key)
                .collect()
        };
        let mut out = Vec::new();
        for c in candidates {
            let root = match dunce::canonicalize(&c.root) {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(root = %c.root.display(), error = %e, "candidate skipped");
                    continue;
                }
            };
            let report = self.validate_root(&root);
            out.push(DiscoveredInstallationDto {
                root_path: root.to_string_lossy().into_owned(),
                platform: c.platform,
                source: c.source,
                edition: report.edition,
                status: report.status,
                missing_files: report
                    .missing
                    .iter()
                    .map(|p| p.as_str().to_owned())
                    .collect(),
                exe_version: report.exe_version.clone(),
                writable: report.writable,
                already_registered: registered.contains(&root_key(&root)),
                registrable: is_registrable(&report),
                problems: problems(&report),
            });
        }
        Ok(out)
    }

    /// Validates and registers a folder as an installation.
    pub fn register_installation(
        &self,
        req: RegisterInstallationRequest,
    ) -> Result<InstallationDto, AppError> {
        // Resolves a junction/symlink at the root itself only; links inside stay forbidden.
        let root = dunce::canonicalize(&req.root_path).map_err(|e| {
            AppError::new(ErrorCode::InvalidInput, "The folder does not exist.")
                .with_details(format!("{}: {e}", req.root_path))
        })?;
        let report = self.validate_root(&root);
        let Some(edition) = report.edition.filter(|_| is_registrable(&report)) else {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "This folder is not a complete GTA V installation.",
            )
            .with_hint("Select the game folder that contains GTA5.exe or GTA5_Enhanced.exe.")
            .with_details(problems(&report).join(" ")));
        };
        let key = root_key(&root);
        let platform = req.platform.unwrap_or_else(|| infer_platform(&root));
        let display_name = match req.display_name.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => check_name(n)?,
            _ => format!("{} ({})", edition.display_name(), platform_label(platform)),
        };
        let now = Utc::now();
        let inst = Installation {
            id: InstallationId::new(),
            edition,
            platform,
            display_name,
            root_path: root,
            root_key: key.clone(),
            exe_version: report.exe_version,
            status: report.status,
            baseline: BaselineTrust::None,
            created_at: now,
            updated_at: now,
        };
        let db = self.lock_db()?;
        if db.find_installation_by_root_key(&key)?.is_some() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "This installation is already registered.",
            ));
        }
        db.insert_installation(&inst)?;
        if db.list_installations()?.len() == 1 {
            db.set_setting(SETTING_SELECTED_INSTALLATION, &inst.id.to_string())?;
        }
        Ok(InstallationDto::from(&inst))
    }

    /// Lists registered installations, oldest first.
    pub fn list_installations(&self) -> Result<Vec<InstallationDto>, AppError> {
        let db = self.lock_db()?;
        Ok(db
            .list_installations()?
            .iter()
            .map(InstallationDto::from)
            .collect())
    }

    /// Re-validates the stored folder and persists the new status and version.
    pub fn refresh_installation(&self, id: &str) -> Result<InstallationDto, AppError> {
        let id = parse_id(id)?;
        let inst = self
            .lock_db()?
            .get_installation(id)?
            .ok_or_else(not_found)?;
        let report = self.validate_root(&inst.root_path);
        let db = self.lock_db()?;
        db.update_installation_validation(
            id,
            report.status,
            report.exe_version.as_deref(),
            Utc::now(),
        )?;
        let updated = db.get_installation(id)?.ok_or_else(not_found)?;
        Ok(InstallationDto::from(&updated))
    }

    /// Renames an installation (trimmed, 1-80 characters).
    pub fn rename_installation(
        &self,
        id: &str,
        display_name: &str,
    ) -> Result<InstallationDto, AppError> {
        let id = parse_id(id)?;
        let name = check_name(display_name.trim())?;
        let db = self.lock_db()?;
        db.rename_installation(id, &name, Utc::now())?;
        let updated = db.get_installation(id)?.ok_or_else(not_found)?;
        Ok(InstallationDto::from(&updated))
    }

    /// Makes an installation the active one.
    pub fn select_installation(&self, id: &str) -> Result<(), AppError> {
        let id = parse_id(id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        db.set_setting(SETTING_SELECTED_INSTALLATION, &id.to_string())?;
        Ok(())
    }

    /// Returns the active installation; a stale selection is cleared and yields `None`.
    pub fn get_selected_installation(&self) -> Result<Option<InstallationDto>, AppError> {
        let db = self.lock_db()?;
        let Some(raw) = db.get_setting(SETTING_SELECTED_INSTALLATION)? else {
            return Ok(None);
        };
        let found = match InstallationId::from_str(&raw) {
            Ok(id) => db.get_installation(id)?,
            Err(_) => None,
        };
        if found.is_none() {
            db.delete_setting(SETTING_SELECTED_INSTALLATION)?;
        }
        Ok(found.as_ref().map(InstallationDto::from))
    }
}

fn check_name(name: &str) -> Result<String, AppError> {
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "The name must be between 1 and 80 characters.",
        ));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_inference() {
        let p = |s: &str| infer_platform(Path::new(s));
        assert_eq!(
            p("D:/Steam/STEAMAPPS/Common/GTA V"),
            DistributionPlatform::Steam
        );
        assert_eq!(
            p(r"C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto V"),
            DistributionPlatform::Steam
        );
        assert_eq!(
            p("C:/Program Files/Epic Games/GTAV"),
            DistributionPlatform::Epic
        );
        assert_eq!(p("E:/Games/GTA"), DistributionPlatform::Unknown);
    }

    #[test]
    fn name_limits() {
        assert!(check_name("").is_err());
        assert!(check_name(&"a".repeat(80)).is_ok());
        assert!(check_name(&"a".repeat(81)).is_err());
        assert!(check_name(&"é".repeat(80)).is_ok());
    }
}
