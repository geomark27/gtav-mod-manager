//! Tauri 2 shell: window, logging, application state and command registration.

mod commands;
mod state;

use std::sync::Arc;

use gmm_app::{App, AppDirs, AppError, init_logging};
use state::AppState;

fn build_state() -> AppState {
    let dirs = match AppDirs::from_system() {
        Ok(dirs) => dirs,
        Err(e) => {
            return AppState {
                app: Err(AppError::from(e)),
                _log_guard: None,
            };
        }
    };
    // Logging needs the logs directory; App::init creates it again idempotently.
    let guard = match std::fs::create_dir_all(&dirs.logs_dir) {
        Ok(()) => init_logging(&dirs.logs_dir).ok(),
        Err(_) => None,
    };
    let app = App::init(dirs).map(Arc::new).inspect_err(|e| {
        tracing::error!(code = ?e.code, message = %e.message, details = ?e.details, "initialization failed");
    });
    AppState {
        app,
        _log_guard: guard,
    }
}

/// Builds and runs the Tauri application.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(build_state())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::discover_installations,
            commands::register_installation,
            commands::list_installations,
            commands::refresh_installation,
            commands::rename_installation,
            commands::select_installation,
            commands::get_selected_installation,
            commands::scan_installation,
            commands::list_inventory,
            commands::inventory_summary,
            commands::capture_baseline,
            commands::cancel_operation,
            commands::operation_status,
            commands::guard_status,
            commands::list_mods,
            commands::get_mod,
            commands::preview_adopt,
            commands::adopt_mod,
            commands::set_mod_enabled,
            commands::preview_set_category,
            commands::set_category_enabled,
            commands::update_mod,
            commands::forget_mod,
            commands::reconcile_installation,
            commands::list_profiles,
            commands::profile_name_suggestions,
            commands::create_profile_from_current,
            commands::get_profile,
            commands::update_profile_entries,
            commands::rename_profile,
            commands::delete_profile,
            commands::preview_apply_profile,
            commands::apply_profile,
            commands::detect_mods,
            commands::adopt_detected,
            commands::check_online_readiness,
            commands::prepare_for_online,
            commands::vanilla_status,
            commands::enter_vanilla,
            commands::restore_from_vanilla,
            commands::list_transactions,
            commands::dashboard,
            commands::restore_mod,
            commands::readopt_mod,
            commands::stage_import,
            commands::remap_import,
            commands::confirm_import,
            commands::discard_import,
            commands::add_dependency,
            commands::remove_dependency,
            commands::preview_set_mod_enabled,
            commands::vault_report,
            commands::vault_gc,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!(error = %e, "tauri runtime failed");
    }
}
