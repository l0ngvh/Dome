//! The keystroke grammar and the keymap tables it keys.

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::config::lua::caller_error;
use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table};

pub(crate) const KEYSTROKE_ADD_ERROR: &str =
    "a key binding holds one key, so a key joins with nothing further";

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct Modifiers: u8 {
        const META = 1 << 0;
        const SHIFT = 1 << 1;
        const ALT = 1 << 2;
        const CTRL = 1 << 3;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Keystroke {
    pub(crate) key: String,
    pub(crate) modifiers: Modifiers,
}

impl FromStr for Keystroke {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        let key = parts.last().unwrap().to_ascii_lowercase();
        if key.is_empty() {
            return Err(anyhow!("Empty key name"));
        }
        let mut modifiers = Modifiers::empty();
        for m in &parts[..parts.len() - 1] {
            // The Lua globals are Title-cased, so a user mirroring `Meta` in a
            // string key writes "Meta+h".
            modifiers |= match m.to_ascii_lowercase().as_str() {
                "meta" | "cmd" | "win" => Modifiers::META,
                "shift" => Modifiers::SHIFT,
                "alt" | "option" | "opt" => Modifiers::ALT,
                "ctrl" | "control" => Modifiers::CTRL,
                _ => return Err(anyhow!("Unknown modifier: {}", m)),
            };
        }
        Ok(Keystroke { key, modifiers })
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (bit, token) in [
            (Modifiers::META, "meta"),
            (Modifiers::CTRL, "ctrl"),
            (Modifiers::ALT, "alt"),
            (Modifiers::SHIFT, "shift"),
        ] {
            if self.contains(bit) {
                if !first {
                    f.write_str("+")?;
                }
                f.write_str(token)?;
                first = false;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.modifiers.is_empty() {
            write!(f, "{}+", self.modifiers)?;
        }
        f.write_str(&self.key)
    }
}

impl mlua::UserData for Keystroke {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| {
            Ok(this.to_string())
        });
        // Luau would otherwise report its own arithmetic error, which names
        // neither Dome nor the mistake.
        methods.add_meta_method(mlua::MetaMethod::Add, |lua, _, _: mlua::Value| {
            Err::<mlua::Value, _>(caller_error(lua, KEYSTROKE_ADD_ERROR))
        });
    }
}

impl FromLuaValue for Keystroke {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let mut keystroke = if let mlua::Value::UserData(handle) = value {
            handle
                .borrow::<Keystroke>()
                .map(|held| held.clone())
                .map_err(|_| mlua::Error::runtime("expected a key built from a modifier"))?
        } else {
            let text = String::from_lua_value(value, cx)?;
            text.parse::<Keystroke>()
                .map_err(|e| mlua::Error::runtime(format!("invalid key {text:?}: {e}")))?
        };
        if keystroke.key == "enter" {
            keystroke.key = "return".to_string();
        }
        Ok(keystroke)
    }
}

pub(crate) const BASE_MODE: &str = "main";

#[derive(Debug, Clone, Default)]
pub(crate) struct ModalKeymaps {
    pub(crate) modes: HashMap<String, HashMap<Keystroke, mlua::Function>>,
}

impl FromLuaValue for ModalKeymaps {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table of keymaps")?;
        let mut modes = HashMap::new();
        for pair in table.pairs::<mlua::Value, mlua::Value>() {
            let (name, bindings) = pair?;
            let Ok(name) = String::from_lua_value(&name, cx) else {
                cx.warn_dropped(&format!(
                    "keymap name must be a string, got {}",
                    name.type_name()
                ));
                continue;
            };
            cx.push(&name);
            if name.is_empty() {
                cx.warn_dropped("a keymap name must not be empty");
                cx.pop();
                continue;
            }
            match read_bindings(&bindings, cx) {
                Ok(bindings) => {
                    modes.insert(name, bindings);
                }
                Err(e) => cx.warn_dropped(&e.to_string()),
            }
            cx.pop();
        }
        if !modes.is_empty() && !modes.contains_key(BASE_MODE) {
            tracing::warn!(
                base_mode = BASE_MODE,
                "Keymaps define no base mode, startup keypresses will not resolve"
            );
        }
        Ok(ModalKeymaps { modes })
    }
}

fn read_bindings(
    value: &mlua::Value,
    cx: &mut LoadContext,
) -> mlua::Result<HashMap<Keystroke, mlua::Function>> {
    let table = as_table(value, "a table of bindings")?;
    let mut out: HashMap<Keystroke, mlua::Function> = HashMap::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, action) = pair?;
        let label = binding_label(&key, cx);
        cx.push(&label);
        match read_binding(&key, &action, cx) {
            Ok((keystroke, function)) => {
                // Which of the two survives is unspecified, because a Lua table
                // records no insertion order.
                if out.insert(keystroke.clone(), function).is_some() {
                    cx.warn_dropped(&format!("{keystroke} is bound twice"));
                }
            }
            Err(e) => cx.warn_dropped(&e.to_string()),
        }
        cx.pop();
    }
    Ok(out)
}

// A typed key would label as "userdata", so read it first and keep the raw text
// as the fallback for a string that does not parse.
fn binding_label(key: &mlua::Value, cx: &mut LoadContext) -> String {
    match Keystroke::from_lua_value(key, cx) {
        Ok(keystroke) => keystroke.to_string(),
        Err(_) => String::from_lua_value(key, cx).unwrap_or_else(|_| key.type_name().to_string()),
    }
}

fn read_binding(
    key: &mlua::Value,
    action: &mlua::Value,
    cx: &mut LoadContext,
) -> mlua::Result<(Keystroke, mlua::Function)> {
    let keystroke = Keystroke::from_lua_value(key, cx)?;
    let function = mlua::Function::from_lua_value(action, cx)?;
    Ok((keystroke, function))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_reads_the_modifiers_and_the_key_name() {
        for (input, modifiers) in [
            ("meta+t", Modifiers::META),
            ("cmd+t", Modifiers::META),
            ("win+t", Modifiers::META),
            ("cmd+shift+t", Modifiers::META | Modifiers::SHIFT),
        ] {
            let key: Keystroke = input.parse().unwrap();
            assert_eq!(key.modifiers, modifiers, "{input}");
            assert_eq!(key.key, "t", "{input}");
        }
    }

    #[test]
    fn parsing_ignores_case_in_the_key_and_the_modifiers() {
        for input in ["Meta+H", "META+h", "Cmd+Shift+T", "Option+t", "Control+t"] {
            let key: Keystroke = input.parse().unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(key.key, key.key.to_ascii_lowercase(), "{input}");
        }
        assert_eq!(
            "Meta+H".parse::<Keystroke>().unwrap(),
            "meta+h".parse::<Keystroke>().unwrap()
        );
        assert_eq!(
            "Option+t".parse::<Keystroke>().unwrap(),
            "alt+t".parse::<Keystroke>().unwrap()
        );
    }

    #[test]
    fn parsing_rejects_a_malformed_keystroke() {
        for input in ["", "meta+", "hyper+t", "meta+shift+"] {
            assert!(input.parse::<Keystroke>().is_err(), "{input}");
        }
    }

    #[test]
    fn display_round_trips_through_parsing() {
        for input in ["t", "meta+t", "meta+ctrl+alt+shift+t"] {
            let key: Keystroke = input.parse().unwrap();
            assert_eq!(key.to_string(), input, "{input}");
            assert_eq!(
                key.to_string().parse::<Keystroke>().unwrap(),
                key,
                "{input}"
            );
        }
    }
}
