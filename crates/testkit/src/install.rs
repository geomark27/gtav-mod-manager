//! Fake game installations living in a temp dir.

use crate::error::{Error, Result};
use crate::fixtures::{base_files, fake_bytes};
use crate::snapshot::Snapshot;
use gmm_domain::{Edition, RelPath};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A fake installation; its temp dir is deleted on drop.
#[derive(Debug)]
pub struct FakeInstall {
    dir: TempDir,
    edition: Edition,
}

impl FakeInstall {
    /// Starts building an installation of `edition`.
    pub fn builder(edition: Edition) -> FakeInstallBuilder {
        FakeInstallBuilder {
            edition,
            steps: Vec::new(),
            omitted: Vec::new(),
        }
    }

    /// Root directory of the installation.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Edition of this installation.
    pub fn edition(&self) -> Edition {
        self.edition
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let rel = RelPath::parse(rel)?;
        Ok(self.root().join(rel.as_str()))
    }

    /// Absolute path of `rel`. An invalid `rel` yields a path that cannot exist
    /// (`<root>/<invalid-rel-path>`), never a path outside the root.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.resolve(rel)
            .unwrap_or_else(|_| self.root().join("<invalid-rel-path>"))
    }

    /// Writes a file, creating parent directories and overwriting.
    pub fn write(&self, rel: &str, data: &[u8]) -> Result<()> {
        write_file(&self.resolve(rel)?, data)
    }

    /// Reads a file.
    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let p = self.resolve(rel)?;
        std::fs::read(&p).map_err(|e| Error::io("read", &p, e))
    }

    /// Removes a file or an (even non-empty) directory.
    pub fn remove(&self, rel: &str) -> Result<()> {
        let p = self.resolve(rel)?;
        let meta = std::fs::symlink_metadata(&p).map_err(|e| Error::io("stat", &p, e))?;
        let res = if meta.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
        res.map_err(|e| Error::io("remove", &p, e))
    }

    /// True when `rel` is valid and exists.
    pub fn exists(&self, rel: &str) -> bool {
        self.resolve(rel).is_ok_and(|p| p.exists())
    }

    /// Snapshot of the whole installation.
    pub fn snapshot(&self) -> Result<Snapshot> {
        Snapshot::of(self.root())
    }
}

fn write_file(p: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io("create_dir", parent, e))?;
    }
    std::fs::write(p, data).map_err(|e| Error::io("write", p, e))
}

#[derive(Debug)]
enum Step {
    BaseFiles,
    File(String, Vec<u8>),
    Dir(String),
}

/// Builder of [`FakeInstall`]; steps are applied in order.
#[derive(Debug)]
pub struct FakeInstallBuilder {
    edition: Edition,
    steps: Vec<Step>,
    omitted: Vec<String>,
}

impl FakeInstallBuilder {
    /// Adds every base file with content `fake_bytes(path, 4096)`.
    pub fn with_base_files(mut self) -> Self {
        self.steps.push(Step::BaseFiles);
        self
    }

    /// Omits a base file (for incomplete installations); only filters base files.
    pub fn without(mut self, rel: &str) -> Self {
        self.omitted.push(rel.to_lowercase());
        self
    }

    /// Adds a file with the given content.
    pub fn file(mut self, rel: &str, data: impl Into<Vec<u8>>) -> Self {
        self.steps.push(Step::File(rel.to_owned(), data.into()));
        self
    }

    /// Adds an empty directory.
    pub fn dir(mut self, rel: &str) -> Self {
        self.steps.push(Step::Dir(rel.to_owned()));
        self
    }

    fn seeded(self, rel: String) -> Self {
        let data = fake_bytes(&rel, 256);
        self.file(&rel, data)
    }

    /// Adds `<name>.asi` and `<name>.ini`.
    pub fn asi_mod(self, name: &str) -> Self {
        self.seeded(format!("{name}.asi"))
            .seeded(format!("{name}.ini"))
    }

    /// Adds `ScriptHookV.dll` and `dinput8.dll`.
    pub fn script_hook(self) -> Self {
        self.seeded("ScriptHookV.dll".to_owned())
            .seeded("dinput8.dll".to_owned())
    }

    /// Adds `scripts/<name>.dll` and `scripts/<name>.ini`.
    pub fn scripts_mod(self, name: &str) -> Self {
        self.seeded(format!("scripts/{name}.dll"))
            .seeded(format!("scripts/{name}.ini"))
    }

    /// Creates the temp dir and applies the steps in order.
    pub fn build(self) -> Result<FakeInstall> {
        let dir = tempfile::tempdir().map_err(|e| Error::io("tempdir", Path::new(""), e))?;
        let install = FakeInstall {
            dir,
            edition: self.edition,
        };
        for step in &self.steps {
            match step {
                Step::BaseFiles => {
                    for rel in base_files(self.edition) {
                        if self.omitted.contains(&rel.to_lowercase()) {
                            continue;
                        }
                        install.write(rel, &fake_bytes(rel, 4096))?;
                    }
                }
                Step::File(rel, data) => install.write(rel, data)?,
                Step::Dir(rel) => {
                    let p = install.resolve(rel)?;
                    std::fs::create_dir_all(&p).map_err(|e| Error::io("create_dir", &p, e))?;
                }
            }
        }
        Ok(install)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Change, Entry};

