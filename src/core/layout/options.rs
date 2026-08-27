//! Scalar layout options and their serde deserialization.

use serde::{Deserialize, Deserializer, Serialize};

use crate::core::node::pixels_from_number;
use crate::core::{Length, Logical, Pixels, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Strategy {
    #[default]
    PartitionTree,
    Master,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct PartitionTreeConfig {
    pub(crate) tab_bar_height: Pixels<Logical>,
    pub(crate) automatic_tiling: bool,
}

impl Default for PartitionTreeConfig {
    fn default() -> Self {
        PartitionTreeConfig {
            tab_bar_height: Pixels::new(24),
            automatic_tiling: true,
        }
    }
}

/// Seed new workspaces only. A reload does not push these into existing
/// workspaces, and runtime tuning persists.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct MasterConfig {
    pub(crate) master_ratio: f32,
    pub(crate) master_count: usize,
}

impl Default for MasterConfig {
    fn default() -> Self {
        MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct SizeConstraints {
    pub(crate) minimum_width: SizeConstraint,
    pub(crate) minimum_height: SizeConstraint,
    pub(crate) maximum_width: SizeConstraint,
    pub(crate) maximum_height: SizeConstraint,
}

impl Default for SizeConstraints {
    fn default() -> Self {
        Self {
            minimum_width: SizeConstraint::default_min(),
            minimum_height: SizeConstraint::default_min(),
            maximum_width: SizeConstraint::default(),
            maximum_height: SizeConstraint::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SizeConstraint {
    Pixels(Pixels<Logical>),
    Percent(f32),
}

impl Default for SizeConstraint {
    fn default() -> Self {
        SizeConstraint::Pixels(Pixels::ZERO)
    }
}

impl SizeConstraint {
    /// `Pixels` is a config-denominated absolute logical length, so it goes
    /// through `to_unit(scale)` to reach the frame unit. `Percent` is a ratio of
    /// `screen_size`, which the caller passes in frame units already, so `scale`
    /// does not apply.
    pub(crate) fn resolve(&self, screen_size: Length<Unit>, scale: f32) -> Length<Unit> {
        match self {
            SizeConstraint::Pixels(px) => Length::from_pixels(*px).to_unit(scale),
            SizeConstraint::Percent(pct) => screen_size * (pct / 100.0),
        }
    }

    pub(crate) fn default_min() -> Self {
        SizeConstraint::Percent(5.0)
    }
}

impl<'de> Deserialize<'de> for SizeConstraint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SizeConstraintVisitor;

        impl<'de> serde::de::Visitor<'de> for SizeConstraintVisitor {
            type Value = SizeConstraint;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter
                    .write_str("a whole number for pixels or a string percentage (e.g., \"10%\")")
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                pixels_from_number(v).map(SizeConstraint::Pixels)
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                self.visit_f64(v as f64)
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                self.visit_f64(v as f64)
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if let Some(pct) = s.strip_suffix('%') {
                    let val: f32 = pct.trim().parse().map_err(E::custom)?;
                    if !(0.0..=100.0).contains(&val) {
                        return Err(E::custom("percentage must be between 0 and 100"));
                    }
                    Ok(SizeConstraint::Percent(val))
                } else {
                    Err(E::custom("string must be a percentage (e.g., \"10%\")"))
                }
            }
        }

        deserializer.deserialize_any(SizeConstraintVisitor)
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
