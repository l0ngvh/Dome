pub(crate) use crate::geometry::{Dimension, FractionalLength, Length, Logical, PixelRect, Unit};
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
mod scrolling;
mod slot;
mod strategy;
mod tiling;
mod workspace;

pub(crate) use crate::geometry::ReservedArea;
pub use hub::Hub;
pub use hub::{
    ContainerPlacement, FloatWindowPlacement, MonitorLayout, MonitorPlacements,
    TilingWindowPlacement, VisiblePlacements,
};
pub use master::{MasterConfig, read_master_count_override, read_master_ratio_override};
pub use matcher::{WindowMatcher, pattern_matches};
pub(crate) use monitor::MonitorSelector;
pub use monitor::ReportedMonitor;
pub use node::Direction;
pub use node::MinimizedWindowEntry;
pub use node::{
    ContainerId, LimitObservation, LimitUpdate, MonitorId, WindowId, WindowMetadata,
    WindowRestrictions,
};
pub use partition_tree::{PartitionTreeConfig, SplitMode, TreeLayoutNode};
pub use preferred_layout::{
    PreferredLayouts, PreferredMaster, PreferredScrolling, PreferredTiling, PreferredWorkspace,
};
pub(crate) use scrolling::{ColumnConfig, ScrollingConfig};
pub(crate) use strategy::{StrategyAction, TilingAction};
pub use tiling::{SizeConstraint, SizeConstraints, Strategy, TilingConfig};

pub use master::PaneDisplay;

const MAX_ITERATIONS: usize = 10000;

pub(super) fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
