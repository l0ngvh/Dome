//! The shape a config file parses into, before it merges over the bundled
//! defaults.

use super::defaults::DefaultValues;
use super::lua::deserializer::{FromLuaValue, LoadContext, as_table};
use super::{Appearance, Config, LogLevel, ModalKeymaps};
use crate::core::{
    Logical, MasterConfig, PartitionTreeConfig, Pixels, ScrollingConfig, SizeConstraint,
    SizeConstraints, Strategy, TilingConfig, WindowMatcher, read_master_count_override,
    read_master_ratio_override,
};
use crate::font::{FontConfig, MAX_FONT_SIZE, MIN_FONT_SIZE};
use crate::theme::Flavor;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ConfigOverrides {
    /// A present table replaces the bundled keymaps rather than merging, so a
    /// binding the user set to nil stays gone.
    keymaps: Option<ModalKeymaps>,
    layout: Option<Strategy>,
    border_size: Option<Pixels<Logical>>,
    partition_tree: Option<PartitionTreeOverrides>,
    master: Option<MasterOverrides>,
    scrolling: Option<ScrollingOverrides>,
    size_constraints: SizeConstraintOverrides,
    float: Option<Vec<WindowMatcher>>,
    fullscreen: Option<Vec<WindowMatcher>>,
    ignore: Option<Vec<WindowMatcher>>,
    theme: Option<Flavor>,
    font: FontOverrides,
    log_level: Option<LogLevel>,
    start_at_login: Option<bool>,
    env: HashMap<String, String>,
}

#[derive(Default)]
pub(super) struct PartitionTreeOverrides {
    tab_bar_height: Option<Pixels<Logical>>,
    automatic_tiling: Option<bool>,
}

#[derive(Default)]
pub(super) struct MasterOverrides {
    master_ratio: Option<f32>,
    master_count: Option<usize>,
}

#[derive(Default)]
pub(super) struct ScrollingOverrides {
    default_column_width: Option<SizeConstraint>,
}

#[derive(Default)]
pub(super) struct SizeConstraintOverrides {
    minimum_width: Option<SizeConstraint>,
    minimum_height: Option<SizeConstraint>,
    maximum_width: Option<SizeConstraint>,
    maximum_height: Option<SizeConstraint>,
}

#[derive(Default)]
pub(super) struct FontOverrides {
    size: Option<f32>,
    family: Option<String>,
}

impl ConfigOverrides {
    pub(super) fn needs_default_keymaps(&self) -> bool {
        self.keymaps.is_none()
    }

    pub(super) fn into_keymaps(self) -> mlua::Result<ModalKeymaps> {
        require(self.keymaps, "keymaps")
    }

    pub(super) fn merge_over(
        self,
        defaults: &DefaultValues,
        default_keymaps: Option<ModalKeymaps>,
        cx: &mut LoadContext,
    ) -> Config {
        // Destructured without `..` so a field added above and left unmerged
        // fails the build rather than silently doing nothing.
        let ConfigOverrides {
            keymaps,
            layout,
            border_size,
            partition_tree,
            master,
            scrolling,
            size_constraints,
            float,
            fullscreen,
            ignore,
            theme,
            font,
            log_level,
            start_at_login,
            env,
        } = self;
        let tiling = &defaults.tiling;
        Config {
            keymaps: keymaps
                .or(default_keymaps)
                .expect("keymaps must come from the user config or the bundled defaults"),
            tiling: TilingConfig {
                layout: layout.unwrap_or(tiling.layout),
                border_size: border_size.unwrap_or(tiling.border_size),
                partition_tree: partition_tree
                    .unwrap_or_default()
                    .merge_over(&tiling.partition_tree),
                master: master.unwrap_or_default().merge_over(&tiling.master),
                scrolling: scrolling.unwrap_or_default().merge_over(&tiling.scrolling),
                size_constraints: size_constraints.merge_over(&tiling.size_constraints, cx),
                float: float.unwrap_or_default(),
                fullscreen: fullscreen.unwrap_or_default(),
                ignore: ignore.unwrap_or_default(),
            },
            appearance: Appearance {
                theme: theme.unwrap_or(defaults.appearance.theme),
                font: font.merge_over(&defaults.appearance.font),
            },
            log_level: log_level.unwrap_or(defaults.log_level),
            start_at_login: start_at_login.unwrap_or(defaults.start_at_login),
            env,
        }
    }

