//! First-launch writes into the config directory.

use std::path::Path;

use super::paths;

pub(super) const STARTER_LUA: &str = include_str!("../../resources/config.starter.lua");

pub(super) const META_LUA: &str = include_str!("../../resources/dome.meta.lua");
pub(super) const LUARC_JSON: &str = include_str!("../../resources/config.luarc.json");

// An explicit `-c <path>` is used as given, even when it is missing. Only the
// default path is bootstrapped.
pub(crate) fn resolve_config_path(explicit: Option<String>) -> String {
    match explicit {
        Some(path) => path,
        None => {
            let path = paths::default_path();
            bootstrap_config(&path);
            write_editor_support(&path);
            path
        }
    }
}

// A write failure leaves Dome on the bundled default, so warn rather than abort.
pub(super) fn bootstrap_config(path: &str) {
    let path = Path::new(path);
    if path.exists() {
        return;
    }
    if let Some(dir) = path.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        tracing::warn!(path = %path.display(), error = %e, "Could not create config directory, using bundled defaults");
        return;
    }
    match std::fs::write(path, STARTER_LUA) {
        Ok(()) => tracing::info!(path = %path.display(), "Wrote starter config on first launch"),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Could not write starter config, using bundled defaults");
        }
    }
}

// dome.meta.lua is rewritten every launch to track the version. A user-edited
// .luarc.json is left in place.
pub(super) fn write_editor_support(config_path: &str) {
    let Some(dir) = Path::new(config_path).parent() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(dir = %dir.display(), error = %e, "Could not create config directory for editor support");
        return;
    }
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
