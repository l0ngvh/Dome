//! The default config values, read from the bundled Lua source.

use super::overrides::ConfigOverrides;
use super::{Appearance, LogLevel, ModalKeymaps, lua};
use crate::core::TilingConfig;

pub(super) const BUNDLED_SOURCE: &str = "return dome.defaults()";

#[derive(Debug)]
pub(crate) struct DefaultValues {
    pub(crate) tiling: TilingConfig,
    pub(crate) appearance: Appearance,
    pub(crate) log_level: LogLevel,
    pub(crate) start_at_login: bool,
}

pub(crate) fn bundled() -> mlua::Result<DefaultValues> {
    read(BUNDLED_SOURCE)
}

/// Reads on the caller's VM, because a binding is an `mlua::Function` that dies
/// with the VM that built it.
pub(crate) fn bundled_keymaps(
    lua: &mlua::Lua,
    cx: &mut lua::deserializer::LoadContext,
) -> mlua::Result<ModalKeymaps> {
    let overrides: ConfigOverrides = lua::evaluate_with(lua, "default.lua", BUNDLED_SOURCE, cx)?;
    overrides.into_keymaps()
}

fn read(src: &str) -> mlua::Result<DefaultValues> {
    let lua = lua::new_vm()?;
    let overrides: ConfigOverrides = lua::evaluate(&lua, "default.lua", src)?;
    overrides.into_defaults()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_default_key_is_an_error() {
        let error = read("local c = dome.defaults() c.border_size = nil return c")
            .expect_err("a missing key should fail the read");
        assert!(error.to_string().contains("border_size"), "{error}");
    }
}
