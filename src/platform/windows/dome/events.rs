use std::sync::Arc;

use crate::action::WorkspaceInfo;
use crate::config::Config;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, MonitorId, Physical, PixelRect, Pixels,
    TilingWindowPlacement, WindowId,
};
use crate::platform::windows::external::{ManageExternalWindow, ShowCmd, ZOrder};

pub(in crate::platform::windows) enum HubMessage {
    Scene(RenderScene),
    MonitorsChanged(MonitorSetChange),
    ConfigChanged(Box<Config>),
    Placements(Vec<PendingPlacement>),
}

/// The domain's only route to the window side. `Dome` names this instead of `WindowThread`
/// so the window work can move onto its own thread without the domain learning that it did.
pub(in crate::platform::windows) trait SceneSender {
    fn send(&mut self, msg: HubMessage);
}

/// The domain decides that an overlay exists, the window side owns the handle it gets.
pub(in crate::platform::windows) struct MonitorSetChange {
    pub(in crate::platform::windows) added: Vec<NewTilingOverlay>,
    pub(in crate::platform::windows) removed: Vec<MonitorId>,
}

pub(in crate::platform::windows) struct NewTilingOverlay {
    pub(in crate::platform::windows) monitor_id: MonitorId,
    pub(in crate::platform::windows) work_area: PixelRect,
    pub(in crate::platform::windows) scale: f32,
}

pub(in crate::platform::windows) struct RenderScene {
    pub(in crate::platform::windows) monitors: Vec<MonitorScene>,
    pub(in crate::platform::windows) float_overlays: Vec<FloatOverlayAction>,
    /// Set only when focus moved to a different monitor this cycle, so the window half
    /// applies it without keeping focus history of its own.
    pub(in crate::platform::windows) focus_monitor: Option<MonitorId>,
    /// Tray label source, carried here because the tray lives with the windows.
    pub(in crate::platform::windows) workspaces: Vec<WorkspaceInfo>,
    pub(in crate::platform::windows) placements: Vec<PendingPlacement>,
}

pub(in crate::platform::windows) struct PendingPlacement {
    pub(in crate::platform::windows) ext: Arc<dyn ManageExternalWindow>,
    pub(in crate::platform::windows) action: PlacementAction,
}

pub(in crate::platform::windows) enum PlacementAction {
    SetPosition {
        z_order: ZOrder,
        rect: PixelRect<Physical>,
    },
    AnchorAboveOverlay {
        monitor_id: MonitorId,
        rect: PixelRect<Physical>,
        /// Two-step exit from the topmost band. Placing self below a non-topmost reference
        /// does not, by itself, clear WS_EX_TOPMOST. Only HWND_NOTOPMOST and HWND_BOTTOM are
        /// documented to drop the flag. NotTopmost first to escape the band, then a second
        /// call to position above the overlay reference.
        escape_topmost: bool,
    },
    MoveOffscreen,
    ShowCmd(ShowCmd),
    SetForegroundWindow,
}

pub(in crate::platform::windows) enum FloatOverlayAction {
    Update {
        window_id: WindowId,
        placement: FloatWindowPlacement,
        z_order: ZOrder,
        scale: f32,
        border_thickness: Pixels<Physical>,
    },
    /// Absent overlay is normal here, because a window leaving the float state can be hidden
    /// before its overlay is retained away.
    Hide(WindowId),
}

pub(in crate::platform::windows) struct MonitorScene {
    pub(in crate::platform::windows) monitor_id: MonitorId,
    pub(in crate::platform::windows) work_area: PixelRect,
    /// Resolved on the domain side, since the monitor registry does not cross the seam.
    pub(in crate::platform::windows) scale: f32,
    pub(in crate::platform::windows) border_thickness: Pixels<Physical>,
    pub(in crate::platform::windows) tiling_windows: Vec<TilingWindowPlacement>,
    pub(in crate::platform::windows) float_windows: Vec<FloatWindowPlacement>,
    pub(in crate::platform::windows) containers: Vec<ContainerPlacement>,
}
