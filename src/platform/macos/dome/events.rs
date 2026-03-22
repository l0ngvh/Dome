use std::fmt;

use objc2::rc::Retained;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;
use objc2_io_surface::IOSurface;

use crate::action::Actions;
use crate::config::Config;
use crate::core::{ContainerId, ContainerPlacement, Dimension, WindowId, WindowPlacement};
use crate::platform::macos::running_application::RunningApp;

use super::monitor::MonitorInfo;

pub(in crate::platform::macos) enum HubEvent {
    /// Visible windows changed for an app (window created/destroyed/minimized/shown/hidden).
    VisibleWindowsChanged {
        pid: i32,
    },
    /// Sync focus for an app. Separated from VisibleWindowsChanged because offscreen windows (on
    /// other workspaces) still report as "active", which would hijack focus and prevent switching
    /// to empty workspaces.
    SyncFocus {
        pid: i32,
    },
    AppTerminated {
        pid: i32,
    },
    TitleChanged(CGWindowID),
    /// One or more windows of app with pid got resized or moved.
    /// This can't be on a per CGWindowID basis, as these events are unreliable and are often fired
    /// on the wrong window. For example, Slack doesn't emit this event on the main application
    /// window. This can however create a scenario when one window in the app finishes
    /// moving/resizing and send this notification, but other windows are not finish yet.
    WindowMovedOrResized {
        pid: i32,
    },
    Action(Actions),
    ConfigChanged(Config),
    /// Periodic sync to catch missed AX notifications, as AX notifications are unreliable. Only
    /// syncs window state, not focus, as focus changes should come from user interactions. Beside
    /// we receive plenty of focus events, so missing them isn't a concern.
    Sync,
    ScreensChanged(Vec<MonitorInfo>),
    MirrorClicked(WindowId),
    TabClicked(ContainerId, usize),
    /// macOS Space changed. Used to detect native fullscreen enter/exit since
    /// native fullscreen moves windows to a separate Space.
    SpaceChanged,
    Shutdown,
}

impl fmt::Display for HubEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VisibleWindowsChanged { pid } => write!(f, "VisibleWindowsChanged(pid={pid})"),
            Self::SyncFocus { pid } => write!(f, "SyncFocus(pid={pid})"),
            Self::AppTerminated { pid } => write!(f, "AppTerminated(pid={pid})"),
            Self::TitleChanged(cg_id) => write!(f, "TitleChanged(cg_id={cg_id})"),
            Self::WindowMovedOrResized { pid } => {
                write!(f, "WindowMovedOrResized(pid={pid})")
            }
            Self::Action(actions) => write!(f, "Action({actions})"),
            Self::ConfigChanged(_) => write!(f, "ConfigChanged"),
            Self::Sync => write!(f, "Sync"),
            Self::ScreensChanged(monitors) => {
                write!(f, "ScreensChanged(count={})", monitors.len())
            }
            Self::MirrorClicked(window_id) => write!(f, "MirrorClicked({window_id})"),
            Self::TabClicked(container_id, tab_idx) => {
                write!(f, "TabClicked({container_id}, tab_idx={tab_idx})")
            }
            Self::SpaceChanged => write!(f, "SpaceChanged"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

pub(in crate::platform::macos) enum HubMessage {
    Frame(RenderFrame),
    RegisterObservers(Vec<RunningApp>),
    CaptureFrame {
        window_id: WindowId,
        surface: Retained<IOSurface>,
    },
    CaptureFailed {
        window_id: WindowId,
    },
    ConfigChanged(Config),
    Shutdown,
}

pub(in crate::platform::macos) struct RenderFrame {
    pub(in crate::platform::macos) creates: Vec<OverlayCreate>,
    pub(in crate::platform::macos) deletes: Vec<WindowId>,
    pub(in crate::platform::macos) shows: Vec<OverlayShow>,
    pub(in crate::platform::macos) container_creates: Vec<ContainerOverlayData>,
    pub(in crate::platform::macos) containers: Vec<ContainerOverlayData>,
    pub(in crate::platform::macos) deleted_containers: Vec<ContainerId>,
}

pub(in crate::platform::macos) struct OverlayCreate {
    pub(in crate::platform::macos) window_id: WindowId,
    pub(in crate::platform::macos) frame: NSRect,
}

pub(in crate::platform::macos) struct OverlayShow {
    pub(in crate::platform::macos) window_id: WindowId,
    pub(in crate::platform::macos) placement: WindowPlacement,
    pub(in crate::platform::macos) cocoa_frame: NSRect,
    pub(in crate::platform::macos) scale: f64,
    pub(in crate::platform::macos) visible_content: Option<Dimension>,
}

pub(in crate::platform::macos) struct ContainerOverlayData {
    pub(in crate::platform::macos) placement: ContainerPlacement,
    pub(in crate::platform::macos) tab_titles: Vec<String>,
    pub(in crate::platform::macos) cocoa_frame: NSRect,
}
