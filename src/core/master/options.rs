use std::ops::RangeInclusive;

use crate::config::lua::deserializer::LoadContext;

const MASTER_RATIO_RANGE: RangeInclusive<f32> = 0.1..=0.9;
pub(crate) const MIN_MASTER_COUNT: usize = 1;

/// Seed new workspaces only. A reload does not push these into existing
/// workspaces, and runtime tuning persists.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MasterConfig {
    pub(crate) master_ratio: f32,
    pub(crate) master_count: usize,
}

pub(crate) fn clamp_master_ratio(v: f32) -> f32 {
    v.clamp(*MASTER_RATIO_RANGE.start(), *MASTER_RATIO_RANGE.end())
}

/// Yields no override rather than a default, so the caller decides where an
/// absent or out-of-range ratio falls back to.
pub(crate) fn read_master_ratio_override(table: &mlua::Table, cx: &mut LoadContext) -> Option<f32> {
    let ratio: Option<f32> = cx.field(table, "master_ratio");
    let ratio = ratio?;
    if MASTER_RATIO_RANGE.contains(&ratio) {
        return Some(ratio);
    }
    cx.push("master_ratio");
    cx.warn_value(&format!(
        "must be between {} and {}, got {ratio}",
        MASTER_RATIO_RANGE.start(),
        MASTER_RATIO_RANGE.end()
    ));
    cx.pop();
    None
}

/// Yields no override rather than a default, so the caller decides where an
/// absent or too-small count falls back to.
pub(crate) fn read_master_count_override(
    table: &mlua::Table,
    cx: &mut LoadContext,
) -> Option<usize> {
    let count: Option<usize> = cx.field(table, "master_count");
    let count = count?;
    if count >= MIN_MASTER_COUNT {
        return Some(count);
    }
    cx.push("master_count");
    cx.warn_value(&format!("must be at least {MIN_MASTER_COUNT}, got {count}"));
    cx.pop();
    None
}
