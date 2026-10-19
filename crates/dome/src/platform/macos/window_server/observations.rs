use dome_core::FractionalCoordinate;
use dome_core::{FractionalLength, Logical, PixelRect, WindowId};
use objc2_core_graphics::CGWindowID;
use std::collections::{HashMap, HashSet};

pub(in crate::platform::macos) struct WindowObservation {
    pub(in crate::platform::macos) id: WindowId,
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) metadata: WindowMetadata,
    pub(in crate::platform::macos) valid: bool,
    pub(in crate::platform::macos) minimized: bool,
    pub(in crate::platform::macos) native_fullscreen: Option<bool>,
    pub(in crate::platform::macos) role: Option<String>,
    pub(in crate::platform::macos) subrole: Option<String>,
    pub(in crate::platform::macos) is_root: bool,
    pub(in crate::platform::macos) position_settable: bool,
    pub(in crate::platform::macos) size_settable: bool,
    pub(in crate::platform::macos) main_settable: bool,
    pub(in crate::platform::macos) rect: Option<PixelRect>,
    pub(in crate::platform::macos) placement: Option<(PixelRect<Logical>, std::time::Instant)>,
    pub(in crate::platform::macos) corner_radius: Option<FractionalLength<Logical>>,
}

#[derive(Clone)]
pub(in crate::platform::macos) struct NativeWindowObservation {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) metadata: WindowMetadata,
    pub(in crate::platform::macos) valid: bool,
    pub(in crate::platform::macos) minimized: bool,
    pub(in crate::platform::macos) native_fullscreen: Option<bool>,
    pub(in crate::platform::macos) role: Option<String>,
    pub(in crate::platform::macos) subrole: Option<String>,
    pub(in crate::platform::macos) is_root: bool,
    pub(in crate::platform::macos) position_settable: bool,
    pub(in crate::platform::macos) size_settable: bool,
    pub(in crate::platform::macos) main_settable: bool,
    pub(in crate::platform::macos) rect: Option<PixelRect>,
    pub(in crate::platform::macos) placement: Option<(PixelRect<Logical>, std::time::Instant)>,
    pub(in crate::platform::macos) corner_radius: Option<FractionalLength<Logical>>,
}

pub(in crate::platform::macos) struct AppObservation {
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) listed: Vec<WindowObservation>,
    /// Requested identities, including windows absent from the application's list.
    /// A missing entry had no retained Accessibility handle when the read began.
    pub(in crate::platform::macos) requested: HashMap<WindowId, WindowObservation>,
    pub(in crate::platform::macos) requested_ids: HashSet<WindowId>,
}

impl AppObservation {
    pub(in crate::platform::macos) fn new(
        pid: i32,
        listed: Vec<WindowObservation>,
        requested: HashMap<WindowId, WindowObservation>,
        requested_ids: HashSet<WindowId>,
    ) -> Self {
        Self {
            pid,
            listed,
            requested,
            requested_ids,
        }
    }
}

pub(in crate::platform::macos) struct SystemObservation {
    pub(in crate::platform::macos) terminated_pids: Vec<i32>,
    pub(in crate::platform::macos) hidden_pids: Vec<i32>,
    pub(in crate::platform::macos) apps: Vec<AppObservation>,
}

pub(in crate::platform::macos) struct NativeFullscreenObservation {
    pub(in crate::platform::macos) id: WindowId,
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) metadata: WindowMetadata,
    pub(in crate::platform::macos) is_native_fullscreen: Option<bool>,
    pub(in crate::platform::macos) position:
        Option<(FractionalCoordinate<Logical>, FractionalCoordinate<Logical>)>,
    pub(in crate::platform::macos) size:
        Option<(FractionalLength<Logical>, FractionalLength<Logical>)>,
}

impl NativeFullscreenObservation {
    pub(in crate::platform::macos) fn new(
        id: WindowId,
        pid: i32,
        metadata: WindowMetadata,
        is_native_fullscreen: Option<bool>,
        position: Option<(FractionalCoordinate<Logical>, FractionalCoordinate<Logical>)>,
        size: Option<(FractionalLength<Logical>, FractionalLength<Logical>)>,
    ) -> Self {
        Self {
            id,
            pid,
            metadata,
            is_native_fullscreen,
            position,
            size,
        }
    }
}

pub(in crate::platform::macos) struct NativeAppObservation {
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) listed: Vec<NativeWindowObservation>,
    pub(in crate::platform::macos) requested: HashMap<CGWindowID, NativeWindowObservation>,
    pub(in crate::platform::macos) requested_ids: Vec<CGWindowID>,
    pub(in crate::platform::macos) invalid: Vec<CGWindowID>,
}

#[derive(Debug, Clone)]
pub(in crate::platform::macos) struct WindowMetadata {
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

pub(in crate::platform::macos) struct NativeFocusedWindow {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) pid: i32,
    pub(in crate::platform::macos) metadata: WindowMetadata,
    pub(in crate::platform::macos) is_native_fullscreen: Option<bool>,
    pub(in crate::platform::macos) position:
        Option<(FractionalCoordinate<Logical>, FractionalCoordinate<Logical>)>,
    pub(in crate::platform::macos) size:
        Option<(FractionalLength<Logical>, FractionalLength<Logical>)>,
}