    /// A key `default.lua` never set has nowhere left to fall back to, so it is
    /// an error here rather than a silent zero.
    pub(super) fn into_defaults(self) -> mlua::Result<DefaultValues> {
        let partition_tree = self.partition_tree.unwrap_or_default();
        let master = self.master.unwrap_or_default();
        let scrolling = self.scrolling.unwrap_or_default();
        Ok(DefaultValues {
            tiling: TilingConfig {
                layout: require(self.layout, "layout")?,
                border_size: require(self.border_size, "border_size")?,
                partition_tree: PartitionTreeConfig {
                    tab_bar_height: require(
                        partition_tree.tab_bar_height,
                        "partition_tree.tab_bar_height",
                    )?,
                    automatic_tiling: require(
                        partition_tree.automatic_tiling,
                        "partition_tree.automatic_tiling",
                    )?,
                },
                master: MasterConfig {
                    master_ratio: require(master.master_ratio, "master.master_ratio")?,
                    master_count: require(master.master_count, "master.master_count")?,
                },
                scrolling: ScrollingConfig {
                    default_column_width: require(
                        scrolling.default_column_width,
                        "scrolling.default_column_width",
                    )?,
                },
                size_constraints: SizeConstraints {
                    minimum_width: require(self.size_constraints.minimum_width, "minimum_width")?,
                    minimum_height: require(
                        self.size_constraints.minimum_height,
                        "minimum_height",
                    )?,
                    maximum_width: require(self.size_constraints.maximum_width, "maximum_width")?,
                    maximum_height: require(
                        self.size_constraints.maximum_height,
                        "maximum_height",
                    )?,
                },
                // A matcher list has no bundled default.
                float: Vec::new(),
                fullscreen: Vec::new(),
                ignore: Vec::new(),
            },
            appearance: Appearance {
                theme: require(self.theme, "theme")?,
                font: FontConfig {
                    size: require(self.font.size, "font_size")?,
                    // Lua cannot express a present key holding nil, so this one
                    // default stays in Rust.
                    family: None,
                },
            },
            log_level: require(self.log_level, "log_level")?,
            start_at_login: require(self.start_at_login, "start_at_login")?,
        })
    }
}

impl PartitionTreeOverrides {
    fn merge_over(self, defaults: &PartitionTreeConfig) -> PartitionTreeConfig {
        let PartitionTreeOverrides {
            tab_bar_height,
            automatic_tiling,
        } = self;
        PartitionTreeConfig {
            tab_bar_height: tab_bar_height.unwrap_or(defaults.tab_bar_height),
            automatic_tiling: automatic_tiling.unwrap_or(defaults.automatic_tiling),
        }
    }
}

impl MasterOverrides {
    fn merge_over(self, defaults: &MasterConfig) -> MasterConfig {
        let MasterOverrides {
            master_ratio,
            master_count,
        } = self;
        MasterConfig {
            master_ratio: master_ratio.unwrap_or(defaults.master_ratio),
            master_count: master_count.unwrap_or(defaults.master_count),
        }
    }
}

impl ScrollingOverrides {
    fn merge_over(self, defaults: &ScrollingConfig) -> ScrollingConfig {
        let ScrollingOverrides {
            default_column_width,
        } = self;
        ScrollingConfig {
            default_column_width: default_column_width.unwrap_or(defaults.default_column_width),
        }
    }
}

