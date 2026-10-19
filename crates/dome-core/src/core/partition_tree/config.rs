use crate::core::node::{Length, Logical};

#[derive(Debug, Clone, PartialEq)]
pub struct PartitionTreeConfig {
    pub tab_bar_height: Length<Logical>,
    pub automatic_tiling: bool,
}
