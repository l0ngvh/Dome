//! Reads a loaded `mlua::Value` into Dome's config types.
//!
//! A field that fails to convert does not fail the load. `LoadContext::field`
//! logs the field's path and substitutes the default, so one bad value costs
//! that value rather than the whole config. Only a structural failure, such as
//! a chunk that returns something other than a table, reaches the caller as an
//! error.

use std::collections::BTreeMap;
use std::collections::HashMap;

/// Caps how deep a config may nest. A table that contains itself would
/// otherwise recurse until the stack runs out.
const MAX_DEPTH: usize = 64;

/// The largest whole number an f64 holds exactly, 2^53 - 1. Lua stores every
/// number as an f64, so a count above this arrives already rounded.
const MAX_WHOLE_NUMBER: f64 = 9_007_199_254_740_991.0;

pub(crate) trait FromLuaValue: Sized {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self>;
}

/// Tracks where the reader is inside the config, so a warning can name the
/// field rather than just its type.
#[derive(Default)]
pub(crate) struct LoadContext {
    path: Vec<String>,
}

impl LoadContext {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn field<T: FromLuaValue + Default>(&mut self, table: &mlua::Table, key: &str) -> T {
        self.field_or_else(table, key, T::default)
    }

    pub(crate) fn field_or_else<T: FromLuaValue>(
        &mut self,
        table: &mlua::Table,
        key: &str,
        default: impl FnOnce() -> T,
    ) -> T {
        let value = match table.get::<mlua::Value>(key) {
            Ok(value) => value,
            Err(e) => {
                self.push(key);
                self.warn_value(&e.to_string());
                self.pop();
                return default();
            }
        };
        // An absent key takes the default silently. Only a present, unusable
        // value is worth a warning.
        if value.is_nil() {
            return default();
        }
        self.read(key, &value, default)
    }

    /// Converts `value` as if it sat at `key`, for a caller that already holds
    /// the value.
    pub(crate) fn read<T: FromLuaValue>(
        &mut self,
        key: &str,
        value: &mlua::Value,
        default: impl FnOnce() -> T,
    ) -> T {
        self.push(key);
        let out = if self.depth_exceeded() {
            self.warn_value("nests too deeply, which a table cycle can cause");
            default()
        } else {
            match T::from_lua_value(value, self) {
                Ok(out) => out,
                Err(e) => {
                    self.warn_value(&e.to_string());
                    default()
                }
            }
        };
        self.pop();
        out
    }

    pub(crate) fn depth_exceeded(&self) -> bool {
        self.path.len() > MAX_DEPTH
    }

    pub(crate) fn warn_value(&self, detail: &str) {
        tracing::warn!(
            field = %self.field_path(),
            detail = %detail,
            "Invalid config value, using default"
        );
    }

    /// A collection has no slot worth filling with a default, so a bad entry
    /// leaves the collection instead.
    pub(crate) fn warn_dropped(&self, detail: &str) {
        tracing::warn!(
            field = %self.field_path(),
            detail = %detail,
            "Invalid config entry, dropping"
        );
    }

    pub(crate) fn push(&mut self, segment: &str) {
        self.path.push(segment.to_string());
    }

    pub(crate) fn pop(&mut self) {
        self.path.pop();
    }

    pub(crate) fn field_path(&self) -> String {
        self.path.join(".")
    }
}

/// A Lua table holds a list part and a named part at once. A field reads only
/// one part, so the warning names the entries the field ignores.
pub(crate) fn warn_if_shape_mismatched(table: &mlua::Table, wanted: Shape, cx: &LoadContext) {
    let sequence = table.raw_len();
    let total = table.pairs::<mlua::Value, mlua::Value>().flatten().count();
    let keyed = total.saturating_sub(sequence);
    match wanted {
        Shape::List if keyed > 0 => cx.warn_dropped(&format!(
            "expected a list of entries, got {keyed} named field(s), which a list cannot hold"
        )),
        Shape::Map if sequence > 0 => cx.warn_dropped(&format!(
            "expected named fields, got {sequence} list entry(ies), which named fields cannot hold"
        )),
        _ => {}
    }
}

pub(crate) enum Shape {
    List,
    Map,
}

pub(crate) fn type_error<T>(expected: &str, value: &mlua::Value) -> mlua::Result<T> {
    Err(mlua::Error::runtime(format!(
        "expected {expected}, got {}",
        value.type_name()
    )))
}

pub(crate) fn as_table<'v>(
    value: &'v mlua::Value,
    expected: &str,
) -> mlua::Result<&'v mlua::Table> {
    match value {
        mlua::Value::Table(table) => Ok(table),
        _ => type_error(expected, value),
    }
}

