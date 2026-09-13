//! A serde deserializer over `mlua::Value` that turns a Lua function into a
//! `CallbackId`.
//!
//! serde has no data-model type for a function, and mlua's own deserializer
//! converts one to an error or to unit before any `Deserialize` impl can see it
//! (mlua-0.10.5/src/serde/de.rs:169-180). So the only place a callback can be
//! captured is inside a deserializer that owns the descent. This one owns every
//! shape that can hold a child value and delegates the rest to mlua.

use std::collections::HashSet;
use std::ffi::c_void;

use serde::de::{self, DeserializeOwned, DeserializeSeed, Visitor};

use crate::keybinding::CALLBACK_NEWTYPE_NAME;

pub(super) fn from_lua_value<T: DeserializeOwned>(
    value: mlua::Value,
    callbacks: &mut Vec<mlua::Function>,
) -> mlua::Result<T> {
    let mut state = DeserializerState {
        callbacks,
        visited_tables: HashSet::new(),
    };
    T::deserialize(LuaDeserializer {
        value,
        state: &mut state,
    })
}

struct DeserializerState<'callbacks> {
    /// Functions lifted out of the tree, indexed by `CallbackId`.
    callbacks: &'callbacks mut Vec<mlua::Function>,
    /// Addresses of the tables on the current path. A table that refers to
    /// itself is legal Lua, and without this the descent never ends.
    visited_tables: HashSet<*const c_void>,
}

struct LuaDeserializer<'state, 'callbacks> {
    value: mlua::Value,
    state: &'state mut DeserializerState<'callbacks>,
}

impl LuaDeserializer<'_, '_> {
    fn delegate(self) -> mlua::serde::Deserializer {
        mlua::serde::Deserializer::new(self.value)
    }

    fn take_table(&mut self, expected: &str) -> mlua::Result<mlua::Table> {
        match std::mem::replace(&mut self.value, mlua::Value::Nil) {
            mlua::Value::Table(table) => Ok(table),
            other => Err(mlua::Error::runtime(format!(
                "expected {expected}, got {}",
                other.type_name()
            ))),
        }
    }
}

/// Delegates a value shape that cannot hold a child value. Adding a container
/// shape here would hand its whole subtree to mlua, whose child deserializers
/// cannot capture a callback.
macro_rules! forward_to_mlua {
    ($($method:ident),* $(,)?) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> mlua::Result<V::Value> {
                serde::Deserializer::$method(self.delegate(), visitor)
            }
        )*
    };
}

impl<'de> serde::Deserializer<'de> for LuaDeserializer<'_, '_> {
    type Error = mlua::Error;

