mod allocator;
mod export;
mod float;
mod fullscreen;
mod hub;
mod master;
mod matcher;
mod minimize;
mod monitor;
mod node;
mod partition_tree;
mod preferred_layout;
mod strategy;
#[cfg(test)]
mod tests;
mod tiling;
mod workspace;

pub(crate) use hub::Hub;
#[cfg(target_os = "macos")]
pub(crate) use hub::MonitorPlacements;
pub(crate) use hub::{
    ContainerPlacement, FloatWindowPlacement, MonitorLayout, TilingWindowPlacement,
};
pub(crate) use master::{MasterConfig, read_master_count_override, read_master_ratio_override};
pub(crate) use matcher::{WindowMatcher, pattern_matches};
pub(crate) use monitor::MonitorSelector;
pub(crate) use monitor::ReportedMonitor;
pub(crate) use node::Direction;
#[cfg(target_os = "windows")]
pub(crate) use node::Physical;
pub(crate) use node::PixelRect;
pub(crate) use node::Pixels;
pub(crate) use node::{
    ContainerId, Dimension, Length, LimitObservation, LimitUpdate, Logical, MonitorId, Unit,
    WindowId, WindowMetadata, WindowRestrictions,
};
pub(crate) use partition_tree::{PartitionTreeConfig, SplitMode, TreeLayoutNode};
pub(crate) use preferred_layout::{PreferredLayouts, PreferredWorkspace};
pub(crate) use strategy::{StrategyAction, TilingAction};
pub(crate) use tiling::{SizeConstraint, SizeConstraints, Strategy, TilingConfig};

pub(crate) use master::PaneDisplay;

const MAX_ITERATIONS: usize = 10000;

pub(super) fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
