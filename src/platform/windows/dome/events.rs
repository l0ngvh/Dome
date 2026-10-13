use std::collections::HashMap;

use super::MonitorInfo;
use crate::action::WorkspaceInfo;
use crate::config::Appearance;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, Length, Logical, MonitorId, Physical, PixelRect,
    Pixels, TilingWindowPlacement, WindowId,
};
use crate::platform::windows::external::HwndId;

pub(in crate::platform::windows) enum HubMessage {
    Scene(Scene),
    Command(WindowCommand),
    AppearanceChanged(Appearance),
    DisplayRefreshed(Vec<MonitorInfo>),
    Shutdown,
}

pub(in crate::platform::windows) trait SceneSender {
    fn send(&mut self, message: HubMessage);
}

#[derive(Clone)]
pub(in crate::platform::windows) struct Scene {
    pub windows: HashMap<HwndId, ExpectedWindow>,
    pub monitors: Vec<MonitorScene>,
    pub focused_window: Option<HwndId>,
    pub focus_request: Option<FocusTarget>,
    pub workspaces: Vec<WorkspaceInfo>,
}

#[derive(Clone, Copy)]
pub(in crate::platform::windows) struct ExpectedWindow {
    pub id: WindowId,
    pub placement: WindowPlacement,
}

#[derive(Clone, Copy)]
pub(in crate::platform::windows) enum WindowPlacement {
    Tiling {
        monitor: MonitorId,
        placement: TilingWindowPlacement,
    },
    Float {
        monitor: MonitorId,
        placement: FloatWindowPlacement,
    },
    Fullscreen {
        monitor: MonitorId,
    },
    Hidden,
    Minimized,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::platform::windows) enum FocusTarget {
    Window(HwndId),
    Monitor(MonitorId),
}

pub(in crate::platform::windows) enum WindowCommand {
    Close(HwndId),
    Unminimize(HwndId),
}

#[derive(Clone)]
pub(in crate::platform::windows) struct MonitorScene {
    pub id: MonitorId,
    pub native: isize,
    pub work_area: PixelRect<Physical>,
    pub system_work_area: PixelRect<Physical>,
    pub scale: f32,
    pub border_thickness: Pixels<Physical>,
    pub tiling_windows: Vec<TilingWindowPlacement>,
    pub float_windows: Vec<FloatWindowPlacement>,
    pub containers: Vec<ContainerPlacement>,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::windows) struct TilingWindowShow {
    pub placement: TilingWindowPlacement,
    pub corner_radius: Length<Logical>,
}
