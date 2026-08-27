//! The chord grammar and the keymap tables it keys.

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde::de::{self, MapAccess, Visitor};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

mod state;

pub(crate) use state::{KeymapPublisher, KeymapState};

/// The newtype name `CallbackId` deserializes through. A Lua function is
/// accepted under this name and rejected everywhere else, so an integer cannot
/// stand in for a callback and a function cannot stand in for a number.
pub(crate) const CALLBACK_NEWTYPE_NAME: &str = "$dome::CallbackId";

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
pub(crate) struct Keymap {
    pub(crate) key: String,
    pub(crate) modifiers: Modifiers,
}

impl FromStr for Keymap {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty keymap"));
        }
        let key = parts.last().unwrap().to_string();
        let mut modifiers = Modifiers::empty();
        for m in &parts[..parts.len() - 1] {
            modifiers |= match *m {
                "meta" | "cmd" | "win" => Modifiers::META,
                "shift" => Modifiers::SHIFT,
                "alt" => Modifiers::ALT,
                "ctrl" => Modifiers::CTRL,
                _ => return Err(anyhow!("Unknown modifier: {}", m)),
            };
        }
        Ok(Keymap { key, modifiers })
    }
}

impl fmt::Display for Keymap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (bit, token) in [
            (Modifiers::META, "meta"),
            (Modifiers::CTRL, "ctrl"),
            (Modifiers::ALT, "alt"),
            (Modifiers::SHIFT, "shift"),
        ] {
            if self.modifiers.contains(bit) {
                write!(f, "{token}+")?;
            }
        }
        f.write_str(&self.key)
    }
}

impl<'de> Deserialize<'de> for Keymap {
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let chord = String::deserialize(deserializer)?;
        chord
            .parse()
            .map_err(|e| de::Error::custom(format!("invalid key {chord:?}: {e}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CallbackId(pub usize);

impl<'de> Deserialize<'de> for CallbackId {
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct CallbackIdVisitor;

        impl<'de> Visitor<'de> for CallbackIdVisitor {
            type Value = CallbackId;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a function")
            }

            fn visit_u64<E: de::Error>(self, id: u64) -> std::result::Result<CallbackId, E> {
                Ok(CallbackId(id as usize))
            }
        }

        deserializer.deserialize_newtype_struct(CALLBACK_NEWTYPE_NAME, CallbackIdVisitor)
    }
}

const BASE_MODE: &str = "main";

#[derive(Debug, Clone, Default)]
pub(crate) struct ModalKeymaps {
    pub(crate) modes: HashMap<String, HashMap<Keymap, CallbackId>>,
}

impl<'de> Deserialize<'de> for ModalKeymaps {
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(ModalKeymapsVisitor)
    }
}

struct ModalKeymapsVisitor;

impl<'de> Visitor<'de> for ModalKeymapsVisitor {
    type Value = ModalKeymaps;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a table of keymaps")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<ModalKeymaps, A::Error> {
        let mut modes: HashMap<String, HashMap<Keymap, CallbackId>> = HashMap::new();
        loop {
            let mode_name = match map.next_key::<String>() {
                Ok(Some(name)) => name,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(field = "keymaps", error = %e, "Invalid mode name, dropping");
                    continue;
                }
            };
            if mode_name.is_empty() {
                tracing::warn!(field = "keymaps.", "Empty mode name, dropping");
                let _ = map.next_value::<de::IgnoredAny>();
                continue;
            }
            let prefix = format!("keymaps.{mode_name}");
            match map.next_value_seed(BindingsSeed { prefix: &prefix }) {
                Ok(bindings) => {
                    modes.insert(mode_name, bindings);
                }
                Err(e) => tracing::warn!(
                    field = %prefix,
                    error = %e,
                    "Expected a table of bindings for the mode, dropping",
                ),
            }
        }

        if !modes.is_empty() && !modes.contains_key(BASE_MODE) {
            tracing::warn!(
                base_mode = BASE_MODE,
                "Keymaps define no base mode, startup keypresses will not resolve"
            );
        }
        Ok(ModalKeymaps { modes })
    }

    fn visit_unit<E: de::Error>(self) -> std::result::Result<ModalKeymaps, E> {
        Ok(ModalKeymaps::default())
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> std::result::Result<ModalKeymaps, E> {
        Ok(invalid_keymaps("boolean"))
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<ModalKeymaps, E> {
        Ok(invalid_keymaps("number"))
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> std::result::Result<ModalKeymaps, E> {
        Ok(invalid_keymaps("number"))
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> std::result::Result<ModalKeymaps, E> {
        Ok(invalid_keymaps("number"))
    }

    fn visit_str<E: de::Error>(self, _: &str) -> std::result::Result<ModalKeymaps, E> {
        Ok(invalid_keymaps("string"))
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, _: A) -> std::result::Result<ModalKeymaps, A::Error> {
        Ok(invalid_keymaps("list"))
    }
}

fn invalid_keymaps(found: &str) -> ModalKeymaps {
    tracing::warn!(
        field = "keymaps",
        error = %format!("expected table, got {found}"),
        "Invalid value, using none",
    );
    ModalKeymaps::default()
}

fn field_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

struct BindingsSeed<'prefix> {
    prefix: &'prefix str,
}

impl<'de> de::DeserializeSeed<'de> for BindingsSeed<'_> {
    type Value = HashMap<Keymap, CallbackId>;

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        deserializer.deserialize_map(BindingsVisitor {
            prefix: self.prefix,
        })
    }
}

struct BindingsVisitor<'prefix> {
    prefix: &'prefix str,
}

impl<'de> Visitor<'de> for BindingsVisitor<'_> {
    type Value = HashMap<Keymap, CallbackId>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a table of bindings")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut bindings = HashMap::new();
        loop {
            // Both arms continue rather than return, because the map access
            // consumes an entry before it can fail.
            let keymap = match map.next_key::<Keymap>() {
                Ok(Some(keymap)) => keymap,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(field = %self.prefix, error = %e, "Invalid key binding, dropping");
                    continue;
                }
            };
            match map.next_value::<CallbackId>() {
                Ok(id) => {
                    bindings.insert(keymap, id);
                }
                Err(e) => {
                    let field = field_path(self.prefix, &keymap.to_string());
                    tracing::warn!(field = %field, error = %e, "Invalid binding, dropping");
                }
            }
        }
        Ok(bindings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymap_parses_meta_modifier() {
        let key: Keymap = "meta+t".parse().unwrap();
        assert_eq!(key.modifiers, Modifiers::META);
    }

    #[test]
    fn keymap_accepts_cmd_and_win_aliases() {
        let cmd: Keymap = "cmd+t".parse().unwrap();
        assert_eq!(cmd.modifiers, Modifiers::META);
        let win: Keymap = "win+t".parse().unwrap();
        assert_eq!(win.modifiers, Modifiers::META);
        let mixed: Keymap = "cmd+shift+t".parse().unwrap();
        assert_eq!(mixed.modifiers, Modifiers::META | Modifiers::SHIFT);
    }
}
