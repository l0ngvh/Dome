//! Builds the Luau VM and the read-only `dome` global.

use crate::config::{DEFAULT_LUA, Modifiers};

const MODIFIER_ADD_ERROR: &str = "a modifier joins only with another modifier or a key string";

// A setter captures its modifier in a closure, so no state lives on `dome` and
// it stays a plain frozen table.
const INSTALL_DEFAULTS: &str = r#"return function(build, dome)
	dome.defaults = function()
		return build()
	end
	dome.with_default_modifier = function(modifier)
		if modifier ~= Meta and modifier ~= Alt then
			error("dome.with_default_modifier accepts only Meta or Alt", 2)
		end
		return {
			defaults = function()
				return build(modifier)
			end,
		}
	end
end"#;

#[derive(Clone, Copy)]
struct Modifier(Modifiers);

impl mlua::UserData for Modifier {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            mlua::MetaMethod::Add,
            |lua, this, rhs: mlua::Value| match rhs {
                mlua::Value::UserData(ud) => {
                    let other = ud
                        .borrow::<Modifier>()
                        .map_err(|_| mlua::Error::runtime(MODIFIER_ADD_ERROR))?;
                    lua.create_userdata(Modifier(this.0 | other.0))
                        .map(mlua::Value::UserData)
                }
                mlua::Value::String(key) => lua
                    .create_string(chord_string(this.0, &key.to_string_lossy()))
                    .map(mlua::Value::String),
                _ => Err(mlua::Error::runtime(MODIFIER_ADD_ERROR)),
            },
        );
        methods.add_meta_method(mlua::MetaMethod::Eq, |_, this, other: mlua::Value| {
            let equal = other
                .as_userdata()
                .and_then(|ud| ud.borrow::<Modifier>().ok().map(|o| o.0 == this.0))
                .unwrap_or(false);
            Ok(equal)
        });
    }
}

fn chord_string(mods: Modifiers, key: &str) -> String {
    let mut chord = String::new();
    for (bit, token) in [
        (Modifiers::META, "meta"),
        (Modifiers::CTRL, "ctrl"),
        (Modifiers::ALT, "alt"),
        (Modifiers::SHIFT, "shift"),
    ] {
        if mods.contains(bit) {
            chord.push_str(token);
            chord.push('+');
        }
    }
    chord.push_str(key);
    chord
}

pub(crate) fn build_vm() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    let globals = lua.globals();

    let dome = lua.create_table()?;
    dome.set(
        "os",
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "windows"
        },
    )?;
    dome.set(
        "executable",
        lua.create_function(|_, name: String| Ok(which::which(name).is_ok()))?,
    )?;

    let build: mlua::Function = lua.load(DEFAULT_LUA).set_name("default.lua").eval()?;
    let install: mlua::Function = lua
        .load(INSTALL_DEFAULTS)
        .set_name("dome.defaults")
        .eval()?;
    install.call::<()>((build, dome.clone()))?;

    let table_lib: mlua::Table = globals.get("table")?;
    let freeze: mlua::Function = table_lib.get("freeze")?;
    let dome: mlua::Table = freeze.call(dome)?;
    globals.set("dome", dome)?;

    globals.set("Meta", Modifier(Modifiers::META))?;
    globals.set("Alt", Modifier(Modifiers::ALT))?;
    globals.set("Ctrl", Modifier(Modifiers::CTRL))?;
    globals.set("Shift", Modifier(Modifiers::SHIFT))?;
    globals.set("Cmd", Modifier(Modifiers::META))?;
    globals.set("Win", Modifier(Modifiers::META))?;
    globals.set("Option", Modifier(Modifiers::ALT))?;
    globals.set("Opt", Modifier(Modifiers::ALT))?;
    globals.set("Control", Modifier(Modifiers::CTRL))?;

    // Sandbox after installing the surface, so its globals are protected from
    // in-place mutation.
    lua.sandbox(true)?;
    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_add_unions_and_attaches_key() {
        let lua = build_vm().unwrap();
        let one: String = lua.load(r#"return Meta + "h""#).eval().unwrap();
        assert_eq!(one, "meta+h");
        let two: String = lua.load(r#"return (Meta + Ctrl) + "y""#).eval().unwrap();
        assert_eq!(two, "meta+ctrl+y");
        let four: String = lua
            .load(r#"return (Meta + Alt + Ctrl + Shift) + "x""#)
            .eval()
            .unwrap();
        assert_eq!(four, "meta+ctrl+alt+shift+x");
    }

    #[test]
    fn modifier_second_key_errors() {
        let lua = build_vm().unwrap();
        assert!(
            lua.load(r#"return Meta + "h" + "j""#)
                .eval::<String>()
                .is_err()
        );
    }

    #[test]
    fn dome_surface_is_frozen() {
        let lua = build_vm().unwrap();
        let os: String = lua.load("return dome.os").eval().unwrap();
        assert!(os == "macos" || os == "windows");
        assert!(lua.load(r#"dome.os = "x""#).exec().is_err());
        assert!(lua.load("dome.defaults = 1").exec().is_err());
        assert!(lua.load("dome.new_field = 1").exec().is_err());
    }

    #[test]
    fn dome_with_default_modifier_rebuilds_the_keymap() {
        let lua = build_vm().unwrap();
        let default_has_alt: bool = lua
            .load(r#"return dome.defaults().keymaps.main["alt+h"] ~= nil"#)
            .eval()
            .unwrap();
        assert!(default_has_alt);
        let rebuilt: bool = lua
            .load(
                r#"local m = dome.with_default_modifier(Meta).defaults().keymaps.main
return m["meta+h"] ~= nil and m["alt+h"] == nil and m["meta+ctrl+h"] ~= nil"#,
            )
            .eval()
            .unwrap();
        assert!(rebuilt);
    }

    #[test]
    fn with_default_modifier_accepts_only_meta_or_alt() {
        let lua = build_vm().unwrap();
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
    fn executable_query_returns_boolean() {
        let lua = build_vm().unwrap();
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
        let lua = build_vm().unwrap();
        let leaked: bool = lua
            .load(
                r#"local a = dome.defaults()
a.keymaps.main["meta+x"] = function() end
local b = dome.defaults()
return b.keymaps.main["meta+x"] ~= nil"#,
            )
            .eval()
            .unwrap();
        assert!(!leaked);
    }
}
