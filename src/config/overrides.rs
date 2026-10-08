//! Reads a config table into a `Config`, taking each key the table leaves out
//! or gets wrong from the bundled defaults.

use super::lua::deserializer::{FromLuaValue, LoadContext, as_table};
use super::{Appearance, Config};
use crate::core::{
    Logical, MasterConfig, PartitionTreeConfig, Pixels, ScrollingConfig, SizeConstraint,
    SizeConstraints, TilingConfig, read_master_count_override, read_master_ratio_override,
};
use crate::font::{FontConfig, MAX_FONT_SIZE, MIN_FONT_SIZE};
use std::collections::HashMap;

/// `defaults` must come from the same VM as `user`, because a default binding
/// is a function that dies with the VM that built it.
pub(super) fn read_config(
    user: &mlua::Value,
    defaults: &mlua::Value,
    cx: &mut LoadContext,
) -> mlua::Result<Config> {
    let user = as_table(user, "a config table")?;
    let defaults = as_table(defaults, "a config table")?;
    Ok(Config {
        // A present table replaces the bundled keymaps rather than merging, so a
        // binding the user set to nil stays gone.
        keymaps: user_or_default(cx, user, defaults, "keymaps")?,
        reserved_area: user_or_default(cx, user, defaults, "reserved_area")?,
        tiling: TilingConfig {
            layout: user_or_default(cx, user, defaults, "layout")?,
            border_size: user_or_default(cx, user, defaults, "border_size")?,
            partition_tree: read_group(cx, user, defaults, "partition_tree", read_partition_tree)?,
            master: read_group(cx, user, defaults, "master", read_master)?,
            scrolling: read_group(cx, user, defaults, "scrolling", read_scrolling)?,
            size_constraints: read_size_constraints(cx, user, defaults)?,
            ignore: user_or_default(cx, user, defaults, "ignore")?,
        },
        appearance: Appearance {
            theme: user_or_default(cx, user, defaults, "theme")?,
            font: FontConfig {
                size: user_or_default_with(cx, user, defaults, "font_size", read_font_size)?,
                // A nil default leaves the key out of the table, so default.lua
                // cannot be required to set this one.
                family: read_font_family(user, cx).or_else(|| read_font_family(defaults, cx)),
            },
        },
        log_level: user_or_default(cx, user, defaults, "log_level")?,
        start_at_login: user_or_default(cx, user, defaults, "start_at_login")?,
        env: user_or_default_with(cx, user, defaults, "env", read_env)?,
    })
}

/// Fails the load when default.lua does not set `key` either.
fn user_or_default<T: FromLuaValue>(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
    key: &str,
) -> mlua::Result<T> {
    user_or_default_with(cx, user, defaults, key, |table, cx| cx.field(table, key))
}

/// `user_or_default` for a key whose reader also checks the value it reads.
fn user_or_default_with<T>(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
    key: &str,
    read: impl Fn(&mlua::Table, &mut LoadContext) -> Option<T>,
) -> mlua::Result<T> {
    match read(user, cx) {
        Some(value) => Ok(value),
        None => read(defaults, cx).ok_or_else(|| missing_default(cx, key)),
    }
}

fn required<T: FromLuaValue>(
    cx: &mut LoadContext,
    defaults: &mlua::Table,
    key: &str,
) -> mlua::Result<T> {
    let value: Option<T> = cx.field(defaults, key);
    value.ok_or_else(|| missing_default(cx, key))
}

/// default.lua is the last place a value can come from, so a key it leaves out
/// fails the load.
fn missing_default(cx: &mut LoadContext, key: &str) -> mlua::Error {
    cx.push(key);
    let path = cx.field_path();
    cx.pop();
    mlua::Error::runtime(format!("default.lua must set {path}"))
}

/// A present user group falls back to the bundled group key by key. A user
/// group that is missing or is not a table reads as the bundled group.
fn read_group<T>(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
    key: &str,
    read: impl FnOnce(&mut LoadContext, &mlua::Table, &mlua::Table) -> mlua::Result<T>,
) -> mlua::Result<T> {
    let bundled: mlua::Table = required(cx, defaults, key)?;
    let user: Option<mlua::Table> = cx.field(user, key);
    let user = user.unwrap_or_else(|| bundled.clone());
    cx.push(key);
    let out = read(cx, &user, &bundled);
    cx.pop();
    out
}

fn read_partition_tree(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
) -> mlua::Result<PartitionTreeConfig> {
    Ok(PartitionTreeConfig {
        tab_bar_height: user_or_default_with(
            cx,
            user,
            defaults,
            "tab_bar_height",
            read_tab_bar_height,
        )?,
        automatic_tiling: user_or_default(cx, user, defaults, "automatic_tiling")?,
    })
}

fn read_master(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
) -> mlua::Result<MasterConfig> {
    Ok(MasterConfig {
        master_ratio: user_or_default_with(
            cx,
            user,
            defaults,
            "master_ratio",
            read_master_ratio_override,
        )?,
        master_count: user_or_default_with(
            cx,
            user,
            defaults,
            "master_count",
            read_master_count_override,
        )?,
    })
}

fn read_scrolling(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
) -> mlua::Result<ScrollingConfig> {
    Ok(ScrollingConfig {
        column_width: user_or_default(cx, user, defaults, "column_width")?,
    })
}

