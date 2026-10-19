use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table, string_enum};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct WindowMatcher {
    pub app: Option<String>,
    pub bundle_id: Option<String>,
    pub title: Option<String>,
    pub process: Option<String>,
    pub class: Option<String>,
    pub aumid: Option<String>,
}

impl FromLuaValue for WindowMatcher {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a window matcher table")?;
        Ok(WindowMatcher {
            app: read_pattern(table, "app", cx),
            bundle_id: read_pattern(table, "bundle_id", cx),
            title: read_pattern(table, "title", cx),
            process: read_pattern(table, "process", cx),
            class: read_pattern(table, "class", cx),
            aumid: read_pattern(table, "aumid", cx),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WindowMode {
    Tiling,
    Float,
    Fullscreen,
}

string_enum!(
    WindowMode,
    "\"tiling\", \"float\" or \"fullscreen\"",
    "tiling" => WindowMode::Tiling,
    "float" => WindowMode::Float,
    "fullscreen" => WindowMode::Fullscreen,
);

pub fn pattern_matches(pattern: &str, text: &str) -> bool {
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        regex::Regex::new(regex)
            .map(|r| r.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
}

fn read_pattern(table: &mlua::Table, key: &str, cx: &mut LoadContext) -> Option<String> {
    let pattern: Option<String> = cx.field(table, key);
    let pattern = pattern?;
    if let Some(regex) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix('/'))
        && let Err(e) = regex::Regex::new(regex)
    {
        cx.push(key);
        cx.warn_value(&format!("is not a valid regex: {e}"));
        cx.pop();
        return None;
    }
    Some(pattern)
}
