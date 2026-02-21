mod allocator;
mod float;
mod hub;
mod node;
mod split;
#[cfg(test)]
mod tests;
mod workspace;

pub(crate) use hub::Hub;
#[cfg(target_os = "macos")]
pub(crate) use hub::WindowPlacement;
pub(crate) use node::{Child, Container, ContainerId, Dimension, MonitorId, SpawnMode, WindowId};
#[cfg(target_os = "macos")]
pub(crate) use node::Window;

const MAX_ITERATIONS: usize = 10000;

fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