/// The pair check runs after the reads, because a minimum and its maximum can
/// come from different tables.
fn read_size_constraints(
    cx: &mut LoadContext,
    user: &mlua::Table,
    defaults: &mlua::Table,
) -> mlua::Result<SizeConstraints> {
    let mut out = SizeConstraints {
        minimum_width: user_or_default(cx, user, defaults, "minimum_width")?,
        minimum_height: user_or_default(cx, user, defaults, "minimum_height")?,
        maximum_width: user_or_default(cx, user, defaults, "maximum_width")?,
        maximum_height: user_or_default(cx, user, defaults, "maximum_height")?,
    };
    let bundled = SizeConstraints {
        minimum_width: required(cx, defaults, "minimum_width")?,
        minimum_height: required(cx, defaults, "minimum_height")?,
        maximum_width: required(cx, defaults, "maximum_width")?,
        maximum_height: required(cx, defaults, "maximum_height")?,
    };
    reconcile_pair(
        cx,
        "width",
        &mut out.minimum_width,
        &mut out.maximum_width,
        bundled.minimum_width,
        bundled.maximum_width,
    );
    reconcile_pair(
        cx,
        "height",
        &mut out.minimum_height,
        &mut out.maximum_height,
        bundled.minimum_height,
        bundled.maximum_height,
    );
    Ok(out)
}

fn read_tab_bar_height(table: &mlua::Table, cx: &mut LoadContext) -> Option<Pixels<Logical>> {
    let height: Option<Pixels<Logical>> = cx.field(table, "tab_bar_height");
    let height = height?;
    if height > Pixels::ZERO {
        return Some(height);
    }
    cx.push("tab_bar_height");
    cx.warn_value("must be greater than zero");
    cx.pop();
    None
}

fn read_font_size(table: &mlua::Table, cx: &mut LoadContext) -> Option<f32> {
    let size: Option<f32> = cx.field(table, "font_size");
    let size = size?;
    if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&size) {
        return Some(size);
    }
    cx.push("font_size");
    cx.warn_value(&format!(
        "must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE}, got {size}"
    ));
    cx.pop();
    None
}

fn read_font_family(table: &mlua::Table, cx: &mut LoadContext) -> Option<String> {
    let family: Option<String> = cx.field(table, "font_family");
    let name = family?;
    if !name.trim().is_empty() {
        return Some(name);
    }
    cx.push("font_family");
    cx.warn_value("must not be blank");
    cx.pop();
    None
}

/// Drops each entry whose name or value cannot become `KEY=VALUE`, warning
/// about each one. The `HashMap` read has already dropped a non-string value.
fn read_env(table: &mlua::Table, cx: &mut LoadContext) -> Option<HashMap<String, String>> {
    let raw: Option<HashMap<String, String>> = cx.field(table, "env");
    let raw = raw?;
    cx.push("env");
    let mut valid = HashMap::new();
    for (name, value) in raw {
        match env_entry_error(&name, &value) {
            Some(reason) => {
                cx.push(&name);
                cx.warn_dropped(reason);
                cx.pop();
            }
            None => {
                valid.insert(environment_variable_name(&name), value);
            }
        }
    }
    cx.pop();
    Some(valid)
}

fn env_entry_error(name: &str, value: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("name must not be empty")
    } else if name.contains('=') {
        Some("name must not contain '='")
    } else if name.contains('\0') {
        Some("name must not contain a NUL byte")
    } else if value.contains('\0') {
        Some("value must not contain a NUL byte")
    } else {
        None
    }
}

/// `name` in uppercase on Windows, where a name is case-insensitive, and
/// unchanged elsewhere.
pub(crate) fn environment_variable_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        name.to_uppercase()
    } else {
        name.to_string()
    }
}

/// A zero maximum means unlimited, so only a positive maximum can conflict. The
/// pair is what is inconsistent, so both ends revert rather than one being
/// picked as the wrong one.
fn reconcile_pair(
    cx: &mut LoadContext,
    axis: &str,
    min: &mut SizeConstraint,
    max: &mut SizeConstraint,
    default_min: SizeConstraint,
    default_max: SizeConstraint,
) {
    if max.is_unlimited() {
        return;
    }
    let ordered = match (*min, *max) {
        (SizeConstraint::Pixels(min_px), SizeConstraint::Pixels(max_px)) => min_px <= max_px,
        (SizeConstraint::Percent(min_pct), SizeConstraint::Percent(max_pct)) => min_pct <= max_pct,
        _ => {
            cx.push(&format!("minimum_{axis}"));
            cx.warn_value(&format!(
                "{} and maximum_{axis} {} mix pixels with a percentage, so their order \
                 depends on the screen size and both stand unchecked",
                min.describe(),
                max.describe()
            ));
            cx.pop();
            return;
        }
    };
    if ordered {
        return;
    }
    cx.push(&format!("minimum_{axis}"));
    cx.warn_value(&format!(
        "{} exceeds maximum_{axis} {}, reverting both to their defaults",
        min.describe(),
        max.describe()
    ));
    cx.pop();
    *min = default_min;
    *max = default_max;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_default_key_is_an_error() {
        let vm = crate::config::lua::new_vm().expect("the VM should build");
        let user: mlua::Value = vm.load("return {}").eval().expect("the user chunk");
        let defaults: mlua::Value = vm
            .load("local c = dome.defaults() c.border_size = nil return c")
            .eval()
            .expect("the defaults chunk");
        let error = read_config(&user, &defaults, &mut LoadContext::new())
            .expect_err("a missing default should fail the read");
        assert!(error.to_string().contains("border_size"), "{error}");
    }
}
