//! Tauri commands. Each maps 1:1 onto a `gmm-app` use case; heavy work runs on the
//! blocking pool so the UI thread never stalls.

use std::sync::Arc;
use std::time::Duration;

use gmm_app::{
    AdoptDetectedRequest, AdoptDetectedResultDto, AdoptPreviewDto, AdoptRequest, App, AppError,
    AppStatusDto, BaselineDto, Category, ConfirmImportRequest, DashboardDto, DetectedModsDto,
    DiscoveredInstallationDto, DriftDto, ErrorCode, GuardStatusDto, ImportPreviewDto,
    InstallationDto, InventoryFilterDto, InventoryPageDto, InventorySummaryDto, ModDetailDto,
    ModDto, OperationStatusDto, PlanPreviewDto, ProfileDetailDto, ProfileDto, ProfileEntryInput,
    ProgressDto, ProgressSink, RegisterInstallationRequest, ScanSummaryDto, ThrottledSink,
    TransactionPageDto, VanillaStatusDto, VaultReportDto,
    profile_name_suggestions as suggested_profile_names,
};
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// Name of the event that carries [`ProgressDto`] to the UI.
const PROGRESS_EVENT: &str = "gmm://progress";

/// At most one progress event per this interval (the final `done` event always passes).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Forwards progress to the webview as the `gmm://progress` event.
struct EventSink(AppHandle);

impl ProgressSink for EventSink {
    fn emit(&self, p: &ProgressDto) {
        if let Err(e) = self.0.emit(PROGRESS_EVENT, p) {
            tracing::debug!(error = %e, "progress event not delivered");
        }
    }
}

fn progress_sink(handle: AppHandle) -> ThrottledSink<EventSink> {
    ThrottledSink::new(EventSink(handle), PROGRESS_INTERVAL)
}

/// Runs a use case on the blocking pool and flattens the join error.
async fn run_blocking<T, F>(state: &State<'_, AppState>, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(Arc<App>) -> Result<T, AppError> + Send + 'static,
{
    let app = state.app()?;
    tauri::async_runtime::spawn_blocking(move || f(app))
        .await
        .map_err(|e| {
            AppError::new(ErrorCode::Internal, "The background task failed.")
                .with_details(e.to_string())
        })?
}

/// Returns the system status (schema version, pragmas, data paths).
#[tauri::command]
pub async fn get_app_status(state: State<'_, AppState>) -> Result<AppStatusDto, AppError> {
    run_blocking(&state, |app| app.status()).await
}

/// Detects installations and validates each candidate.
#[tauri::command]
pub async fn discover_installations(
    state: State<'_, AppState>,
) -> Result<Vec<DiscoveredInstallationDto>, AppError> {
    run_blocking(&state, |app| app.discover_installations()).await
}

/// Registers a folder as an installation.
#[tauri::command]
pub async fn register_installation(
    state: State<'_, AppState>,
    req: RegisterInstallationRequest,
) -> Result<InstallationDto, AppError> {
    run_blocking(&state, move |app| app.register_installation(req)).await
}

/// Lists registered installations.
#[tauri::command]
pub async fn list_installations(
    state: State<'_, AppState>,
) -> Result<Vec<InstallationDto>, AppError> {
    run_blocking(&state, |app| app.list_installations()).await
}

/// Re-validates an installation.
#[tauri::command]
pub async fn refresh_installation(
    state: State<'_, AppState>,
    id: String,
) -> Result<InstallationDto, AppError> {
    run_blocking(&state, move |app| app.refresh_installation(&id)).await
}

/// Renames an installation.
#[tauri::command]
pub async fn rename_installation(
    state: State<'_, AppState>,
    id: String,
    display_name: String,
) -> Result<InstallationDto, AppError> {
    run_blocking(&state, move |app| {
        app.rename_installation(&id, &display_name)
    })
    .await
}

/// Selects the active installation.
#[tauri::command]
pub async fn select_installation(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    run_blocking(&state, move |app| app.select_installation(&id)).await
}

/// Returns the active installation, if any.
#[tauri::command]
pub async fn get_selected_installation(
    state: State<'_, AppState>,
) -> Result<Option<InstallationDto>, AppError> {
    run_blocking(&state, |app| app.get_selected_installation()).await
}

/// Scans an installation, emitting `gmm://progress` events.
#[tauri::command]
pub async fn scan_installation(
    handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ScanSummaryDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.scan_installation(&id, &sink)).await
}