    forward_to_mlua!(
        deserialize_bool,
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_f32,
        deserialize_f64,
        deserialize_char,
        deserialize_str,
        deserialize_string,
        deserialize_bytes,
        deserialize_byte_buf,
        deserialize_unit,
        deserialize_identifier,
    );

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> mlua::Result<V::Value> {
        match &self.value {
            // mlua treats a table with a positive raw length as a sequence
            // (de.rs:157). The other half of its test, `Table::is_array`, is
            // pub(crate). That half matches only a metatable `lua.to_value`
            // attaches, which a Lua-built tree never has.
            mlua::Value::Table(table) if table.raw_len() > 0 => self.deserialize_seq(visitor),
            mlua::Value::Table(_) => self.deserialize_map(visitor),
            mlua::Value::Function(_) => {
                Err(mlua::Error::runtime("expected a value, got a function"))
            }
            _ => serde::Deserializer::deserialize_any(self.delegate(), visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> mlua::Result<V::Value> {
        match &self.value {
            mlua::Value::Nil => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> mlua::Result<V::Value> {
        serde::Deserializer::deserialize_unit_struct(self.delegate(), name, visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        mut self,
        name: &'static str,
        visitor: V,
    ) -> mlua::Result<V::Value> {
        if name != CALLBACK_NEWTYPE_NAME {
            return visitor.visit_newtype_struct(self);
        }
        match std::mem::replace(&mut self.value, mlua::Value::Nil) {
            mlua::Value::Function(function) => {
                let id = self.state.callbacks.len();
                self.state.callbacks.push(function);
                visitor.visit_u64(id as u64)
            }
            other => Err(mlua::Error::runtime(format!(
                "expected a function, got {}",
                other.type_name()
            ))),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(mut self, visitor: V) -> mlua::Result<V::Value> {
        let table = self.take_table("a list")?;
        let values: Vec<mlua::Value> = table.sequence_values().collect::<mlua::Result<_>>()?;
        // Last, because only the access value below removes the pointer again.
        let pointer = enter_table(self.state, &table)?;
        visitor.visit_seq(SequenceAccess {
            values: values.into_iter(),
            state: self.state,
            pointer,
        })
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> mlua::Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> mlua::Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> mlua::Result<V::Value> {
        let table = self.take_table("a table")?;
        let entries: Vec<(mlua::Value, mlua::Value)> =
            table.pairs().collect::<mlua::Result<_>>()?;
        // Last, because only the access value below removes the pointer again.
        let pointer = enter_table(self.state, &table)?;
        visitor.visit_map(TableAccess {
            entries: entries.into_iter(),
            pending_value: None,
            state: self.state,
            pointer,
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> mlua::Result<V::Value> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        mut self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> mlua::Result<V::Value> {
        if !matches!(self.value, mlua::Value::Table(_)) {
            return serde::Deserializer::deserialize_enum(self.delegate(), name, variants, visitor);
        }
        let table = self.take_table("a variant table")?;
        let (variant, payload) = table
            .pairs()
            .next()
            .transpose()?
            .ok_or_else(|| mlua::Error::runtime("expected a variant table with one entry"))?;
        // A variant table needs its own guard. An enum whose variant holds the
        // same enum descends through here alone, never through a seq or a map.
        let pointer = enter_table(self.state, &table)?;
        visitor.visit_enum(VariantAccess {
            variant,
            payload,
            state: self.state,
            pointer,
        })
    }

    /// Skips the value with no descent. mlua forwards this to
    /// `deserialize_any`, which errors on a function.
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> mlua::Result<V::Value> {
        visitor.visit_unit()
    }
}

/// Records `table` as being on the current path, rejecting a cycle. The access
/// value that owns the table removes it again on drop.
fn enter_table(
    state: &mut DeserializerState<'_>,
    table: &mlua::Table,
) -> mlua::Result<*const c_void> {
    let pointer = table.to_pointer();
    if !state.visited_tables.insert(pointer) {
        return Err(mlua::Error::runtime("recursive table detected"));
    }
    Ok(pointer)
}

/// Leaves the table on drop rather than at the end of the last access method,
/// so an early return or an abandoned access still ends the path.
macro_rules! leave_table_on_drop {
    ($($access:ident),* $(,)?) => {
        $(
            impl Drop for $access<'_, '_> {
                fn drop(&mut self) {
                    self.state.visited_tables.remove(&self.pointer);
                }
            }
        )*
    };
}

leave_table_on_drop!(SequenceAccess, TableAccess, VariantAccess);

struct SequenceAccess<'state, 'callbacks> {
    values: std::vec::IntoIter<mlua::Value>,
    state: &'state mut DeserializerState<'callbacks>,
    pointer: *const c_void,
}

impl<'de> de::SeqAccess<'de> for SequenceAccess<'_, '_> {
    type Error = mlua::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> mlua::Result<Option<T::Value>> {
        let Some(value) = self.values.next() else {
            return Ok(None);
        };
        seed.deserialize(LuaDeserializer {
            value,
            state: &mut *self.state,
        })
        .map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.values.len())
    }
}

/// Both entry methods consume the entry before they can fail, so a visitor may
/// log an error and ask for the next entry. serde makes no such promise in
/// general.
struct TableAccess<'state, 'callbacks> {
    entries: std::vec::IntoIter<(mlua::Value, mlua::Value)>,
    pending_value: Option<mlua::Value>,
    state: &'state mut DeserializerState<'callbacks>,
    pointer: *const c_void,
}

impl<'de> de::MapAccess<'de> for TableAccess<'_, '_> {
    type Error = mlua::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> mlua::Result<Option<K::Value>> {
        let Some((key, value)) = self.entries.next() else {
            self.pending_value = None;
            return Ok(None);
        };
        self.pending_value = Some(value);
        seed.deserialize(LuaDeserializer {
            value: key,
            state: &mut *self.state,
        })
        .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> mlua::Result<V::Value> {
        let value = self
            .pending_value
            .take()
            .ok_or_else(|| mlua::Error::runtime("value requested without a key"))?;
        seed.deserialize(LuaDeserializer {
            value,
            state: &mut *self.state,
        })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

struct VariantAccess<'state, 'callbacks> {
    variant: mlua::Value,
    payload: mlua::Value,
    state: &'state mut DeserializerState<'callbacks>,
    pointer: *const c_void,
}

impl<'de> de::EnumAccess<'de> for VariantAccess<'_, '_> {
    type Error = mlua::Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(mut self, seed: V) -> mlua::Result<(V::Value, Self)> {
        let variant = std::mem::replace(&mut self.variant, mlua::Value::Nil);
        let name = seed.deserialize(LuaDeserializer {
            value: variant,
            state: &mut *self.state,
        })?;
        Ok((name, self))
    }
}

impl<'de> de::VariantAccess<'de> for VariantAccess<'_, '_> {
    type Error = mlua::Error;

    fn unit_variant(self) -> mlua::Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(mut self, seed: T) -> mlua::Result<T::Value> {
        let payload = std::mem::replace(&mut self.payload, mlua::Value::Nil);
        seed.deserialize(LuaDeserializer {
            value: payload,
            state: &mut *self.state,
        })
    }

    fn tuple_variant<V: Visitor<'de>>(mut self, _len: usize, visitor: V) -> mlua::Result<V::Value> {
        let payload = std::mem::replace(&mut self.payload, mlua::Value::Nil);
        serde::Deserializer::deserialize_seq(
            LuaDeserializer {
                value: payload,
                state: &mut *self.state,
            },
            visitor,
        )
    }

    fn struct_variant<V: Visitor<'de>>(
        mut self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> mlua::Result<V::Value> {
        let payload = std::mem::replace(&mut self.payload, mlua::Value::Nil);
        serde::Deserializer::deserialize_map(
            LuaDeserializer {
                value: payload,
                state: &mut *self.state,
            },
            visitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinding::CallbackId;
    use serde::Deserialize;

    /// Locals drop in reverse declaration order, so `lua` outlives every value
    /// taken from it. An `mlua::Function` dropped after its `Lua` panics.
    fn deserialize<T: DeserializeOwned>(src: &str) -> mlua::Result<T> {
        let lua = mlua::Lua::new();
        let value: mlua::Value = lua.load(src).eval().expect("test source should evaluate");
        let mut callbacks = Vec::new();
        from_lua_value(value, &mut callbacks)
    }

    fn deserialize_with<T: DeserializeOwned, F: FnOnce(mlua::Result<T>, &[mlua::Function])>(
        src: &str,
        check: F,
    ) {
        let lua = mlua::Lua::new();
        let value: mlua::Value = lua.load(src).eval().expect("test source should evaluate");
        let mut callbacks = Vec::new();
        let result = from_lua_value(value, &mut callbacks);
        check(result, &callbacks);
    }

    #[derive(Debug, Deserialize)]
    struct Scalars {
        count: u32,
        ratio: f64,
        enabled: bool,
        name: String,
    }

    #[test]
    fn forwards_scalars_to_mlua() {
        let scalars: Scalars =
            deserialize(r#"return { count = 3, ratio = 0.5, enabled = true, name = "dome" }"#)
                .expect("scalars should deserialize");
        assert_eq!(scalars.count, 3);
        assert_eq!(scalars.ratio, 0.5);
        assert!(scalars.enabled);
        assert_eq!(scalars.name, "dome");
    }

    #[derive(Deserialize)]
    struct Rule {
        app: String,
    }

    #[derive(Deserialize)]
    struct Rules {
        rules: Vec<Rule>,
        empty: Vec<Rule>,
    }

    #[test]
    fn deserializes_a_nested_list_of_structs() {
        let rules: Rules =
            deserialize(r#"return { rules = { { app = "a" }, { app = "b" } }, empty = {} }"#)
                .expect("rules should deserialize");
        let apps: Vec<&str> = rules.rules.iter().map(|r| r.app.as_str()).collect();
        assert_eq!(apps, ["a", "b"]);
        assert!(rules.empty.is_empty());
    }

    #[derive(Debug, Deserialize)]
    struct Handlers {
        on_key: CallbackId,
    }

    #[test]
    fn interns_a_function_as_a_callback_id() {
        deserialize_with(
            "return { on_key = function() return 7 end }",
            |handlers: mlua::Result<Handlers>, callbacks| {
                let handlers = handlers.expect("handlers should deserialize");
                assert_eq!(handlers.on_key.0, 0);
                let value: i64 = callbacks[handlers.on_key.0]
                    .call(())
                    .expect("the interned function should call");
                assert_eq!(value, 7);
            },
        );
    }

    #[test]
    fn rejects_a_function_where_a_scalar_belongs() {
        let error = deserialize::<Scalars>(
            r#"return { count = function() end, ratio = 0.5, enabled = true, name = "dome" }"#,
        )
        .expect_err("a function is not a count");
        assert!(error.to_string().contains("function"), "{error}");
    }

    #[test]
    fn rejects_an_integer_where_a_callback_belongs() {
        let error =
            deserialize::<Handlers>("return { on_key = 7 }").expect_err("7 is not a function");
        assert!(error.to_string().contains("expected a function"), "{error}");
    }

    #[derive(Deserialize)]
    struct Named {
        name: String,
    }

    #[test]
    fn ignores_an_unknown_field_holding_a_function() {
        deserialize_with(
            r#"return { name = "dome", extra = function() end }"#,
            |named: mlua::Result<Named>, callbacks| {
                assert_eq!(named.expect("named should deserialize").name, "dome");
                assert!(callbacks.is_empty());
            },
        );
    }

    #[test]
    fn rejects_a_self_referential_table() {
        let error = deserialize::<serde_json::Value>("local t = { n = 1 } t.me = t return t")
            .expect_err("a cycle cannot deserialize");
        assert!(error.to_string().contains("recursive table"), "{error}");
    }

    #[derive(Deserialize)]
    struct Inner {
        value: u32,
    }

    #[derive(Deserialize)]
    struct Pair {
        first: Inner,
        second: Inner,
    }

    #[test]
    fn shares_a_table_between_two_fields() {
        let pair: Pair =
            deserialize("local shared = { value = 4 } return { first = shared, second = shared }")
                .expect("a table on two fields is not a cycle");
        assert_eq!(pair.first.value, 4);
        assert_eq!(pair.second.value, 4);
    }

    #[derive(Debug, Deserialize)]
    enum Chain {
        #[expect(dead_code, reason = "the payload only has to make the type recursive")]
        Next(Box<Chain>),
    }

    /// A variant table descends through `deserialize_enum` alone, so without a
    /// guard there this recurses until the stack ends.
    #[test]
    fn rejects_a_self_referential_variant_table() {
        let error = deserialize::<Chain>("local t = {} t.Next = t return t")
            .expect_err("a variant cycle cannot deserialize");
        assert!(error.to_string().contains("recursive table"), "{error}");
    }

    /// Keeps the entries that deserialize and skips the rest, which only works
    /// because `TableAccess` survives an entry error.
    struct Survivors(Vec<u32>);

    impl<'de> Deserialize<'de> for Survivors {
        fn deserialize<D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> std::result::Result<Self, D::Error> {
            struct SurvivorsVisitor;

            impl<'de> Visitor<'de> for SurvivorsVisitor {
                type Value = Survivors;

                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a table of numbers")
                }

                fn visit_map<A: de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> std::result::Result<Survivors, A::Error> {
                    let mut kept = Vec::new();
                    loop {
                        match map.next_key::<String>() {
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(_) => continue,
                        }
                        if let Ok(value) = map.next_value::<u32>() {
                            kept.push(value);
                        }
                    }
                    kept.sort_unstable();
                    Ok(Survivors(kept))
                }
            }

            deserializer.deserialize_map(SurvivorsVisitor)
        }
    }

    #[test]
    fn map_access_continues_after_an_entry_error() {
        let survivors: Survivors =
            deserialize(r#"return { a = 1, b = "not a number", c = 3, d = function() end }"#)
                .expect("the surviving entries should deserialize");
        assert_eq!(survivors.0, [1, 3]);
    }

    #[derive(Deserialize)]
    enum Handler {
        Direct(CallbackId),
    }

    #[test]
    fn deserializes_an_enum_variant_carrying_a_callback() {
        deserialize_with(
            "return { Direct = function() return 9 end }",
            |handler: mlua::Result<Handler>, callbacks| {
                let Handler::Direct(id) = handler.expect("the variant should deserialize");
                let value: i64 = callbacks[id.0].call(()).expect("the callback should call");
                assert_eq!(value, 9);
            },
        );
    }
}
