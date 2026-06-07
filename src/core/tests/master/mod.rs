mod layout;
mod scroll;

use crate::config::Strategy;
use crate::core::hub::{Hub, HubConfig};
use crate::core::node::{Dimension, Length};

pub(super) fn setup_master() -> Hub {
    let mut config = HubConfig::default();
    config.layout.strategy = Strategy::Master;
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
