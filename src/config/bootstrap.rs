//! First-launch writes into the config directory.

use std::path::Path;

use super::paths;

pub(super) const STARTER_LUA: &str = include_str!("../../resources/config.starter.lua");

pub(super) const META_LUA: &str = include_str!("../../resources/dome.meta.lua");
pub(super) const LUARC_JSON: &str = include_str!("../../resources/config.luarc.json");

// An explicit `-c <path>` is used as given, even when it is missing.
pub(crate) fn resolve_config_path(explicit: Option<String>) -> String {
    match explicit {
        Some(path) => path,
        None => {
            let path = paths::default_path();
            bootstrap_default_path(&path);
            path
        }
    }
}

pub(super) fn bootstrap_default_path(path: &str) {
    let path = Path::new(path);
    let Some(dir) = path.parent() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(dir = %dir.display(), error = %e, "Could not create config directory, using bundled defaults");
        return;
    }
    write_starter_config(path);
    write_editor_support(dir);
}

// dome.meta.lua is rewritten every launch to stay current with the binary. A
// user-edited .luarc.json is left in place.
pub(super) fn write_editor_support(dir: &Path) {
    let meta = dir.join("dome.meta.lua");
    if let Err(e) = std::fs::write(&meta, META_LUA) {
        tracing::warn!(path = %meta.display(), error = %e, "Could not write dome.meta.lua");
    }
    let luarc = dir.join(".luarc.json");
    if !luarc.exists()
        && let Err(e) = std::fs::write(&luarc, LUARC_JSON)
    {
        tracing::warn!(path = %luarc.display(), error = %e, "Could not write .luarc.json");
    }
}

fn write_starter_config(path: &Path) {
    if path.exists() {
        return;
    }
    match std::fs::write(path, STARTER_LUA) {
        Ok(()) => tracing::info!(path = %path.display(), "Wrote starter config on first launch"),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Could not write starter config, using bundled defaults");
        }
    }
}
