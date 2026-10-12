use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) struct ConfigDirectory(PathBuf);

impl ConfigDirectory {
    pub(crate) fn create() -> Self {
        // Parallel tests share the process id and can start within one clock tick.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("dome_test_{}_{index}", std::process::id()));
        // A later process can reuse the id of a killed run that left files behind.
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir(&path).expect("the test config directory should be created");
        Self(path)
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub(crate) fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, source).expect("the test config file should be written");
        path
    }
}

impl Drop for ConfigDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}
