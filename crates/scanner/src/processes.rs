//! Processes that must not be running while the game folder is modified.

use crate::os::OsProbe;

/// Launcher and game processes that block any modification of the installation.
/// ASSUMPTION A-07: verify on real installations in M7.
pub const GUARDED_PROCESSES: &[&str] = &[
    "GTA5.exe",
    "GTA5_Enhanced.exe",
    "PlayGTAV.exe",
    "GTAVLauncher.exe",
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
    fn nothing_running() {
        assert!(running_guarded(&FakeOs::new()).is_empty());
        assert!(running_guarded(&FakeOs::new().processes(&["notepad.exe"])).is_empty());
    }
}
