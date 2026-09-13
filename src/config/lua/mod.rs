mod deserializer;
mod globals;

pub(crate) use globals::new_vm;

/// Evaluates `src` and deserializes the value it returns into `T`. A Lua
/// function anywhere in that value is pushed onto `callbacks` and deserializes
/// as its index, so `T` can hold a `CallbackId`.
pub(super) fn evaluate<T: serde::de::DeserializeOwned>(
    lua: &mlua::Lua,
    name: &str,
    src: &str,
    callbacks: &mut Vec<mlua::Function>,
) -> mlua::Result<T> {
    let value: mlua::Value = lua.load(src).set_name(name).eval()?;
    deserializer::from_lua_value(value, callbacks)
}
