//! Loads a `layout.lua` file into the preferred layouts it describes.

use anyhow::anyhow;

use crate::core::PreferredLayouts;

use super::lua;

impl PreferredLayouts {
    pub(crate) fn load(path: &str) -> anyhow::Result<Self> {
        let src = std::fs::read_to_string(path)?;
        Self::from_lua(path, &src)
    }

    pub(crate) fn load_or_default(path: &str) -> Self {
        match Self::load(path) {
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
    pub(crate) fn from_lua(path: &str, src: &str) -> anyhow::Result<Self> {
        let vm = mlua::Lua::new();
        lua::evaluate(&vm, path, src).map_err(|e| anyhow!("{path}: {e}"))
    }
}
