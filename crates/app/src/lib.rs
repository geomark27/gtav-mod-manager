//! Application layer of GTA V Mod Manager: use cases, error model, DTOs, logging and the
//! single-instance lock. The Tauri shell maps its commands 1:1 onto this crate.

pub mod adopt;
pub mod app;
pub mod baseline;
pub mod dashboard;
pub mod dependencies;
pub mod detect;
pub mod drift;
pub mod error;
pub mod guards;
pub mod history;
pub mod import;
pub mod installations;
pub mod inventory;
pub mod lock;
pub mod logging;
pub mod mods;
pub mod operation;
pub mod profiles;
pub mod scan;
pub mod status;
pub mod toggle;
mod txn;
pub mod vanilla;
pub mod vault;

pub use adopt::{AdoptFileDto, AdoptPreviewDto, AdoptRequest, BlockedFileDto};
pub use app::{App, AppOptions};
pub use baseline::BaselineDto;
pub use dashboard::{DashboardDto, ModCountsDto};
pub use dependencies::ModRefDto;
pub use detect::{
    AdoptDetectedRequest, AdoptDetectedResultDto, AmbiguousFileDto, DetectedAdoptItem,
    DetectedModDto, DetectedModsDto, DetectionConfidence, RequirementDto, RequirementStatus,
    SkippedAdoptionDto,
};
pub use error::{AppError, ErrorCode};
pub use gmm_domain::Category;
pub use gmm_platform::AppDirs;
pub use guards::{GuardNeeds, GuardStatusDto, SpaceProbe, SystemSpace};
pub use history::{TransactionDto, TransactionPageDto};
pub use import::{
    ConfirmImportRequest, ImportFileDto, ImportMappedDto, ImportPreviewDto, ImportTargetClass,
};
pub use installations::{DiscoveredInstallationDto, InstallationDto, RegisterInstallationRequest};
pub use inventory::{InventoryFilterDto, InventoryItemDto, InventoryPageDto, InventorySummaryDto};
pub use lock::InstanceLock;
pub use logging::{LogGuard, init_logging};
pub use mods::{ModConflictDto, ModDetailDto, ModDto, ModEntryDto, ModSourceDto};
pub use operation::{
    NullSink, OperationKind, OperationStatusDto, ProgressDto, ProgressSink, ThrottledSink,
};
pub use profiles::{
    ProfileDetailDto, ProfileDto, ProfileEntryDto, ProfileEntryInput, VANILLA_PROFILE_ID,
    profile_name_suggestions,
};
pub use scan::{ClassCountDto, ScanDto, ScanStatusDto, ScanSummaryDto};
pub use status::{AppStatusDto, RecoveryDto};
pub use toggle::{AffectedModDto, DriftDto, DriftKindDto, PlanPreviewDto, PlannedActionDto};
pub use vanilla::VanillaStatusDto;
pub use vault::{VaultObjectDto, VaultReportDto};
