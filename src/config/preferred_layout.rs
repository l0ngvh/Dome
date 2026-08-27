//! Loads a `layout.lua` file into the preferred layouts it describes.

use anyhow::anyhow;

use crate::core::PreferredLayouts;

use super::lua;

impl PreferredLayouts {
    pub(crate) fn load(path: &str) -> anyhow::Result<Self> {
        let src = std::fs::read_to_string(path)?;
        Self::from_lua(path, &src)
    }

    // No field of a layout file holds a function. The VM needs no `dome` global
    // either.
    pub(super) fn from_lua(path: &str, src: &str) -> anyhow::Result<Self> {
        let vm = mlua::Lua::new();
        lua::evaluate(&vm, path, src).map_err(|e| anyhow!("{path}: {e}"))
    }
}
