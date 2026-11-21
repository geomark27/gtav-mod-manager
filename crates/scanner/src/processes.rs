//! Processes that must not be running while the game folder is modified.

use crate::os::OsProbe;
use gmm_domain::Game;

/// Rockstar Games Launcher and services: shared by every game, so they block every game.
/// ASSUMPTION A-07: verify on real installations.
pub const ROCKSTAR_PROCESSES: &[&str] = &[
    "Launcher.exe",
    "LauncherPatcher.exe",
    "RockstarService.exe",
    "SocialClubHelper.exe",
];

/// GTA V and its own launchers (both editions). ASSUMPTION A-07.
pub const GTAV_PROCESSES: &[&str] = &[
    "GTA5.exe",
    "GTA5_Enhanced.exe",
    "PlayGTAV.exe",
    "GTAVLauncher.exe",
];

/// Red Dead Redemption 2 and its launcher. ASSUMPTION A-10.
pub const RDR2_PROCESSES: &[&str] = &["RDR2.exe", "PlayRDR2.exe"];

/// Processes that block any modification of an installation of `game`: the game's own and
/// the shared Rockstar ones. A running game never blocks changes to the other game.
pub fn guarded_processes(game: Game) -> impl Iterator<Item = &'static str> {
    let own = match game {
        Game::Gtav => GTAV_PROCESSES,
        Game::Rdr2 => RDR2_PROCESSES,
    };
    own.iter().chain(ROCKSTAR_PROCESSES).copied()
}

/// Running processes that block changes to `game` (see [`guarded_processes`]), in their
/// canonical spelling, matched case-insensitively, sorted and deduplicated.
pub fn running_guarded(os: &dyn OsProbe, game: Game) -> Vec<String> {
    let running: Vec<String> = os
        .running_process_names()
        .iter()
        .map(|n| n.to_lowercase())
        .collect();
    let mut out: Vec<String> = guarded_processes(game)
        .filter(|g| running.contains(&g.to_lowercase()))
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::testing::FakeOs;

    #[test]
    fn matches_case_insensitively_sorted_and_deduplicated() {
        let os = FakeOs::new().processes(&[
            "explorer.exe",
            "gta5.EXE",
            "GTA5.exe",
            "launcher.exe",
            "playgtav.exe",
        ]);
        assert_eq!(
            running_guarded(&os, Game::Gtav),
            ["GTA5.exe", "Launcher.exe", "PlayGTAV.exe"]
        );
    }

    #[test]
    fn a_running_game_only_blocks_itself() {
        let os = FakeOs::new().processes(&["rdr2.exe", "PlayRDR2.exe"]);
        assert_eq!(
            running_guarded(&os, Game::Rdr2),
            ["PlayRDR2.exe", "RDR2.exe"]
        );
        assert!(running_guarded(&os, Game::Gtav).is_empty());
        let os = FakeOs::new().processes(&["GTA5_Enhanced.exe"]);
        assert_eq!(running_guarded(&os, Game::Gtav), ["GTA5_Enhanced.exe"]);
        assert!(running_guarded(&os, Game::Rdr2).is_empty());
    }

    #[test]
    fn the_rockstar_launcher_blocks_every_game() {
        let os = FakeOs::new().processes(&["RockstarService.exe"]);
        for game in Game::ALL {
            assert_eq!(
                running_guarded(&os, game),
                ["RockstarService.exe"],
                "{game}"
            );
        }
    }

    #[test]
    fn nothing_running() {
        for game in Game::ALL {
            assert!(running_guarded(&FakeOs::new(), game).is_empty());
            let os = FakeOs::new().processes(&["notepad.exe"]);
            assert!(running_guarded(&os, game).is_empty());
        }
    }
}
