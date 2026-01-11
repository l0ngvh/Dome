mod allocator;
mod hub;
mod node;
#[cfg(test)]
mod tests;

pub(crate) use hub::Hub;
pub(crate) use node::{
    Child, Container, Dimension, FloatWindowId, Focus, SpawnMode, WindowId, WorkspaceId,
};

const MAX_ITERATIONS: usize = 10000;

fn bounded_loop() -> impl Iterator<Item = usize> {
    (0..MAX_ITERATIONS).chain(std::iter::once_with(|| {
        panic!("exceeded {MAX_ITERATIONS} iterations")
    }))
}
