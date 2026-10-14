//! Builds the Luau VM and the read-only `dome` global.

use crate::config::overrides::environment_variable_name;
use crate::config::{Keystroke, Modifiers};

use super::caller_error;

const DEFAULT_LUA: &str = include_str!("../../../resources/default.lua");

const MODIFIER_ADD_ERROR: &str =
    "a modifier joins only with another modifier or a key, such as Space or \"h\"";

// A lossy conversion would fold two distinct byte sequences onto one key, so the
// second binding would replace the first.
const KEY_NAME_ERROR: &str = "a key name must be non-empty valid UTF-8";

const DEFAULT_MODIFIER_ERROR: &str = "dome.with_default_modifier accepts only Meta or Alt";

const LOG_VALUE_ERROR: &str = "a logged value must convert to a string through tostring";

// The intern tables live in the named registry rather than in a global, so a
// sandboxed config can neither read them nor replace them.
const MODIFIER_INTERN_TABLE: &str = "dome.interned_modifiers";

const KEYSTROKE_INTERN_TABLE: &str = "dome.interned_keystrokes";

const MODIFIER_NAMES: [(&str, Modifiers); 9] = [
    ("Meta", Modifiers::META),
    ("Alt", Modifiers::ALT),
    ("Ctrl", Modifiers::CTRL),
    ("Shift", Modifiers::SHIFT),
    ("Cmd", Modifiers::META),
    ("Win", Modifiers::META),
    ("Option", Modifiers::ALT),
    ("Opt", Modifiers::ALT),
    ("Control", Modifiers::CTRL),
];

const KEY_NAMES: [(&str, &str); 11] = [
    ("Space", "space"),
    ("Enter", "return"),
    ("Return", "return"),
    ("Escape", "escape"),
    ("Esc", "escape"),
    ("Tab", "tab"),
    ("Backspace", "backspace"),
    ("Up", "up"),
    ("Down", "down"),
    ("Left", "left"),
    ("Right", "right"),
];

pub(crate) fn new_vm() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    let globals = lua.globals();

    lua.set_named_registry_value(MODIFIER_INTERN_TABLE, lua.create_table()?)?;
    lua.set_named_registry_value(KEYSTROKE_INTERN_TABLE, lua.create_table()?)?;

    let dome = lua.create_table()?;
    let table_lib: mlua::Table = globals.get("table")?;
    let freeze: mlua::Function = table_lib.get("freeze")?;

    dome.set("os", host_os())?;

    let env = lua.create_table()?;
    // `std::env::vars` panics on a name or value that is not valid Unicode.
    for (name, value) in std::env::vars_os() {
        if let (Some(name), Some(value)) = (name.to_str(), value.to_str()) {
            env.set(environment_variable_name(name), value)?;
        }
    }
    // `table.freeze` on `dome` and `lua.sandbox` leave a nested table such as
    // `env` writable.
    dome.set("env", freeze.call::<mlua::Table>(env)?)?;
    dome.set(
        "executable",
        lua.create_function(|_, name: String| Ok(which::which(name).is_ok()))?,
    )?;
    dome.set(
        "log_debug",
        log_function(&lua, |message| tracing::debug!("{message}"))?,
    )?;
    dome.set(
        "log_info",
        log_function(&lua, |message| tracing::info!("{message}"))?,
    )?;
    dome.set(
        "log_warn",
        log_function(&lua, |message| tracing::warn!("{message}"))?,
    )?;
    dome.set(
        "log_error",
        log_function(&lua, |message| tracing::error!("{message}"))?,
    )?;

    let build: mlua::Function = lua.load(DEFAULT_LUA).set_name("default.lua").eval()?;
    let default_build = build.clone();
    dome.set(
        "defaults",
        lua.create_function(move |_, ()| default_build.call::<mlua::Table>(()))?,
    )?;
    dome.set(
        "with_default_modifier",
        lua.create_function(move |lua, modifier: mlua::Value| {
            let modifiers = modifier
                .as_userdata()
                .and_then(|ud| ud.borrow::<Modifier>().ok())
                .map(|m| m.0)
                .filter(|m| *m == Modifiers::META || *m == Modifiers::ALT)
                .ok_or_else(|| caller_error(lua, DEFAULT_MODIFIER_ERROR))?;
            let build = build.clone();
            let builder = lua.create_table()?;
            builder.set(
                "defaults",
                lua.create_function(move |lua, ()| {
                    build.call::<mlua::Table>((interned_modifier(lua, modifiers)?,))
                })?,
            )?;
            Ok(builder)
        })?,
    )?;

    let dome: mlua::Table = freeze.call(dome)?;
    globals.set("dome", dome)?;

    for (name, modifiers) in MODIFIER_NAMES {
        globals.set(name, interned_modifier(&lua, modifiers)?)?;
    }
    for (name, key) in KEY_NAMES {
        globals.set(name, interned_keystroke(&lua, Modifiers::empty(), key)?)?;
    }

    lua.sandbox(true)?;
    Ok(lua)
}

#[derive(Clone, Copy)]
struct Modifier(Modifiers);

