mod env;
mod fixtures;
mod mock;

mod focus;
mod lifecycle;
mod placement;
mod scenes;
mod shutdown;
mod transitions;
mod uncooperative;
mod wake;
mod zorder;

use std::time::Instant;

use crate::core::{
    ContainerPlacement, Dimension, Length, LimitObservation, LimitUpdate, PixelRect, Pixels,
};
use crate::platform::windows::dome::MonitorInfo;
use crate::platform::windows::external::HwndId;

use env::*;
use fixtures::*;
