mod layout;
mod scroll;

use crate::config::{SizeConstraint, Strategy};
use crate::core::hub::Hub;
use crate::core::node::{Dimension, Length};
use crate::core::tests::default_layout_for_tests;

pub(super) fn setup_master() -> Hub {
    let mut config = default_layout_for_tests();
    config.strategy = Strategy::Master;
    // Master validate_tree (master/mod.rs:751) asserts dim.height > 0, which
    // requires effective_min_h > 0 for every tiling window. Production always
    // seeds positive global mins; match that here. 1.0 is small enough not to
    // perturb existing master snapshot tests (screen height is 30, tests pack
    // at most ~8 windows per pane, so sum_mins < container_h and
    // distribute_space uses its binary-search branch as before).
    config.min_width = SizeConstraint::Pixels(Length::new(1.0));
    config.min_height = SizeConstraint::Pixels(Length::new(1.0));
    Hub::new(
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
        config,
        Vec::new(),
    )
}
