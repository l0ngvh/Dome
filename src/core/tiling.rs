use serde::Serialize;

use crate::config::lua::deserializer::{FromLuaValue, LoadContext, string_enum, type_error};
use crate::core::node::pixels_from_lua_number;
use crate::core::{Length, Logical, Pixels, Unit};

use super::master::MasterConfig;
use super::matcher::WindowMatcher;
use super::partition_tree::PartitionTreeConfig;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TilingConfig {
    pub(crate) layout: Strategy,
    pub(crate) border_size: Pixels<Logical>,
    pub(crate) partition_tree: PartitionTreeConfig,
    pub(crate) master: MasterConfig,
    pub(crate) size_constraints: SizeConstraints,
    pub(crate) float: Vec<WindowMatcher>,
    pub(crate) fullscreen: Vec<WindowMatcher>,
    pub(crate) ignore: Vec<WindowMatcher>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum Strategy {
    PartitionTree,
    Master,
}

string_enum!(
    Strategy,
    "\"partition_tree\" or \"master\"",
    "partition_tree" => Strategy::PartitionTree,
    "master" => Strategy::Master,
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
    /// `Pixels` holds config-logical values and needs `scale` to reach frame
    /// units. `Percent` is relative to `screen_size`, already in frame units.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_constraint_resolve() {
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.0)
                .value(),
            200.0
        );
        // On macOS (Unit = Logical), to_unit is identity so scale does not affect
        // Pixels. On Windows (Unit = Physical), scale multiplies through.
        #[cfg(target_os = "windows")]
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            300.0
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            SizeConstraint::Pixels(Pixels::new(200))
                .resolve(Length::new(1000.0), 1.5)
                .value(),
            200.0
        );
        assert_eq!(
            SizeConstraint::Percent(10.0)
                .resolve(Length::new(1000.0), 1.0)
                .value(),
            100.0
        );
        assert_eq!(
            SizeConstraint::Percent(10.0)
                .resolve(Length::new(1000.0), 2.0)
                .value(),
            100.0
        );
        assert_eq!(
            SizeConstraint::Percent(5.0)
                .resolve(Length::new(1920.0), 1.0)
                .value(),
            96.0
        );
    }
}
