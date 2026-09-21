//! Builds the Luau VM and the read-only `dome` global.

use crate::config::{Keystroke, Modifiers};

use super::caller_error;

const DEFAULT_LUA: &str = include_str!("../../../resources/default.lua");

const MODIFIER_ADD_ERROR: &str =
    "a modifier joins only with another modifier or a key, such as Space or \"h\"";

// A lossy conversion would fold two distinct byte sequences onto one key, so the
// second binding would replace the first.
const KEY_NAME_ERROR: &str = "a key name must be non-empty valid UTF-8";

const DEFAULT_MODIFIER_ERROR: &str = "dome.with_default_modifier accepts only Meta or Alt";

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
    // Both names reach the main Return key. On macOS the numpad Enter is a
    // separate key named "enter", which only the string form binds.
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

pub(crate) fn new_vm() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    let globals = lua.globals();

    // The intern tables must exist before any chunk loads.
    lua.set_named_registry_value(MODIFIER_INTERN_TABLE, lua.create_table()?)?;
    lua.set_named_registry_value(KEYSTROKE_INTERN_TABLE, lua.create_table()?)?;

    let dome = lua.create_table()?;
    dome.set("os", host_os())?;
    dome.set(
        "executable",
        lua.create_function(|_, name: String| Ok(which::which(name).is_ok()))?,
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

    let table_lib: mlua::Table = globals.get("table")?;
    let freeze: mlua::Function = table_lib.get("freeze")?;
    let dome: mlua::Table = freeze.call(dome)?;
    globals.set("dome", dome)?;

    for (name, modifiers) in MODIFIER_NAMES {
        globals.set(name, interned_modifier(&lua, modifiers)?)?;
    }
    for (name, key) in KEY_NAMES {
        globals.set(name, interned_keystroke(&lua, Modifiers::empty(), key)?)?;
    }

    // Sandbox after installing the surface, so its globals are protected from
    // in-place mutation.
    lua.sandbox(true)?;
    Ok(lua)
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
//
// `key` is a whole table key here, never spliced around a separator, so a key
// named "+" interns like any other.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::keybinding::KEYSTROKE_ADD_ERROR;

    #[test]
    fn modifier_add_builds_a_keystroke_value() {
        let lua = new_vm().unwrap();
        let typed: String = lua.load(r#"return type(Meta + "h")"#).eval().unwrap();
        assert_eq!(typed, "userdata");
        let one: String = lua.load(r#"return tostring(Meta + "h")"#).eval().unwrap();
        assert_eq!(one, "meta+h");
        let two: String = lua
            .load(r#"return tostring((Meta + Ctrl) + "y")"#)
            .eval()
            .unwrap();
        assert_eq!(two, "meta+ctrl+y");
        let four: String = lua
            .load(r#"return tostring((Meta + Alt + Ctrl + Shift) + "x")"#)
            .eval()
            .unwrap();
        assert_eq!(four, "meta+ctrl+alt+shift+x");
    }

    #[test]
    fn the_same_keystroke_is_one_table_key() {
        let lua = new_vm().unwrap();
        let count: usize = lua
            .load(
                r#"local t = {}
t[Meta + "h"] = 1
t[Meta + "h"] = 2
t[Cmd + "h"] = 3
local n = 0
for _ in pairs(t) do n = n + 1 end
return n"#,
            )
            .eval()
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn modifier_aliases_compare_equal() {
        let lua = new_vm().unwrap();
        let equal: bool = lua
            .load("return Meta == Cmd and Meta == Win and Alt == Option and Alt == Opt and Ctrl == Control")
            .eval()
            .unwrap();
        assert!(equal);
        let distinct: bool = lua.load("return Meta == Alt").eval().unwrap();
        assert!(!distinct);
    }

    #[test]
    fn modifier_union_interns_by_bits() {
        let lua = new_vm().unwrap();
        let same: bool = lua
            .load("return (Meta + Ctrl) == (Ctrl + Meta)")
            .eval()
            .unwrap();
        assert!(same);
        let alias: bool = lua
            .load("return (Meta + Ctrl) == (Cmd + Control)")
            .eval()
            .unwrap();
        assert!(alias);
    }

    #[test]
    fn a_second_key_names_the_config_line() {
        let lua = new_vm().unwrap();
        let error = lua
            .load(r#"return Meta + "h" + "j""#)
            .set_name("config.lua")
            .eval::<mlua::Value>()
            .unwrap_err()
            .to_string();
        assert!(error.contains(KEYSTROKE_ADD_ERROR), "{error}");
        assert!(error.contains(r#"[string "config.lua"]:1:"#), "{error}");
    }

    #[test]
    fn a_key_global_carries_the_platform_key_name() {
        let lua = new_vm().unwrap();
        for (expression, expected) in [
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
            ("Meta + Space", "meta+space"),
            ("Meta + Shift + Return", "meta+shift+return"),
            ("Ctrl + Left", "ctrl+left"),
            (
                "(Meta + Alt + Ctrl + Shift) + Enter",
                "meta+ctrl+alt+shift+return",
            ),
        ] {
            let rendered: String = lua
                .load(format!("return tostring({expression})"))
                .eval()
                .unwrap();
            assert_eq!(rendered, expected, "{expression}");
        }
    }

    #[test]
    fn an_alias_key_global_names_the_same_key() {
        let lua = new_vm().unwrap();
        let equal: bool = lua
            .load("return Enter == Return and (Meta + Enter) == (Meta + Return) and Esc == Escape")
            .eval()
            .unwrap();
        assert!(equal);
        let distinct: bool = lua.load("return Space == Return").eval().unwrap();
        assert!(!distinct);
    }

    #[test]
    fn a_key_global_and_a_key_string_are_one_table_key() {
        let lua = new_vm().unwrap();
        let count: usize = lua
            .load(
                r#"local t = {}
t[Meta + Space] = 1
t[Meta + "space"] = 2
t[Return] = 3
t[Enter] = 4
local n = 0
for _ in pairs(t) do n = n + 1 end
return n"#,
            )
            .eval()
            .unwrap();
        assert_eq!(count, 2, "meta+space and return");
    }

    #[test]
    fn a_key_global_joins_with_nothing_further() {
        let lua = new_vm().unwrap();
        let error = lua
            .load("return Space + Meta")
            .set_name("config.lua")
            .eval::<mlua::Value>()
            .unwrap_err()
            .to_string();
        assert!(error.contains(KEYSTROKE_ADD_ERROR), "{error}");
        assert!(error.contains(r#"[string "config.lua"]:1:"#), "{error}");
    }

    #[test]
    fn modifier_add_rejects_a_foreign_userdata() {
        struct NotAModifier;
        impl mlua::UserData for NotAModifier {}

        let lua = new_vm().unwrap();
        let add: mlua::Function = lua
            .load("return function(other) return Meta + other end")
            .set_name("config.lua")
            .eval()
            .unwrap();
        let error = add
            .call::<mlua::Value>(lua.create_userdata(NotAModifier).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains(MODIFIER_ADD_ERROR), "{error}");
        assert!(error.contains(r#"[string "config.lua"]:1:"#), "{error}");
    }

    #[test]
    fn modifier_add_error_names_the_config_line() {
        let lua = new_vm().unwrap();
        let error = lua
            .load("return Meta + 1")
            .set_name("config.lua")
            .eval::<mlua::Value>()
            .unwrap_err()
            .to_string();
        assert!(error.contains(MODIFIER_ADD_ERROR), "{error}");
        assert!(error.contains(r#"[string "config.lua"]:1:"#), "{error}");
    }

    #[test]
    fn dome_surface_is_frozen() {
        let lua = new_vm().unwrap();
        let os: String = lua.load("return dome.os").eval().unwrap();
        assert_eq!(os, std::env::consts::OS);
        assert!(lua.load(r#"dome.os = "x""#).exec().is_err());
        assert!(lua.load("dome.defaults = 1").exec().is_err());
        assert!(lua.load("dome.new_field = 1").exec().is_err());
    }

    #[test]
    fn with_default_modifier_swaps_the_modifier_on_every_default_binding() {
        let lua = new_vm().unwrap();
        let bundled: mlua::Table = lua
            .load("return dome.defaults().keymaps.main")
            .eval()
            .unwrap();
        let rebuilt: mlua::Table = lua
            .load("return dome.with_default_modifier(Meta).defaults().keymaps.main")
            .eval()
            .unwrap();

        let mut rebuilt_count = 0;
        for pair in rebuilt.pairs::<mlua::AnyUserData, mlua::Value>() {
            let (key, _) = pair.unwrap();
            let keystroke = key.borrow::<Keystroke>().unwrap();
            assert!(
                keystroke.modifiers.contains(Modifiers::META),
                "{keystroke} should carry the new default modifier"
            );
            assert!(
                !keystroke.modifiers.contains(Modifiers::ALT),
                "{keystroke} should have lost the old default modifier"
            );
            rebuilt_count += 1;
        }

        let bundled_count = bundled.pairs::<mlua::Value, mlua::Value>().count();
        assert!(
            bundled_count > 0,
            "the bundled keymap should bind something"
        );
        assert_eq!(rebuilt_count, bundled_count);
    }

    #[test]
    fn with_default_modifier_swaps_every_default_binding_to_alt() {
        let lua = new_vm().unwrap();
        let from_meta = default_main_keystrokes(&lua, "Meta");
        let from_alt = default_main_keystrokes(&lua, "Alt");

        for keystroke in &from_alt {
            assert!(
                keystroke.modifiers.contains(Modifiers::ALT),
                "{keystroke} should carry the new default modifier"
            );
            assert!(
                !keystroke.modifiers.contains(Modifiers::META),
                "{keystroke} should have lost the old default modifier"
            );
        }

        let mut meta_keys: Vec<&str> = from_meta.iter().map(|k| k.key.as_str()).collect();
        let mut alt_keys: Vec<&str> = from_alt.iter().map(|k| k.key.as_str()).collect();
        meta_keys.sort_unstable();
        alt_keys.sort_unstable();
        assert!(!meta_keys.is_empty(), "the keymap should bind something");
        assert_eq!(alt_keys, meta_keys);
    }

    fn default_main_keystrokes(lua: &mlua::Lua, modifier: &str) -> Vec<Keystroke> {
        let main: mlua::Table = lua
            .load(format!(
                "return dome.with_default_modifier({modifier}).defaults().keymaps.main"
            ))
            .eval()
            .unwrap();
        main.pairs::<mlua::AnyUserData, mlua::Value>()
            .map(|pair| pair.unwrap().0.borrow::<Keystroke>().unwrap().clone())
            .collect()
    }

    #[test]
    fn with_default_modifier_accepts_only_meta_or_alt() {
        let lua = new_vm().unwrap();
        assert!(lua.load("dome.with_default_modifier(Meta)").exec().is_ok());
        assert!(lua.load("dome.with_default_modifier(Alt)").exec().is_ok());
        assert!(lua.load("dome.with_default_modifier(Cmd)").exec().is_ok());
        assert!(lua.load("dome.with_default_modifier(Ctrl)").exec().is_err());
        assert!(
            lua.load("dome.with_default_modifier(Meta + Ctrl)")
                .exec()
                .is_err()
        );
    }

    #[test]
    fn with_default_modifier_rejects_a_non_modifier_value() {
        let lua = new_vm().unwrap();
        let error = lua
            .load(r#"dome.with_default_modifier("meta")"#)
            .set_name("config.lua")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(error.contains(DEFAULT_MODIFIER_ERROR), "{error}");
        assert!(error.contains(r#"[string "config.lua"]:1:"#), "{error}");
    }

    #[test]
    fn executable_query_returns_boolean() {
        let lua = new_vm().unwrap();
        let missing: bool = lua
            .load(r#"return dome.executable("dome-no-such-binary-xyzzy")"#)
            .eval()
            .unwrap();
        assert!(!missing);

        #[cfg(unix)]
        {
            let present: bool = lua
                .load(r#"return dome.executable("/bin/sh")"#)
                .eval()
                .unwrap();
            assert!(present);
        }
    }

    #[test]
    fn dome_defaults_returns_a_fresh_mutable_table() {
        let lua = new_vm().unwrap();
        let leaked: bool = lua
            .load(
                r#"local a = dome.defaults()
a.keymaps.main[Meta + "x"] = function() end
local b = dome.defaults()
return b.keymaps.main[Meta + "x"] ~= nil"#,
            )
            .eval()
            .unwrap();
        assert!(!leaked);
    }
}
