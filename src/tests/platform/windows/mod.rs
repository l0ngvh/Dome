mod fixtures;
mod test_fixture;

mod lifecycle;
mod placement;
mod transitions;
mod uncooperative;
mod zorder;

use std::time::Instant;

use crate::core::{
    ContainerPlacement, Dimension, Length, LimitObservation, LimitUpdate, PixelRect, Pixels,
};
use crate::tests::platform::windows::test_fixture::{
    FocusTarget, HwndId, MonitorInfo, PaintedThumbnail,
};

use environment::*;
mod environment;
use fixtures::*;
