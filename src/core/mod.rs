mod allocator;
mod dto;
mod export;
mod float;
mod fullscreen;
mod hub;
mod master;
mod matcher;
mod minimize;
mod node;
mod partition_tree;
mod strategy;
#[cfg(test)]
mod tests;
mod workspace;

pub(crate) use hub::Hub;
#[cfg(target_os = "macos")]
pub(crate) use hub::MonitorPlacements;
pub(crate) use hub::{
    ContainerPlacement, FloatWindowPlacement, GlobalLayoutConfig, MonitorLayout, SpawnIndicator,
    TilingWindowPlacement,
};
pub(crate) use node::Direction;
#[cfg(target_os = "windows")]
pub(crate) use node::Physical;
pub(crate) use node::PixelRect;
pub(crate) use node::{
    ContainerId, Dimension, Length, LimitObservation, LimitUpdate, Logical, MonitorId, Unit,
    WindowId, WindowMetadata, WindowRestrictions,
};
pub(crate) use node::{Pixels, pixels_from_config};
pub(crate) use strategy::TilingAction;

pub(crate) use dto::WorkspaceInfo;

const MAX_ITERATIONS: usize = 10000;

pub(super) fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
