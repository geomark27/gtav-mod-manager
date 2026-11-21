//! Processes that must not be running while the game folder is modified.

use crate::os::OsProbe;

/// Launcher and game processes that block any modification of an installation.
///
/// One list for every game: the Rockstar launcher and services are shared, and a running game
/// of either title blocks changes to both (conservative, until the guard is per game).
/// ASSUMPTIONS A-07 (GTA V) and A-10 (Red Dead Redemption 2): verify on real installations.
pub const GUARDED_PROCESSES: &[&str] = &[
    "GTA5.exe",
    "GTA5_Enhanced.exe",
    "PlayGTAV.exe",
    "GTAVLauncher.exe",
    "RDR2.exe",
    "PlayRDR2.exe",
    "Launcher.exe",
    "LauncherPatcher.exe",
    "RockstarService.exe",
    "SocialClubHelper.exe",
];

/// Running guarded processes, as listed in [`GUARDED_PROCESSES`] (canonical spelling),
/// matched case-insensitively, sorted and deduplicated.
pub fn running_guarded(os: &dyn OsProbe) -> Vec<String> {
    let running: Vec<String> = os
        .running_process_names()
        .iter()
        .map(|n| n.to_lowercase())
        .collect();
    let mut out: Vec<String> = GUARDED_PROCESSES
        .iter()
        .filter(|g| running.contains(&g.to_lowercase()))
        .map(|g| (*g).to_owned())
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
            running_guarded(&os),
            ["GTA5.exe", "Launcher.exe", "PlayGTAV.exe"]
        );
    }

    #[test]
    fn red_dead_redemption_2_is_guarded() {
        let os = FakeOs::new().processes(&["rdr2.exe", "PlayRDR2.exe"]);
        assert_eq!(running_guarded(&os), ["PlayRDR2.exe", "RDR2.exe"]);
    }

    #[test]
    fn nothing_running() {
        assert!(running_guarded(&FakeOs::new()).is_empty());
        assert!(running_guarded(&FakeOs::new().processes(&["notepad.exe"])).is_empty());
    }
}
