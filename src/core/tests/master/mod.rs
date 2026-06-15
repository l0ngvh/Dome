mod layout;
mod scroll;

use crate::config::Strategy;
use crate::core::hub::Hub;
use crate::core::node::{Dimension, Length};
use crate::core::tests::default_layout_for_tests;

pub(super) fn setup_master() -> Hub {
    let mut config = default_layout_for_tests();
    config.strategy = Strategy::Master;
    Hub::new(
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
        config,
    )
}