/// Lists inventory items with filter and pagination.
#[tauri::command]
pub async fn list_inventory(
    state: State<'_, AppState>,
    id: String,
    filter: InventoryFilterDto,
) -> Result<InventoryPageDto, AppError> {
    run_blocking(&state, move |app| app.list_inventory(&id, filter)).await
}

/// Returns counts, last scan and baseline of an installation.
#[tauri::command]
pub async fn inventory_summary(
    state: State<'_, AppState>,
    id: String,
) -> Result<InventorySummaryDto, AppError> {
    run_blocking(&state, move |app| app.inventory_summary(&id)).await
}

/// Captures the baseline, emitting `gmm://progress` events.
#[tauri::command]
pub async fn capture_baseline(
    handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
    trusted: bool,
) -> Result<BaselineDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.capture_baseline(&id, trusted, &sink)).await
}

/// Asks the running operation to stop.
#[tauri::command]
pub async fn cancel_operation(state: State<'_, AppState>) -> Result<(), AppError> {
    run_blocking(&state, |app| {
        app.cancel_operation();
        Ok(())
    })
    .await
}

/// Reports whether a heavy operation is running. The updater waits for it before restarting.
#[tauri::command]
pub fn operation_status(state: State<'_, AppState>) -> Result<OperationStatusDto, AppError> {
    Ok(state.app()?.operation_status())
}

/// Reports the state of every guard (running game, writable folder, recovery) of an installation.
#[tauri::command]
pub async fn guard_status(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<GuardStatusDto, AppError> {
    run_blocking(&state, move |app| app.guard_status(&installation_id)).await
}

/// Lists the mods of an installation.
#[tauri::command]
pub async fn list_mods(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<Vec<ModDto>, AppError> {
    run_blocking(&state, move |app| app.list_mods(&installation_id)).await
}

/// Returns one mod with its files.
#[tauri::command]
pub async fn get_mod(state: State<'_, AppState>, mod_id: String) -> Result<ModDetailDto, AppError> {
    run_blocking(&state, move |app| app.get_mod(&mod_id)).await
}

/// Shows what adopting the selected files would do.
#[tauri::command]
pub async fn preview_adopt(
    state: State<'_, AppState>,
    installation_id: String,
    rel_paths: Vec<String>,
) -> Result<AdoptPreviewDto, AppError> {
    run_blocking(&state, move |app| {
        app.preview_adopt(&installation_id, &rel_paths)
    })
    .await
}

/// Adopts files as a new mod, emitting `gmm://progress` events.
#[tauri::command]
pub async fn adopt_mod(
    handle: AppHandle,
    state: State<'_, AppState>,
    req: AdoptRequest,
) -> Result<ModDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.adopt_mod(req, &sink)).await
}

/// Enables or disables one mod (optionally with its dependency closure), emitting
/// `gmm://progress` events. `with_dependencies` defaults to `false` when omitted.
#[tauri::command]
pub async fn set_mod_enabled(
    handle: AppHandle,
    state: State<'_, AppState>,
    mod_id: String,
    enabled: bool,
    with_dependencies: Option<bool>,
) -> Result<ModDto, AppError> {
    let sink = progress_sink(handle);
    let with_dependencies = with_dependencies.unwrap_or(false);
    run_blocking(&state, move |app| {
        app.set_mod_enabled_with(&mod_id, enabled, with_dependencies, &sink)
    })
    .await
}

/// Dry run of a category switch.
#[tauri::command]
pub async fn preview_set_category(
    state: State<'_, AppState>,
    installation_id: String,
    category: Category,
    enabled: bool,
) -> Result<PlanPreviewDto, AppError> {
    run_blocking(&state, move |app| {
        app.preview_set_category(&installation_id, category, enabled)
    })
    .await
}

/// Enables or disables every mod of a category in one transaction.
#[tauri::command]
pub async fn set_category_enabled(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
    category: Category,
    enabled: bool,
) -> Result<Vec<ModDto>, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.set_category_enabled(&installation_id, category, enabled, &sink)
    })
    .await
}

/// Changes name, category and notes of a mod.
#[tauri::command]
pub async fn update_mod(
    state: State<'_, AppState>,
    mod_id: String,
    name: String,
    category: Category,
    notes: Option<String>,
) -> Result<ModDto, AppError> {
    run_blocking(&state, move |app| {
        app.update_mod(&mod_id, &name, category, notes)
    })
    .await
}

