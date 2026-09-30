mod bootstrap;
mod lua;
mod preferred_layout;
mod record;

use anyhow::{Result, anyhow};

use super::Config;
use super::defaults::{self, DefaultValues};
use crate::core::TilingConfig;

/// The matcher lists stay empty, so a fixture manages every window it inserts.
pub(crate) fn tiling_config() -> TilingConfig {
    bundled().tiling
}

fn bundled() -> DefaultValues {
    defaults::bundled().expect("the bundled defaults should read")
}

fn config_from(src: &str) -> Config {
    Config::from_lua_src("test config", src).expect("config should load")
}

fn try_config(src: &str) -> Result<Config> {
    Config::from_lua_src("test config", src).map_err(|e| anyhow!("{e}"))
}

pub(crate) struct CleanupFile(pub(crate) std::path::PathBuf);
impl Drop for CleanupFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

pub(crate) fn temp_lua_path(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("dome_{tag}_{nanos}.lua"))
}
