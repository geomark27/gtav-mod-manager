//! Fake [`OsProbe`] for tests (also used by other crates' tests).

use super::{Hive, OsProbe};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// In-memory OS probe. Registry key and value names are case-insensitive, like Windows.
#[derive(Debug, Clone, Default)]
pub struct FakeOs {
    registry: HashMap<(Hive, String, String), String>,
    versions: HashMap<PathBuf, String>,
    publishers: HashMap<PathBuf, String>,
    processes: Vec<String>,
}

fn norm(s: &str) -> String {
    s.to_lowercase().replace('/', "\\")
}

impl FakeOs {
    /// An empty OS: no registry values, no versions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a registry string value.
    pub fn registry(mut self, hive: Hive, key: &str, value: &str, data: &str) -> Self {
        self.registry
            .insert((hive, norm(key), norm(value)), data.to_owned());
        self
    }

    /// Sets the version reported for a file.
    pub fn version(mut self, path: impl AsRef<Path>, version: &str) -> Self {
        self.versions
            .insert(path.as_ref().to_path_buf(), version.to_owned());
        self
    }

    /// Sets the Authenticode publisher reported for a file.
    pub fn publisher(mut self, path: impl AsRef<Path>, publisher: &str) -> Self {
        self.publishers
            .insert(path.as_ref().to_path_buf(), publisher.to_owned());
        self
    }

    /// Sets the running process names.
    pub fn processes(mut self, names: &[&str]) -> Self {
        self.processes = names.iter().map(|n| (*n).to_owned()).collect();
        self
    }
}

impl OsProbe for FakeOs {
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        self.registry.get(&(hive, norm(key), norm(value))).cloned()
    }

    fn file_version(&self, path: &Path) -> Option<String> {
        self.versions.get(path).cloned()
    }

    fn authenticode_publisher(&self, path: &Path) -> Option<String> {
        self.publishers.get(path).cloned()
    }

    fn running_process_names(&self) -> Vec<String> {
        self.processes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_case_insensitive_and_hive_aware() {
        let os = FakeOs::new().registry(
            Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
            "C:/s",
        );
        assert_eq!(
            os.registry_string(Hive::CurrentUser, "SOFTWARE\\valve\\steam", "steampath"),
            Some("C:/s".to_owned())
        );
        assert_eq!(
            os.registry_string(Hive::LocalMachine, "Software\\Valve\\Steam", "SteamPath"),
            None
        );
    }

    #[test]
    fn publishers_and_processes() {
        let os = FakeOs::new()
            .publisher("/g/a.dll", "Acme")
            .processes(&["a.exe", "b.exe"]);
        assert_eq!(
            os.authenticode_publisher(Path::new("/g/a.dll")).as_deref(),
            Some("Acme")
        );
        assert_eq!(os.authenticode_publisher(Path::new("/g/b.dll")), None);
        assert_eq!(os.running_process_names(), ["a.exe", "b.exe"]);
    }

    #[test]
    fn versions_by_path() {
        let os = FakeOs::new().version("/g/GTA5.exe", "1.0.3095.0");
        assert_eq!(
            os.file_version(Path::new("/g/GTA5.exe")).as_deref(),
            Some("1.0.3095.0")
        );
        assert_eq!(os.file_version(Path::new("/g/other.exe")), None);
    }
}