/// Ends the management of a mod whose files are in the game folder.
#[tauri::command]
pub async fn forget_mod(state: State<'_, AppState>, mod_id: String) -> Result<(), AppError> {
    run_blocking(&state, move |app| app.forget_mod(&mod_id)).await
}

/// Compares the database with the disk and returns every divergence.
#[tauri::command]
pub async fn reconcile_installation(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<Vec<DriftDto>, AppError> {
    run_blocking(&state, move |app| {
        app.reconcile_installation(&installation_id)
    })
    .await
}

/// Lists the profiles of an installation (Vanilla first).
#[tauri::command]
pub async fn list_profiles(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<Vec<ProfileDto>, AppError> {
    run_blocking(&state, move |app| app.list_profiles(&installation_id)).await
}

/// Names offered when creating a profile.
#[tauri::command]
pub async fn profile_name_suggestions() -> Result<Vec<String>, AppError> {
    Ok(suggested_profile_names())
}

/// Saves the current enabled/disabled state of every mod as a new profile.
#[tauri::command]
pub async fn create_profile_from_current(
    state: State<'_, AppState>,
    installation_id: String,
    name: String,
) -> Result<ProfileDto, AppError> {
    run_blocking(&state, move |app| {
        app.create_profile_from_current(&installation_id, &name)
    })
    .await
}

/// Returns a stored profile with its entries.
#[tauri::command]
pub async fn get_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfileDetailDto, AppError> {
    run_blocking(&state, move |app| app.get_profile(&profile_id)).await
}

/// Replaces the entries of a stored profile.
#[tauri::command]
pub async fn update_profile_entries(
    state: State<'_, AppState>,
    profile_id: String,
    entries: Vec<ProfileEntryInput>,
) -> Result<ProfileDetailDto, AppError> {
    run_blocking(&state, move |app| {
        app.update_profile_entries(&profile_id, entries)
    })
    .await
}

/// Renames a stored profile.
#[tauri::command]
pub async fn rename_profile(
    state: State<'_, AppState>,
    profile_id: String,
    name: String,
) -> Result<ProfileDto, AppError> {
    run_blocking(&state, move |app| app.rename_profile(&profile_id, &name)).await
}

/// Deletes a stored profile (the mods are not touched).
#[tauri::command]
pub async fn delete_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), AppError> {
    run_blocking(&state, move |app| app.delete_profile(&profile_id)).await
}

/// Dry run of applying a profile (or `"vanilla"`).
#[tauri::command]
pub async fn preview_apply_profile(
    state: State<'_, AppState>,
    installation_id: String,
    profile_id: String,
) -> Result<PlanPreviewDto, AppError> {
    run_blocking(&state, move |app| {
        app.preview_apply_profile(&installation_id, &profile_id)
    })
    .await
}

/// Applies a profile (or `"vanilla"`) in one transaction, emitting `gmm://progress` events.
#[tauri::command]
pub async fn apply_profile(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
    profile_id: String,
) -> Result<Vec<ModDto>, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.apply_profile(&installation_id, &profile_id, &sink)
    })
    .await
}

/// Mods proposed from the last scan: known mods by name and grouped candidates.
#[tauri::command]
pub async fn detect_mods(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<DetectedModsDto, AppError> {
    run_blocking(&state, move |app| app.detect_mods(&installation_id)).await
}

/// Adopts several proposals, one transaction each, emitting `gmm://progress` events.
#[tauri::command]
pub async fn adopt_detected(
    handle: AppHandle,
    state: State<'_, AppState>,
    req: AdoptDetectedRequest,
) -> Result<AdoptDetectedResultDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.adopt_detected(req, &sink)).await
}

/// Scans and lists what is left before playing online (read-only for the game folder),
/// emitting `gmm://progress` events.
#[tauri::command]
pub async fn check_online_readiness(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<OnlineCheckDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.check_online_readiness(&installation_id, &sink)
    })
    .await
}

/// Scans, disables every mod (vanilla mode) and runs the online checks, emitting
/// `gmm://progress` events.
#[tauri::command]
pub async fn prepare_for_online(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<OnlineCheckDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.prepare_for_online(&installation_id, &sink)
    })
    .await
}

/// Vanilla mode of an installation: restore point and how many mods are in the game.
#[tauri::command]
pub async fn vanilla_status(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<VanillaStatusDto, AppError> {
    run_blocking(&state, move |app| app.vanilla_status(&installation_id)).await
}

/// Saves a restore point and disables every mod in one transaction, emitting
/// `gmm://progress` events.
#[tauri::command]
pub async fn enter_vanilla(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<VanillaStatusDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.enter_vanilla(&installation_id, &sink)
    })
    .await
}