    fn legacy() -> Result<FakeInstall> {
        FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .build()
    }

    #[test]
    fn every_edition_has_base_files() -> Result<()> {
        for edition in Edition::ALL {
            let i = FakeInstall::builder(edition).with_base_files().build()?;
            assert_eq!(i.edition(), edition);
            assert!(i.exists(edition.executable_name()));
            for f in base_files(edition) {
                assert!(i.exists(f), "{f}");
            }
            let rpfs: Vec<&&str> = base_files(edition)
                .iter()
                .filter(|f| f.ends_with(".rpf"))
                .collect();
            assert!(!rpfs.is_empty(), "{edition} has no archive");
            for rpf in rpfs {
                assert_eq!(i.read(rpf)?.len(), 4096);
            }
        }
        Ok(())
    }

    #[test]
    fn editions_are_distinguished_by_executable() -> Result<()> {
        let l = legacy()?;
        assert!(!l.exists("GTA5_Enhanced.exe"));
        let e = FakeInstall::builder(Edition::Enhanced)
            .with_base_files()
            .build()?;
        assert!(!e.exists("GTA5.exe"));
        Ok(())
    }

    #[test]
    fn without_omits_base_file() -> Result<()> {
        let i = FakeInstall::builder(Edition::Legacy)
            .with_base_files()
            .without("x64a.rpf")
            .build()?;
        assert!(!i.exists("x64a.rpf"));
        assert!(i.exists("common.rpf"));
        Ok(())
    }

    #[test]
    fn mod_helpers() -> Result<()> {
        let i = FakeInstall::builder(Edition::Legacy)
            .asi_mod("Trainer")
            .script_hook()
            .scripts_mod("Foo")
            .dir("mods/empty")
            .file("mods/a.txt", b"hi".to_vec())
            .build()?;
        for f in [
            "Trainer.asi",
            "Trainer.ini",
            "ScriptHookV.dll",
            "dinput8.dll",
            "scripts/Foo.dll",
            "scripts/Foo.ini",
            "mods/a.txt",
        ] {
            assert!(i.exists(f), "{f}");
        }
        assert_eq!(i.snapshot()?.entries().get("mods/empty"), Some(&Entry::Dir));
        assert_eq!(i.read("mods/a.txt")?, b"hi");
        Ok(())
    }

    #[test]
    fn invalid_paths_rejected() -> Result<()> {
        let i = legacy()?;
        assert!(matches!(i.write("../x", b"x"), Err(Error::InvalidPath(_))));
        assert!(matches!(i.read("/abs"), Err(Error::InvalidPath(_))));
        assert!(!i.exists("..\\x"));
        assert!(i.path("..").starts_with(i.root()));
        assert!(
            FakeInstall::builder(Edition::Legacy)
                .file("C:\\x", b"x".to_vec())
                .build()
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn identical_builds_have_equal_snapshots() -> Result<()> {
        assert_eq!(legacy()?.snapshot()?, legacy()?.snapshot()?);
        Ok(())
    }

    #[test]
    fn write_remove_modify_are_diffed_exactly() -> Result<()> {
        let i = legacy()?;
        let before = i.snapshot()?;
        i.write("new/dir/a.asi", b"a")?;
        i.write("common.rpf", b"changed")?;
        i.remove("x64a.rpf")?;
        let changes = before.diff(&i.snapshot()?);
        assert_eq!(
            changes,
            vec![
                Change::Modified("common.rpf".into()),
                Change::Added("new".into()),
                Change::Added("new/dir".into()),
                Change::Added("new/dir/a.asi".into()),
                Change::Removed("x64a.rpf".into()),
            ]
        );
        assert!(before.diff(&before).is_empty());
        Ok(())
    }

    #[test]
    fn remove_directory() -> Result<()> {
        let i = legacy()?;
        i.remove("update")?;
        assert!(!i.exists("update/update.rpf"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_fails_snapshot() -> Result<()> {
        let i = legacy()?;
        std::os::unix::fs::symlink(i.path("common.rpf"), i.path("link"))
            .map_err(|e| Error::io("symlink", i.root(), e))?;
        assert!(matches!(
            i.snapshot(),
            Err(Error::SymlinkInSnapshot(p)) if p == "link"
        ));
        Ok(())
    }

    #[test]
    fn temp_dir_deleted_on_drop() -> Result<()> {
        let i = legacy()?;
        let root = i.root().to_path_buf();
        assert!(root.exists());
        drop(i);
        assert!(!root.exists());
        Ok(())
    }
}