fn as_f64(value: &mlua::Value, expected: &str) -> mlua::Result<f64> {
    // Deliberately no string coercion. Lua would accept "24" as a number, which
    // hides a quoted config value instead of reporting it.
    let n = match value {
        mlua::Value::Integer(i) => *i as f64,
        mlua::Value::Number(n) => *n,
        _ => return type_error(expected, value),
    };
    if !n.is_finite() {
        return Err(mlua::Error::runtime("expected a finite number"));
    }
    Ok(n)
}

impl FromLuaValue for bool {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        match value {
            mlua::Value::Boolean(b) => Ok(*b),
            _ => type_error("a boolean", value),
        }
    }
}

impl FromLuaValue for String {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => Ok(s.to_str()?.to_owned()),
            _ => type_error("a string", value),
        }
    }
}

impl FromLuaValue for f64 {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        as_f64(value, "a number")
    }
}

impl FromLuaValue for f32 {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        let n = as_f64(value, "a number")? as f32;
        // A finite f64 too large for f32 casts to infinity.
        if !n.is_finite() {
            return Err(mlua::Error::runtime("expected a number an f32 can hold"));
        }
        Ok(n)
    }
}

impl FromLuaValue for usize {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        let n = as_f64(value, "a whole number")?;
        // An `as usize` cast saturates, so an out-of-range value would arrive as
        // `usize::MAX` and pass every later bound check.
        if n.fract() != 0.0 || !(0.0..=MAX_WHOLE_NUMBER).contains(&n) {
            return Err(mlua::Error::runtime(format!(
                "expected a non-negative whole number no greater than {MAX_WHOLE_NUMBER}"
            )));
        }
        Ok(n as usize)
    }
}

impl FromLuaValue for mlua::Function {
    fn from_lua_value(value: &mlua::Value, _cx: &mut LoadContext) -> mlua::Result<Self> {
        match value {
            mlua::Value::Function(f) => Ok(f.clone()),
            _ => type_error("a function", value),
        }
    }
}

impl<T: FromLuaValue> FromLuaValue for Option<T> {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        if value.is_nil() {
            return Ok(None);
        }
        T::from_lua_value(value, cx).map(Some)
    }
}

impl<T: FromLuaValue> FromLuaValue for Vec<T> {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a list")?;
        warn_if_shape_mismatched(table, Shape::List, cx);
        let len = table.raw_len();
        let mut out = Vec::with_capacity(len);
        for i in 1..=len {
            let item: mlua::Value = table.raw_get(i)?;
            cx.push(&format!("[{i}]"));
            if cx.depth_exceeded() {
                cx.warn_dropped(&format!(
                    "nests too deeply, which a table cycle can cause, so this and the {} after it are dropped",
                    len - i
                ));
                cx.pop();
                break;
            }
            match T::from_lua_value(&item, cx) {
                Ok(item) => out.push(item),
                Err(e) => cx.warn_dropped(&e.to_string()),
            }
            cx.pop();
        }
        Ok(out)
    }
}

impl<T: FromLuaValue> FromLuaValue for HashMap<String, T> {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        warn_if_shape_mismatched(table, Shape::Map, cx);
        let mut out = HashMap::new();
        for pair in table.pairs::<mlua::Value, mlua::Value>() {
            let (key, item) = pair?;
            let Ok(key) = String::from_lua_value(&key, cx) else {
                let label = match &key {
                    mlua::Value::Integer(i) => i.to_string(),
                    mlua::Value::Number(n) => n.to_string(),
                    mlua::Value::Boolean(b) => b.to_string(),
                    other => other.type_name().to_string(),
                };
                let reason = match &key {
                    mlua::Value::String(_) => "key is a string that is not valid UTF-8".to_string(),
                    other => format!("key must be a string, got {}", other.type_name()),
                };
                cx.push(&format!("[{label}]"));
                cx.warn_dropped(&reason);
                cx.pop();
                continue;
            };
            cx.push(&key);
            if cx.depth_exceeded() {
                cx.warn_dropped(
                    "nests too deeply, which a table cycle can cause, so this and every remaining key are dropped",
                );
                cx.pop();
                break;
            }
            match T::from_lua_value(&item, cx) {
                Ok(item) => {
                    out.insert(key, item);
                }
                Err(e) => cx.warn_dropped(&e.to_string()),
            }
            cx.pop();
        }
        Ok(out)
    }
}

impl<T: FromLuaValue> FromLuaValue for BTreeMap<String, T> {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let entries = HashMap::<String, T>::from_lua_value(value, cx)?;
        Ok(entries.into_iter().collect())
    }
}

macro_rules! string_enum {
    ($type:ty, $expected:literal, $($name:literal => $variant:expr),* $(,)?) => {
        impl crate::config::lua::deserializer::FromLuaValue for $type {
            fn from_lua_value(
                value: &mlua::Value,
                cx: &mut crate::config::lua::deserializer::LoadContext,
            ) -> mlua::Result<Self> {
                let name = <String as crate::config::lua::deserializer::FromLuaValue>::from_lua_value(
                    value, cx,
                )?;
                match name.as_str() {
                    $($name => Ok($variant),)*
                    other => Err(mlua::Error::runtime(format!(
                        "expected {}, got \"{other}\"", $expected
                    ))),
                }
            }
        }
    };
}

