//! Loads a `layout.lua` file into the preferred layouts it describes.

use anyhow::anyhow;

use crate::core::PreferredLayouts;

use super::lua;

impl PreferredLayouts {
    pub fn load_or_default(path: &str, source: anyhow::Result<String>) -> Self {
        match source.and_then(|source| Self::from_lua(path, &source)) {
            Ok(layouts) => layouts,
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
            {
                tracing::info!(%path, "File not found, using defaults");
                Self::default()
            }
            Err(e) => {
                tracing::warn!(%path, error = %format!("{e:#}"), "Failed to load, using defaults");
                Self::default()
            }
        }
    }

    // No field of a layout file holds a function. The VM needs no `dome` global
    // either.
    pub fn from_lua(path: &str, src: &str) -> anyhow::Result<Self> {
        let vm = mlua::Lua::new();
        lua::evaluate(&vm, path, src).map_err(|e| anyhow!("{path}: {e}"))
    }
}
