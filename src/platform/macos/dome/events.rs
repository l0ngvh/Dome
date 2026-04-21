use std::collections::HashSet;
use std::fmt;
use std::time::Instant;

use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;

use crate::action::Actions;
use crate::config::Config;
use crate::core::{Child, ContainerId, ContainerPlacement, Dimension, MonitorId, WindowPlacement};

use super::super::MonitorInfo;

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
        observed_at: Instant,
    },
    Action(Actions),
    ConfigChanged(Box<Config>),
    /// Periodic sync to catch missed AX notifications, as AX notifications are unreliable. Only
    /// syncs window state, not focus, as focus changes should come from user interactions. Beside
    /// we receive plenty of focus events, so missing them isn't a concern.
    Sync,
    ScreensChanged(Vec<MonitorInfo>),
    MirrorClicked(CGWindowID),
    TabClicked(ContainerId, usize),
    /// macOS Space changed. Used to detect native fullscreen enter/exit since
    /// native fullscreen moves windows to a separate Space.
    SpaceChanged,
    /// A single PID was observed (from app-launch path).
    PidObserved {
        pid: i32,
    },
    /// Full set of observed PIDs after a refresh cycle. Replaces
    /// `observed_pids` wholesale.
    ObservedPidsRefreshed(HashSet<i32>),
    Shutdown,
}

impl fmt::Display for HubEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VisibleWindowsChanged { pid } => write!(f, "VisibleWindowsChanged(pid={pid})"),
            Self::SyncFocus { pid } => write!(f, "SyncFocus(pid={pid})"),
            Self::AppTerminated { pid } => write!(f, "AppTerminated(pid={pid})"),
            Self::TitleChanged(cg_id) => write!(f, "TitleChanged(cg_id={cg_id})"),
            Self::WindowMovedOrResized { pid, .. } => {
                write!(f, "WindowMovedOrResized(pid={pid})")
            }
            Self::Action(actions) => write!(f, "Action({actions})"),
            Self::ConfigChanged(_) => write!(f, "ConfigChanged"),
            Self::Sync => write!(f, "Sync"),
            Self::ScreensChanged(monitors) => {
                write!(f, "ScreensChanged(count={})", monitors.len())
            }
            Self::MirrorClicked(cg_id) => write!(f, "MirrorClicked({cg_id})"),
            Self::TabClicked(container_id, tab_idx) => {
                write!(f, "TabClicked({container_id}, tab_idx={tab_idx})")
            }
            Self::SpaceChanged => write!(f, "SpaceChanged"),
            Self::PidObserved { pid } => write!(f, "PidObserved(pid={pid})"),
            Self::ObservedPidsRefreshed(pids) => {
                write!(f, "ObservedPidsRefreshed(count={})", pids.len())
            }
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

pub(in crate::platform::macos) enum HubMessage {
    Frame(RenderFrame),
    RefreshObservers,
    ConfigChanged(Config),
    Shutdown,
}

/// Rendering instructions produced by the hub thread after each layout cycle.
/// The main thread consumes this to create, update, and destroy overlay windows,
/// since macOS requires all UI operations to happen on the main thread.
pub(in crate::platform::macos) struct RenderFrame {
    /// One entry per visible monitor. Contains tiling windows and containers to render on
    /// that monitor's shared overlay. Monitors with no tiling content still get an entry
    /// so the main thread can hide their overlay.
    pub(in crate::platform::macos) tiling: Vec<MonitorTilingData>,
    /// Float windows visible on the current workspace. Created on first appearance,
    /// updated on subsequent frames. Float windows are rare, so the UI thread simply
    /// removes overlays and captures for any window not in this list rather than
    /// tracking individual deletions or float-to-tiling transitions.
    pub(in crate::platform::macos) float_shows: Vec<FloatShow>,
    pub(in crate::platform::macos) focused: Option<Child>,
    pub(in crate::platform::macos) focused_monitor_id: MonitorId,
}

pub(in crate::platform::macos) struct MonitorTilingData {
    pub(in crate::platform::macos) monitor_id: MonitorId,
    pub(in crate::platform::macos) monitor_dim: Dimension,
    pub(in crate::platform::macos) cocoa_frame: NSRect,
    pub(in crate::platform::macos) scale: f64,
    pub(in crate::platform::macos) windows: Vec<WindowPlacement>,
    pub(in crate::platform::macos) containers: Vec<(ContainerPlacement, Vec<String>)>,
}

pub(in crate::platform::macos) struct FloatShow {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) placement: WindowPlacement,
    pub(in crate::platform::macos) cocoa_frame: NSRect,
    pub(in crate::platform::macos) scale: f64,
    pub(in crate::platform::macos) content_dim: Dimension,
}