pub(crate) use string_enum;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{TreeLayoutNode, WindowMatcher};

    fn read<T: FromLuaValue>(src: &str) -> (mlua::Result<T>, LoadContext) {
        let lua = mlua::Lua::new();
        let value: mlua::Value = lua.load(src).eval().expect("the chunk should evaluate");
        let mut cx = LoadContext::new();
        let out = T::from_lua_value(&value, &mut cx);
        (out, cx)
    }

    fn ok<T: FromLuaValue>(src: &str) -> T {
        read(src).0.expect("the value should convert")
    }

    #[test]
    fn reads_scalar_leaves() {
        assert!(ok::<bool>("return true"));
        assert!(!ok::<bool>("return false"));
        assert_eq!(ok::<String>("return 'dome'"), "dome");
        assert_eq!(ok::<f64>("return 0.5"), 0.5);
        assert_eq!(ok::<f32>("return 3"), 3.0);
        assert_eq!(ok::<usize>("return 7"), 7);
    }

    /// Lua would read "24" as a number, which would hide a quoted config value.
    #[test]
    fn refuses_to_coerce_a_string_into_a_number() {
        assert!(read::<f64>("return '24'").0.is_err());
        assert!(read::<usize>("return '24'").0.is_err());
    }

    #[test]
    fn refuses_a_non_finite_number() {
        for src in ["return 0/0", "return math.huge", "return -math.huge"] {
            assert!(read::<f64>(src).0.is_err(), "{src}");
            assert!(read::<f32>(src).0.is_err(), "{src}");
            assert!(read::<usize>(src).0.is_err(), "{src}");
        }
    }

    #[test]
    fn refuses_a_fractional_or_negative_whole_number() {
        assert!(read::<usize>("return 1.5").0.is_err());
        assert!(read::<usize>("return -1").0.is_err());
    }

    #[test]
    fn reads_a_list_of_structs_and_an_empty_table() {
        let rules: Vec<WindowMatcher> = ok("return { { app = 'a' }, { app = 'b' } }");
        let names: Vec<_> = rules.iter().map(|r| r.app.clone().unwrap()).collect();
        assert_eq!(names, ["a", "b"]);
        assert!(ok::<Vec<WindowMatcher>>("return {}").is_empty());
    }

    #[test]
    fn reads_a_callable_function() {
        // The VM has to outlive the call, because a stored `Function` holds only
        // a weak reference and panics once its VM drops.
        let lua = mlua::Lua::new();
        let value: mlua::Value = lua
            .load("return function() return 7 end")
            .eval()
            .expect("the chunk should evaluate");
        let mut cx = LoadContext::new();
        let function =
            mlua::Function::from_lua_value(&value, &mut cx).expect("a function should convert");
        let result: i64 = function.call(()).expect("the function should call");
        assert_eq!(result, 7);
    }

    #[test]
    fn refuses_a_function_where_a_scalar_belongs() {
        assert!(read::<f64>("return function() end").0.is_err());
    }

    #[test]
    fn refuses_a_number_where_a_function_belongs() {
        assert!(read::<mlua::Function>("return 7").0.is_err());
    }

    #[test]
    fn ignores_an_unknown_key() {
        let matcher: WindowMatcher = ok("return { app = 'a', extra = function() end }");
        assert_eq!(matcher.app.as_deref(), Some("a"));
    }

    #[test]
    fn keeps_reading_a_list_after_a_bad_element() {
        let rules: Vec<WindowMatcher> = ok("return { { app = 'a' }, 5, { app = 'c' } }");
        let names: Vec<_> = rules.iter().map(|r| r.app.clone().unwrap()).collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn keeps_reading_a_map_after_a_bad_entry() {
        let entries: HashMap<String, String> =
            ok("return { keep = 'yes', drop = 5, also = 'yes' }");
        let mut kept: Vec<_> = entries.keys().cloned().collect();
        kept.sort();
        assert_eq!(kept, ["also", "keep"]);
    }

    #[test]
    fn a_table_on_two_fields_is_not_a_cycle() {
        let rules: Vec<WindowMatcher> =
            ok("local shared = { app = 'a' } return { shared, shared }");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], rules[1]);
    }

    #[test]
    fn a_self_referential_table_stops_at_the_depth_cap() {
        let node: TreeLayoutNode = ok("local t = { children = {} } t.children[1] = t return t");
        assert!(tree_depth(&node) <= MAX_DEPTH);
    }

    fn tree_depth(node: &TreeLayoutNode) -> usize {
        match node {
            TreeLayoutNode::Leaf(_) => 1,
            TreeLayoutNode::Container { children, .. } => {
                1 + children.iter().map(tree_depth).max().unwrap_or(0)
            }
        }
    }
}
