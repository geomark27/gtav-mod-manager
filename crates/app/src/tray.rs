//! What the system tray shows and says. The tray itself lives in the Tauri shell; this module
//! holds the parts that can be tested without a window: the menu entries and the wording of
//! the notifications after "prepare for online" and "restore my mods".
//!
//! Like the rest of the app, a notification never states that playing online is safe.

use gmm_domain::Game;

use crate::error::AppError;
use crate::installations::InstallationDto;
use crate::online::{CheckStatus, OnlineCheckDto, ReadinessVerdict};
use crate::vanilla::VanillaStatusDto;

/// One installation in the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayEntry {
    /// Installation id (UUID).
    pub installation_id: String,
    /// Name shown above its actions.
    pub label: String,
}

/// A desktop notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayNotice {
    /// First line.
    pub title: String,
    /// Detail.
    pub body: String,
}

/// Name of a game's online mode.
pub fn online_name(game: Game) -> &'static str {
    match game {
        Game::Gtav => "GTA Online",
        Game::Rdr2 => "Red Dead Online",
    }
}

/// Menu entries: installations grouped by game (GTA V first), each group in list order.
pub fn tray_entries(installations: &[InstallationDto]) -> Vec<TrayEntry> {
    Game::ALL
        .iter()
        .flat_map(|game| installations.iter().filter(move |i| i.game == *game))
        .map(|i| TrayEntry {
            installation_id: i.id.clone(),
            label: i.display_name.clone(),
        })
        .collect()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn error_body(e: &AppError) -> String {
    match &e.hint {
        Some(hint) => format!("{} {hint}", e.message),
        None => e.message.clone(),
    }
}

/// Notification after "prepare for online".
pub fn prepare_notice(
    inst: &InstallationDto,
    result: &Result<OnlineCheckDto, AppError>,
) -> TrayNotice {
    let name = &inst.display_name;
    let online = online_name(inst.game);
    let r = match result {
        Ok(r) => r,
        Err(e) => {
            return TrayNotice {
                title: format!("{name}: could not prepare for online"),
                body: error_body(e),
            };
        }
    };
    if let Some(e) = &r.action_error {
        return TrayNotice {
            title: format!("{name}: mods not disabled"),
            body: format!("{} Open ModSwitch to see the checks.", error_body(e)),
        };
    }
    let mods = if r.vanilla_applied {
        "Your mods are disabled."
    } else {
        "No managed mod was enabled."
    };
    let count = |s: CheckStatus| r.checks.iter().filter(|c| c.status == s).count();
    match r.verdict {
        ReadinessVerdict::Clear => TrayNotice {
            title: format!("{name}: no mod files found"),
            body: format!(
                "{mods} Based on these checks only. This is not a guarantee for {online}."
            ),
        },
        ReadinessVerdict::Review => TrayNotice {
            title: format!(
                "{name}: {} to review",
                plural(count(CheckStatus::Warn), "thing", "things")
            ),
            body: format!("{mods} Open ModSwitch to look at them before you play online."),
        },
        ReadinessVerdict::Blocked => TrayNotice {
            title: format!(
                "{name}: {} to fix",
                plural(count(CheckStatus::Fail), "thing", "things")
            ),
            body: format!("{mods} Open ModSwitch to see what is left before you open {online}."),
        },
    }
}

/// Notification after "restore my mods".
pub fn restore_notice(
    inst: &InstallationDto,
    result: &Result<VanillaStatusDto, AppError>,
) -> TrayNotice {
    let name = &inst.display_name;
    match result {
        Ok(s) => TrayNotice {
            title: format!("{name}: mods restored"),
            body: format!(
                "{} in the game folder.",
                plural(
                    usize::try_from(s.mods_in_game).unwrap_or(usize::MAX),
                    "mod is back",
                    "mods are back"
                )
            ),
        },
        Err(e) => TrayNotice {
            title: format!("{name}: mods not restored"),
            body: error_body(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::online::{ReadinessCheckDto, ReadinessCheckId};
    use gmm_domain::{BaselineTrust, DistributionPlatform, Edition, InstallationStatus};

    fn inst(id: &str, edition: Edition, name: &str) -> InstallationDto {
        InstallationDto {
            id: id.into(),
            game: edition.game(),
            edition,
            platform: DistributionPlatform::Steam,
            display_name: name.into(),
            root_path: "/games".into(),
            exe_version: None,
            status: InstallationStatus::Ok,
            baseline: BaselineTrust::Trusted,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn check(id: ReadinessCheckId, status: CheckStatus) -> ReadinessCheckDto {
        ReadinessCheckDto {
            id,
            status,
            count: u64::from(status != CheckStatus::Pass),
            items: Vec::new(),
        }
    }

    fn result(verdict: ReadinessVerdict, checks: Vec<ReadinessCheckDto>) -> OnlineCheckDto {
        OnlineCheckDto {
            game: Game::Rdr2,
            checked_at: String::new(),
            scanned_at: None,
            verdict,
            checks,
            baseline: BaselineTrust::Trusted,
            vanilla_applied: true,
            action_error: None,
        }
    }

    #[test]
    fn entries_put_gta_v_first() {
        let list = [
            inst("r", Edition::Rdr2, "RDR2"),
            inst("e", Edition::Enhanced, "Enhanced"),
            inst("l", Edition::Legacy, "Legacy"),
        ];
        let labels: Vec<String> = tray_entries(&list).into_iter().map(|e| e.label).collect();
        assert_eq!(labels, ["Enhanced", "Legacy", "RDR2"]);
        assert!(tray_entries(&[]).is_empty());
    }

    #[test]
    fn a_clear_result_is_never_called_safe() {
        let red = inst("r", Edition::Rdr2, "RDR2 (Steam)");
        let n = prepare_notice(&red, &Ok(result(ReadinessVerdict::Clear, Vec::new())));
        assert_eq!(n.title, "RDR2 (Steam): no mod files found");
        assert_eq!(
            n.body,
            "Your mods are disabled. Based on these checks only. This is not a guarantee for Red Dead Online."
        );
        assert!(!n.title.to_lowercase().contains("safe"));
        assert!(!n.body.to_lowercase().contains("safe"));
    }

    #[test]
    fn review_and_blocked_count_the_checks() {
        let gta = inst("g", Edition::Legacy, "GTA V");
        let review = result(
            ReadinessVerdict::Review,
            vec![check(ReadinessCheckId::UnknownFiles, CheckStatus::Warn)],
        );
        assert_eq!(
            prepare_notice(&gta, &Ok(review)).title,
            "GTA V: 1 thing to review"
        );
        let mut blocked = result(
            ReadinessVerdict::Blocked,
            vec![
                check(ReadinessCheckId::ModFiles, CheckStatus::Fail),
                check(ReadinessCheckId::GameClosed, CheckStatus::Fail),
                check(ReadinessCheckId::Leftovers, CheckStatus::Warn),
            ],
        );
        blocked.vanilla_applied = false;
        let n = prepare_notice(&gta, &Ok(blocked));
        assert_eq!(n.title, "GTA V: 2 things to fix");
        assert_eq!(
            n.body,
            "No managed mod was enabled. Open ModSwitch to see what is left before you open GTA Online."
        );
    }

    #[test]
    fn errors_name_the_reason_and_the_hint() {
        let gta = inst("g", Edition::Legacy, "GTA V");
        let running = AppError::new(ErrorCode::GameRunning, "GTA V is running.")
            .with_hint("Close it, then try again.");
        let mut r = result(ReadinessVerdict::Blocked, Vec::new());
        r.action_error = Some(running.clone());
        let n = prepare_notice(&gta, &Ok(r));
        assert_eq!(n.title, "GTA V: mods not disabled");
        assert_eq!(
            n.body,
            "GTA V is running. Close it, then try again. Open ModSwitch to see the checks."
        );
        let n = prepare_notice(&gta, &Err(running.clone()));
        assert_eq!(n.title, "GTA V: could not prepare for online");
        let n = restore_notice(&gta, &Err(running));
        assert_eq!(n.title, "GTA V: mods not restored");
        assert_eq!(n.body, "GTA V is running. Close it, then try again.");
    }

    #[test]
    fn restore_counts_the_mods_back() {
        let gta = inst("g", Edition::Legacy, "GTA V");
        let status = |n: u64| VanillaStatusDto {
            active: false,
            restore_point: None,
            mods_total: 3,
            mods_in_game: n,
            unmanaged_files: None,
        };
        assert_eq!(
            restore_notice(&gta, &Ok(status(3))).body,
            "3 mods are back in the game folder."
        );
        assert_eq!(
            restore_notice(&gta, &Ok(status(1))).body,
            "1 mod is back in the game folder."
        );
    }
}
