//! Operating-system probes behind a trait so all logic is testable on any platform.

pub mod testing;
#[cfg(windows)]
mod windows_authenticode;
#[cfg(windows)]
mod windows_version;

use std::path::Path;

/// Registry hive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hive {
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
}

/// Read-only OS queries needed by discovery and validation.
pub trait OsProbe: Send + Sync {
    /// Reads a `REG_SZ` value; `None` when the key or value is absent or not a string.
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String>;
    /// `"a.b.c.d"` from the PE version resource; `None` if unavailable.
    fn file_version(&self, path: &Path) -> Option<String>;
    /// Subject of the Authenticode signer of a PE file when its signature is valid;
    /// `None` when unsigned, invalid or unavailable. Default: always `None`.
    fn authenticode_publisher(&self, _path: &Path) -> Option<String> {
        None
    }
    /// Names of the running processes as the OS reports them. Default: none.
    fn running_process_names(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The real OS. On Windows it uses the registry and the version resource; elsewhere every
/// query returns `None`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemOs;

impl OsProbe for SystemOs {
    #[cfg(windows)]
    fn registry_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let predef = match hive {
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
            Hive::CurrentUser => HKEY_CURRENT_USER,
        };
        RegKey::predef(predef)
            .open_subkey(key)
            .ok()?
            .get_value::<String, _>(value)
            .ok()
    }

    #[cfg(not(windows))]
    fn registry_string(&self, _hive: Hive, _key: &str, _value: &str) -> Option<String> {
        None
    }

    #[cfg(windows)]
    fn file_version(&self, path: &Path) -> Option<String> {
        windows_version::file_version(path)
    }

    #[cfg(not(windows))]
    fn file_version(&self, _path: &Path) -> Option<String> {
        None
    }

    #[cfg(windows)]
    fn authenticode_publisher(&self, path: &Path) -> Option<String> {
        windows_authenticode::authenticode_publisher(path)
    }

    #[cfg(not(windows))]
    fn authenticode_publisher(&self, _path: &Path) -> Option<String> {
        None
    }

    fn running_process_names(&self) -> Vec<String> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let Ok(mut system) = System::new() else {
            return Vec::new();
        };
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        system
            .processes()
            .values()
            .map(|p| p.name().to_string_lossy().into_owned())
            .collect()
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn system_os_is_inert_off_windows() {
        let os = SystemOs;
        assert_eq!(os.registry_string(Hive::CurrentUser, "a", "b"), None);
        assert_eq!(os.file_version(Path::new("/nonexistent")), None);
        assert_eq!(os.authenticode_publisher(Path::new("/nonexistent")), None);
    }

    #[test]
    fn system_os_lists_processes() {
        assert!(!SystemOs.running_process_names().is_empty());
    }
}