impl mlua::UserData for Modifier {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| {
            Ok(this.0.to_string())
        });
        // Takes both operands rather than a `self`, so a reversed `"h" + Meta`
        // produces a domain error from the guard below instead of mlua's own
        // extraction error for `self`.
        methods.add_meta_function(
            mlua::MetaMethod::Add,
            |lua, (lhs, rhs): (mlua::Value, mlua::Value)| {
                let mlua::Value::UserData(ud) = &lhs else {
                    return Err(caller_error(lua, MODIFIER_ADD_ERROR));
                };
                let this = *ud
                    .borrow::<Modifier>()
                    .map_err(|_| caller_error(lua, MODIFIER_ADD_ERROR))?;
                match rhs {
                    mlua::Value::UserData(ud) => {
                        if let Ok(other) = ud.borrow::<Modifier>() {
                            return interned_modifier(lua, this.0 | other.0)
                                .map(mlua::Value::UserData);
                        }
                        let named = ud
                            .borrow::<Keystroke>()
                            .map_err(|_| caller_error(lua, MODIFIER_ADD_ERROR))?;
                        interned_keystroke(lua, this.0 | named.modifiers, &named.key)
                            .map(mlua::Value::UserData)
                    }
                    mlua::Value::String(key) => {
                        let key = key
                            .to_str()
                            .map_err(|_| caller_error(lua, KEY_NAME_ERROR))?;
                        if key.is_empty() {
                            return Err(caller_error(lua, KEY_NAME_ERROR));
                        }
                        interned_keystroke(lua, this.0, &key.to_ascii_lowercase())
                            .map(mlua::Value::UserData)
                    }
                    _ => Err(caller_error(lua, MODIFIER_ADD_ERROR)),
                }
            },
        );
    }
}

// Reports an unsupported target under its own name rather than folding it into
// "windows", so a config branching on `dome.os` takes neither platform's arm.
fn host_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        std::env::consts::OS
    }
}

fn log_function(lua: &mlua::Lua, emit: fn(&str)) -> mlua::Result<mlua::Function> {
    // Luau's `tostring` rather than `mlua::Value::to_string`, so a number reads
    // the way `print` shows it and a string that is not valid UTF-8 is logged
    // lossily instead of raising an error.
    let tostring: mlua::Function = lua.globals().get("tostring")?;
    let pcall: mlua::Function = lua.globals().get("pcall")?;
    lua.create_function(move |lua, values: mlua::Variadic<mlua::Value>| {
        let mut parts = Vec::with_capacity(values.len());
        for value in values {
            // An error from `Function::call` arrives with its traceback
            // appended. `pcall` returns the bare reason instead.
            let (converted, result): (bool, mlua::Value) = pcall.call((&tostring, value))?;
            let reason = match result {
                mlua::Value::String(text) if converted => {
                    parts.push(text.to_string_lossy());
                    continue;
                }
                mlua::Value::String(reason) => reason.to_string_lossy(),
                other => format!("(error object is a {} value)", other.type_name()),
            };
            return Err(caller_error(lua, &format!("{LOG_VALUE_ERROR}: {reason}")));
        }
        emit(&parts.join(" "));
        Ok(())
    })
}

// One userdata per bit pattern. Interning makes `Cmd` the very same value as
// `Meta`, so Lua's own identity equality answers an alias comparison.
// Interning also stops a keymap table from holding one modifier set under two
// aliases.
fn interned_modifier(lua: &mlua::Lua, modifiers: Modifiers) -> mlua::Result<mlua::AnyUserData> {
    let table: mlua::Table = lua.named_registry_value(MODIFIER_INTERN_TABLE)?;
    let bits = modifiers.bits();
    if let Some(held) = table.get::<Option<mlua::AnyUserData>>(bits)? {
        return Ok(held);
    }
    let fresh = lua.create_userdata(Modifier(modifiers))?;
    table.set(bits, fresh.clone())?;
    Ok(fresh)
}

// One userdata per modifier set and key pair, because a Lua table indexes by
// primitive equality and ignores `__eq`. Without interning, writing the same
// keystroke twice in one keymap would make two table keys.
fn interned_keystroke(
    lua: &mlua::Lua,
    modifiers: Modifiers,
    key: &str,
) -> mlua::Result<mlua::AnyUserData> {
    let by_modifiers: mlua::Table = lua.named_registry_value(KEYSTROKE_INTERN_TABLE)?;
    let bits = modifiers.bits();
    let by_key = match by_modifiers.get::<Option<mlua::Table>>(bits)? {
        Some(by_key) => by_key,
        None => {
            let fresh = lua.create_table()?;
            by_modifiers.set(bits, fresh.clone())?;
            fresh
        }
    };
    if let Some(held) = by_key.get::<Option<mlua::AnyUserData>>(key)? {
        return Ok(held);
    }
    let fresh = lua.create_userdata(Keystroke {
        key: key.to_string(),
        modifiers,
    })?;
    by_key.set(key, fresh.clone())?;
    Ok(fresh)
}
