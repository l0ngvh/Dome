use serde::Serialize;

use crate::config::lua::deserializer::{FromLuaValue, LoadContext, string_enum, type_error};
use crate::core::node::pixels_from_lua_number;
use crate::core::{Length, Logical, Pixels, Unit};

use super::master::MasterConfig;
use super::matcher::WindowMatcher;
use super::partition_tree::PartitionTreeConfig;
use super::scrolling::ScrollingConfig;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TilingConfig {
    pub(crate) layout: Strategy,
    pub(crate) border_size: Pixels<Logical>,
    pub(crate) partition_tree: PartitionTreeConfig,
    pub(crate) master: MasterConfig,
    pub(crate) scrolling: ScrollingConfig,
    pub(crate) size_constraints: SizeConstraints,
    pub(crate) ignore: Vec<WindowMatcher>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum Strategy {
    PartitionTree,
    Master,
    Scrolling,
}

string_enum!(
    Strategy,
    "\"partition_tree\", \"master\" or \"scrolling\"",
    "partition_tree" => Strategy::PartitionTree,
    "master" => Strategy::Master,
    "scrolling" => Strategy::Scrolling,
);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SizeConstraints {
    pub(crate) minimum_width: SizeConstraint,
    pub(crate) minimum_height: SizeConstraint,
    pub(crate) maximum_width: SizeConstraint,
    pub(crate) maximum_height: SizeConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizeConstraint {
    Pixels(Pixels<Logical>),
    Percent(f32),
}

impl SizeConstraint {
    pub(crate) fn resolve(&self, screen_size: Length<Unit>, scale: f32) -> Length<Unit> {
        match self {
            SizeConstraint::Pixels(px) => Length::from_pixels(*px).to_unit(scale),
            SizeConstraint::Percent(pct) => screen_size * (pct / 100.0),
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            SizeConstraint::Pixels(px) => px.value().to_string(),
            SizeConstraint::Percent(pct) => format!("{pct}%"),
        }
    }

    pub(crate) fn is_unlimited(&self) -> bool {
        match self {
            SizeConstraint::Pixels(px) => *px <= Pixels::ZERO,
            SizeConstraint::Percent(pct) => *pct <= 0.0,
        }
    }
}

impl FromLuaValue for SizeConstraint {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        match value {
            mlua::Value::Integer(_) | mlua::Value::Number(_) => {
                let n = f64::from_lua_value(value, cx)?;
                pixels_from_lua_number(n).map(SizeConstraint::Pixels)
            }
            mlua::Value::String(s) => {
                let text = s.to_str()?;
                let Some(percent) = text.strip_suffix('%') else {
                    return Err(mlua::Error::runtime(
                        "a string must be a percentage, such as \"10%\"",
                    ));
                };
                let percent: f32 = percent
                    .trim()
                    .parse()
                    .map_err(|_| mlua::Error::runtime(format!("{text} is not a percentage")))?;
                if !(0.0..=100.0).contains(&percent) {
                    return Err(mlua::Error::runtime(
                        "a percentage must be between 0 and 100",
                    ));
                }
                Ok(SizeConstraint::Percent(percent))
            }
            _ => type_error(
                "a whole number of pixels or a percentage string such as \"10%\"",
                value,
            ),
        }
    }
}
