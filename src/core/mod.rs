mod allocator;
mod float;
mod fullscreen;
mod hub;
mod master_stack;
mod node;
mod partition_tree;
mod strategy;
#[cfg(test)]
mod tests;
mod workspace;

pub(crate) use hub::Hub;
pub(crate) use hub::{
    ContainerPlacement, FloatWindowPlacement, MonitorLayout, MonitorPlacements, SpawnIndicator,
    TilingWindowPlacement,
};
pub(crate) use node::Direction;
pub(crate) use node::{ContainerId, Dimension, MonitorId, WindowId, WindowRestrictions};
pub(crate) use strategy::TilingAction;

const MAX_ITERATIONS: usize = 10000;

pub(super) fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
