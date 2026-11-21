//! System tray: "Prepare for online" and "Restore my mods" for every installation without
//! opening the window, with a notification of the result.
//!
//! Closing the window hides it; the app keeps running in the tray until "Quit ModSwitch".
//! The actions are the same use cases the window runs (same guards, same transactions); the
//! wording of the menu and the notifications comes from `gmm_app::tray`.

use std::sync::atomic::{AtomicBool, Ordering};

use gmm_app::{
    AppError, InstallationDto, TrayNotice, prepare_notice, restore_notice, tray_entries,
};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::commands::progress_sink;
use crate::state::AppState;

const TRAY_ID: &str = "main";
const MAIN_WINDOW: &str = "main";
/// Tells the UI that a tray action changed data, so it refreshes what it shows.
const DATA_CHANGED_EVENT: &str = "gmm://data-changed";
const OPEN: &str = "open";
const QUIT: &str = "quit";
const PREPARE: &str = "prepare:";
const RESTORE: &str = "restore:";

/// The "still running in the tray" notice is shown once per session.
static HIDE_NOTICE_SHOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum Action {
    Prepare,
    Restore,
}

fn parse_action(id: &str) -> Option<(Action, &str)> {
    if let Some(rest) = id.strip_prefix(PREPARE) {
        Some((Action::Prepare, rest))
    } else {
        id.strip_prefix(RESTORE).map(|rest| (Action::Restore, rest))
    }
}

fn installations(app: &AppHandle) -> Vec<InstallationDto> {
    let Ok(core) = app.state::<AppState>().app() else {
        return Vec::new();
    };
    core.list_installations().unwrap_or_else(|e| {
        tracing::warn!(error = %e.message, "tray: installations unavailable");
        Vec::new()
    })
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        OPEN,
        "Open ModSwitch",
        true,
        None::<&str>,
    )?)?;
    for entry in tray_entries(&installations(app)) {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        // A disabled item works as the heading of the installation's actions.
        menu.append(&MenuItem::new(app, &entry.label, false, None::<&str>)?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("{PREPARE}{}", entry.installation_id),
            "Prepare for online",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("{RESTORE}{}", entry.installation_id),
            "Restore my mods",
            true,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUIT,
        "Quit ModSwitch",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

/// Creates the tray icon. Called once from `setup`.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("ModSwitch")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Rebuilds the menu after the list of installations changed (registered or renamed).
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build_menu(app) {
        Ok(menu) => {
            if let Err(e) = tray.set_menu(Some(menu)) {
                tracing::warn!(error = %e, "tray: menu not updated");
            }
        }
        Err(e) => tracing::warn!(error = %e, "tray: menu not built"),
    }
}

/// Shows, restores and focuses the main window.
pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let shown = window
        .show()
        .and_then(|()| window.unminimize())
        .and_then(|()| window.set_focus());
    if let Err(e) = shown {
        tracing::warn!(error = %e, "main window could not be shown");
    }
}

/// Closing the main window hides it to the tray instead of quitting.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(e) = window.hide() {
            tracing::warn!(error = %e, "main window could not be hidden");
            return;
        }
        if !HIDE_NOTICE_SHOWN.swap(true, Ordering::Relaxed) {
            notify(
                window.app_handle(),
                &TrayNotice {
                    title: "ModSwitch is still running".into(),
                    body: "It stays in the tray, next to the clock. Right-click the icon to prepare for online, restore your mods or quit.".into(),
                },
            );
        }
    }
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        OPEN => show_main_window(app),
        QUIT => app.exit(0),
        _ => {
            if let Some((action, installation_id)) = parse_action(id) {
                run_action(app, action, installation_id.to_owned());
            }
        }
    }
}

/// Runs the action on the blocking pool: it can take a while (scan, transaction).
fn run_action(app: &AppHandle, action: Action, installation_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let core = match app.state::<AppState>().app() {
            Ok(core) => core,
            Err(e) => {
                notify(&app, &failure(&e));
                return;
            }
        };
        let inst = match core
            .list_installations()
            .map(|list| list.into_iter().find(|i| i.id == installation_id))
        {
            Ok(Some(inst)) => inst,
            Ok(None) => {
                refresh(&app);
                return;
            }
            Err(e) => {
                notify(&app, &failure(&e));
                return;
            }
        };
        let sink = progress_sink(app.clone());
        let notice = match action {
            Action::Prepare => prepare_notice(&inst, &core.prepare_for_online(&inst.id, &sink)),
            Action::Restore => restore_notice(&inst, &core.restore_from_vanilla(&inst.id, &sink)),
        };
        notify(&app, &notice);
        if let Err(e) = app.emit(DATA_CHANGED_EVENT, &inst.id) {
            tracing::debug!(error = %e, "data-changed event not delivered");
        }
    });
}

fn failure(e: &AppError) -> TrayNotice {
    TrayNotice {
        title: "ModSwitch could not do it".into(),
        body: e.message.clone(),
    }
}

fn notify(app: &AppHandle, notice: &TrayNotice) {
    let shown = app
        .notification()
        .builder()
        .title(&notice.title)
        .body(&notice.body)
        .show();
    if let Err(e) = shown {
        tracing::warn!(error = %e, title = %notice.title, "notification not shown");
    }
}
