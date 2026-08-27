mod env;
mod fixtures;
mod mock;

mod lifecycle;
mod placement;
mod transitions;
mod uncooperative;
mod zorder;

use std::time::Instant;

use crate::core::{
    ContainerPlacement, Dimension, Length, LimitObservation, LimitUpdate, PixelRect, Pixels,
};
use crate::platform::windows::dome::{Dome, MonitorInfo};
use crate::platform::windows::external::HwndId;

use env::*;
use fixtures::*;
