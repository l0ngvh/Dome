use std::path::PathBuf;

pub struct ConfigDirectory(tempfile::TempDir);

impl ConfigDirectory {
    pub fn create() -> Self {
        Self(tempfile::tempdir().expect("the test directory should be created"))
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.path().join(name)
    }

    pub fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, source).expect("the test file should be written");
        path
    }
}
