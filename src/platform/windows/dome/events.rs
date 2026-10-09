use crate::action::WorkspaceInfo;
use crate::config::Appearance;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, Length, Logical, MonitorId, Physical, PixelRect,
    Pixels, TilingWindowPlacement, WindowId,
};
use crate::platform::windows::external::{HwndId, ZOrder};

pub(in crate::platform::windows) enum HubMessage {
    Scene(RenderScene),
    MonitorsChanged(MonitorSetChange),
    AppearanceChanged(Appearance),
}

/// The domain's only route to the window side.
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
    /// Tray label source, carried here because the tray lives with the windows.
    pub(in crate::platform::windows) workspaces: Vec<WorkspaceInfo>,
}

pub(in crate::platform::windows) enum FloatOverlayAction {
    /// The window thread creates the overlay, so it seeds the first z-order. Every later
    /// z-order write comes from the domain through `ManageOverlay`.
    Create {
        window_id: WindowId,
        placement: FloatWindowPlacement,
        z_order: ZOrder,
        scale: f32,
        border_thickness: Pixels<Physical>,
        corner_radius: Length<Logical>,
    },
    Update {
        window_id: WindowId,
        placement: FloatWindowPlacement,
        scale: f32,
        border_thickness: Pixels<Physical>,
        corner_radius: Length<Logical>,
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
    pub(in crate::platform::windows) tiling_windows: Vec<TilingWindowShow>,
    pub(in crate::platform::windows) float_windows: Vec<FloatWindowPlacement>,
    pub(in crate::platform::windows) containers: Vec<ContainerPlacement>,
    /// Parked tiling windows whose on-screen part shows through a DWM thumbnail. The window
    /// thread drops the thumbnail of a window that no monitor lists.
    pub(in crate::platform::windows) thumbnails: Vec<ThumbnailShow>,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::platform::windows) struct TilingWindowShow {
    pub(in crate::platform::windows) placement: TilingWindowPlacement,
    pub(in crate::platform::windows) corner_radius: Length<Logical>,
}

/// What the DWM thumbnail of a parked tiling window needs to draw the window's on-screen part.
/// The tile's border is not drawn from it.
pub(in crate::platform::windows) struct ThumbnailShow {
    pub(in crate::platform::windows) window_id: WindowId,
    pub(in crate::platform::windows) source: HwndId,
    /// Where the thumbnail overlay sits, in screen coordinates.
    pub(in crate::platform::windows) frame: PixelRect,
    /// The region of `source` the thumbnail draws, relative to the top-left corner of the
    /// window's visible frame.
    pub(in crate::platform::windows) source_rect: PixelRect,
}
