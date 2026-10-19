pub mod config;
mod core;
mod geometry;
pub use geometry::{
    Coordinate, Dimension, FractionalCoordinate, FractionalLength, FractionalOffset,
    FractionalPoint, FractionalSize, Length, Logical, Offset, Physical, PixelRect, Point, Size,
    Unit, UnitKind,
};
pub mod font;
pub mod theme;

pub use core::{
    ContainerId, ContainerPlacement, Direction, FloatWindowPlacement, Hub, LimitObservation,
    LimitUpdate, MasterConfig, MinimizedWindowEntry, MonitorId, MonitorLayout, MonitorPlacements,
    PaneDisplay, PartitionTreeConfig, PreferredLayouts, PreferredMaster, PreferredScrolling,
    PreferredTiling, PreferredWorkspace, ReportedMonitor, SizeConstraint, SizeConstraints,
    SplitMode, Strategy, TilingConfig, TilingWindowPlacement, TreeLayoutNode, VisiblePlacements,
    WindowId, WindowMatcher, WindowMetadata, WindowRestrictions, pattern_matches,
};
pub use dome_ipc::action;

#[cfg(test)]
mod tests;
