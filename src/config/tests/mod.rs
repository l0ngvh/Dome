mod bootstrap;
mod layout;
mod lua;
mod record;
mod watch;

use anyhow::{Result, anyhow};

use super::Config;

fn config_from(src: &str) -> Config {
    Config::from_lua_src("test config", src).expect("config should load")
}

fn try_config(src: &str) -> Result<Config> {
    Config::from_lua_src("test config", src).map_err(|e| anyhow!("{e}"))
}

struct CleanupFile(std::path::PathBuf);
impl Drop for CleanupFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

fn temp_lua_path(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("dome_{tag}_{nanos}.lua"))
}