/// Applies the restore point in one transaction, emitting `gmm://progress` events.
#[tauri::command]
pub async fn restore_from_vanilla(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<VanillaStatusDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.restore_from_vanilla(&installation_id, &sink)
    })
    .await
}

/// Pages the transaction history of an installation, newest first.
#[tauri::command]
pub async fn list_transactions(
    state: State<'_, AppState>,
    installation_id: String,
    limit: u32,
    offset: u32,
) -> Result<TransactionPageDto, AppError> {
    run_blocking(&state, move |app| {
        app.list_transactions(&installation_id, limit, offset)
    })
    .await
}

/// Collects the dashboard of an installation.
#[tauri::command]
pub async fn dashboard(
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<DashboardDto, AppError> {
    run_blocking(&state, move |app| app.dashboard(&installation_id)).await
}

/// Puts the stored files of a drifted mod back into the game folder.
#[tauri::command]
pub async fn restore_mod(
    handle: AppHandle,
    state: State<'_, AppState>,
    mod_id: String,
) -> Result<ModDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.restore_mod(&mod_id, &sink)).await
}

/// Accepts the current files of a drifted mod as its content.
#[tauri::command]
pub async fn readopt_mod(
    handle: AppHandle,
    state: State<'_, AppState>,
    mod_id: String,
) -> Result<ModDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.readopt_mod(&mod_id, &sink)).await
}

/// Stages a ZIP archive for import and previews the mapping (emits `gmm://progress`).
#[tauri::command]
pub async fn stage_import(
    handle: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
    zip_path: String,
) -> Result<ImportPreviewDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| {
        app.stage_import(&installation_id, &zip_path, &sink)
    })
    .await
}

/// Recomputes the mapping of the staged archive for another root folder.
#[tauri::command]
pub async fn remap_import(
    state: State<'_, AppState>,
    staging_id: String,
    root: Option<String>,
) -> Result<ImportPreviewDto, AppError> {
    run_blocking(&state, move |app| {
        app.remap_import(&staging_id, root.as_deref())
    })
    .await
}

/// Imports the staged archive as a disabled mod (optionally enabling it afterwards).
#[tauri::command]
pub async fn confirm_import(
    handle: AppHandle,
    state: State<'_, AppState>,
    req: ConfirmImportRequest,
) -> Result<ModDto, AppError> {
    let sink = progress_sink(handle);
    run_blocking(&state, move |app| app.confirm_import(req, &sink)).await
}

/// Discards the staged archive and deletes its staging folder.
#[tauri::command]
pub async fn discard_import(
    state: State<'_, AppState>,
    staging_id: String,
) -> Result<(), AppError> {
    run_blocking(&state, move |app| app.discard_import(&staging_id)).await
}

/// Records that a mod requires another one.
#[tauri::command]
pub async fn add_dependency(
    state: State<'_, AppState>,
    mod_id: String,
    depends_on_mod_id: String,
) -> Result<ModDetailDto, AppError> {
    run_blocking(&state, move |app| {
        app.add_dependency(&mod_id, &depends_on_mod_id)
    })
    .await
}

/// Removes a dependency between two mods.
#[tauri::command]
pub async fn remove_dependency(
    state: State<'_, AppState>,
    mod_id: String,
    depends_on_mod_id: String,
) -> Result<ModDetailDto, AppError> {
    run_blocking(&state, move |app| {
        app.remove_dependency(&mod_id, &depends_on_mod_id)
    })
    .await
}

/// Dry run of enabling or disabling a mod together with its dependency closure.
#[tauri::command]
pub async fn preview_set_mod_enabled(
    state: State<'_, AppState>,
    mod_id: String,
    enabled: bool,
) -> Result<PlanPreviewDto, AppError> {
    run_blocking(&state, move |app| {
        app.preview_set_mod_enabled(&mod_id, enabled)
    })
    .await
}

/// Reports what the vault holds (objects, unreferenced objects, stray files).
#[tauri::command]
pub async fn vault_report(state: State<'_, AppState>) -> Result<VaultReportDto, AppError> {
    run_blocking(&state, |app| app.vault_report()).await
}

/// Deletes unreferenced vault objects; returns the bytes freed.
#[tauri::command]
pub async fn vault_gc(state: State<'_, AppState>, shas: Vec<String>) -> Result<u64, AppError> {
    run_blocking(&state, move |app| app.vault_gc(&shas)).await
}
