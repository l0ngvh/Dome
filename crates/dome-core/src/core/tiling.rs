use serde::Serialize;

use crate::config::lua::deserializer::{FromLuaValue, LoadContext, string_enum, type_error};
use crate::core::node::pixels_from_lua_number;
use crate::core::{FractionalLength, Length, Logical, Unit};

use super::master::MasterConfig;
use super::matcher::WindowMatcher;
use super::partition_tree::PartitionTreeConfig;
use super::scrolling::ScrollingConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct TilingConfig {
    pub layout: Strategy,
    pub border_size: Length<Logical>,
    pub partition_tree: PartitionTreeConfig,
    pub master: MasterConfig,
    pub(crate) scrolling: ScrollingConfig,
    pub size_constraints: SizeConstraints,
    pub ignore: Vec<WindowMatcher>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Strategy {
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
pub struct SizeConstraints {
    pub minimum_width: SizeConstraint,
    pub minimum_height: SizeConstraint,
    pub maximum_width: SizeConstraint,
    pub maximum_height: SizeConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
    Pixels(Length<Logical>),
    Percent(f64),
}

impl SizeConstraint {
    pub fn resolve(
        &self,
        screen_size: FractionalLength<Unit>,
        scale: f64,
    ) -> FractionalLength<Unit> {
        match self {
            SizeConstraint::Pixels(px) => FractionalLength::from_pixels(*px).to_unit(scale),
            SizeConstraint::Percent(pct) => screen_size * (pct / 100.0),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            SizeConstraint::Pixels(px) => px.value().to_string(),
            SizeConstraint::Percent(pct) => format!("{pct}%"),
        }
    }

    pub fn is_unlimited(&self) -> bool {
        match self {
            SizeConstraint::Pixels(px) => *px <= Length::ZERO,
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
                let percent: f64 = percent
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
