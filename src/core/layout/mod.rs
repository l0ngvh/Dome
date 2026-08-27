mod options;
mod preferred;

pub(crate) use options::{
    MasterConfig, PartitionTreeConfig, SizeConstraint, SizeConstraints, Strategy,
};
pub(crate) use preferred::{PaneConfig, PreferredWorkspace, SplitMode, TreeLayoutNode};

use super::matcher::WindowMatcher;
use super::node::{Logical, Pixels};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutOptions {
    pub(crate) strategy: Strategy,
    pub(crate) border_size: Pixels<Logical>,
    pub(crate) partition_tree: PartitionTreeConfig,
    pub(crate) master: MasterConfig,
    pub(crate) size_constraints: SizeConstraints,
    pub(crate) float: Vec<WindowMatcher>,
    pub(crate) fullscreen: Vec<WindowMatcher>,
    pub(crate) ignore: Vec<WindowMatcher>,
}

impl LayoutOptions {
    pub(crate) fn default_border_size() -> Pixels<Logical> {
        Pixels::new(4)
    }
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            strategy: Strategy::default(),
            border_size: Self::default_border_size(),
            partition_tree: PartitionTreeConfig::default(),
            master: MasterConfig::default(),
            size_constraints: SizeConstraints::default(),
            // Empty rather than `Config::default()`'s bundled matcher lists, so a fixture
            // manages every window it inserts.
            float: Vec::new(),
            fullscreen: Vec::new(),
            ignore: Vec::new(),
        }
    }
}
