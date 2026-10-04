use crate::core::node::{Logical, Pixels};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartitionTreeConfig {
    pub(crate) tab_bar_height: Pixels<Logical>,
    pub(crate) automatic_tiling: bool,
}