impl SizeConstraintOverrides {
    /// The pair check runs here rather than during the read, because a minimum
    /// and its maximum can arrive from different sources.
    fn merge_over(self, defaults: &SizeConstraints, cx: &mut LoadContext) -> SizeConstraints {
        let SizeConstraintOverrides {
            minimum_width,
            minimum_height,
            maximum_width,
            maximum_height,
        } = self;
        let mut out = SizeConstraints {
            minimum_width: minimum_width.unwrap_or(defaults.minimum_width),
            minimum_height: minimum_height.unwrap_or(defaults.minimum_height),
            maximum_width: maximum_width.unwrap_or(defaults.maximum_width),
            maximum_height: maximum_height.unwrap_or(defaults.maximum_height),
        };
        reconcile_pair(
            cx,
            "width",
            &mut out.minimum_width,
            &mut out.maximum_width,
            defaults.minimum_width,
            defaults.maximum_width,
        );
        reconcile_pair(
            cx,
            "height",
            &mut out.minimum_height,
            &mut out.maximum_height,
            defaults.minimum_height,
            defaults.maximum_height,
        );
        out
    }
}

impl FontOverrides {
    fn merge_over(self, defaults: &FontConfig) -> FontConfig {
        let FontOverrides { size, family } = self;
        FontConfig {
            size: size.unwrap_or(defaults.size),
            family: family.or_else(|| defaults.family.clone()),
        }
    }
}

impl FromLuaValue for ConfigOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a config table")?;
        Ok(ConfigOverrides {
            keymaps: cx.field(table, "keymaps"),
            layout: cx.field(table, "layout"),
            border_size: cx.field(table, "border_size"),
            partition_tree: cx.field(table, "partition_tree"),
            master: cx.field(table, "master"),
            scrolling: cx.field(table, "scrolling"),
            // These groups read the root table, because their keys take no
            // group prefix.
            size_constraints: SizeConstraintOverrides::from_lua_value(value, cx)?,
            float: cx.field(table, "float"),
            fullscreen: cx.field(table, "fullscreen"),
            ignore: cx.field(table, "ignore"),
            theme: cx.field(table, "theme"),
            font: FontOverrides::from_lua_value(value, cx)?,
            log_level: cx.field(table, "log_level"),
            start_at_login: cx.field(table, "start_at_login"),
            env: read_env(table, cx),
        })
    }
}

impl FromLuaValue for PartitionTreeOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        Ok(PartitionTreeOverrides {
            tab_bar_height: read_tab_bar_height(table, cx),
            automatic_tiling: cx.field(table, "automatic_tiling"),
        })
    }
}

impl FromLuaValue for MasterOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        Ok(MasterOverrides {
            master_ratio: read_master_ratio_override(table, cx),
            master_count: read_master_count_override(table, cx),
        })
    }
}

impl FromLuaValue for ScrollingOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        Ok(ScrollingOverrides {
            default_column_width: cx.field(table, "default_column_width"),
        })
    }
}

impl FromLuaValue for SizeConstraintOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        Ok(SizeConstraintOverrides {
            minimum_width: cx.field(table, "minimum_width"),
            minimum_height: cx.field(table, "minimum_height"),
            maximum_width: cx.field(table, "maximum_width"),
            maximum_height: cx.field(table, "maximum_height"),
        })
    }
}

impl FromLuaValue for FontOverrides {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a table")?;
        Ok(FontOverrides {
            size: read_font_size(table, cx),
            family: read_font_family(table, cx),
        })
    }
}

fn require<T>(value: Option<T>, key: &str) -> mlua::Result<T> {
    value.ok_or_else(|| mlua::Error::runtime(format!("default.lua must set {key}")))
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

/// A child process receives one entry per name, so a name that cannot become a
/// `KEY=VALUE` entry is dropped here rather than at spawn, where no diagnostic
/// would reach the user. The `HashMap` read has already dropped a non-string
/// value.
fn read_env(table: &mlua::Table, cx: &mut LoadContext) -> HashMap<String, String> {
    let raw: HashMap<String, String> = cx.field(table, "env");
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
    valid
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
