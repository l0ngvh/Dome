use crate::action::WorkspaceInfo;
use crate::config::Appearance;
use crate::core::{
    ContainerPlacement, FloatWindowPlacement, Length, Logical, MonitorId, Physical, PixelRect,
    Pixels, TilingWindowPlacement, WindowId,
};
use crate::platform::windows::external::{HwndId, ZOrder};

pub(crate) enum HubMessage {
    Scene(RenderScene),
    MonitorsChanged(MonitorSetChange),
    AppearanceChanged(Appearance),
}

/// The domain's only route to the window side.
pub(crate) trait SceneSender {
    fn send(&mut self, msg: HubMessage);
}

/// The domain decides that an overlay exists, the window side owns the handle it gets.
pub(crate) struct MonitorSetChange {
    pub(crate) added: Vec<NewTilingOverlay>,
    pub(crate) removed: Vec<MonitorId>,
}

pub(crate) struct NewTilingOverlay {
    pub(crate) monitor_id: MonitorId,
    pub(in crate::platform::windows) work_area: PixelRect,
    pub(in crate::platform::windows) scale: f32,
}

pub(crate) struct RenderScene {
    pub(crate) monitors: Vec<MonitorScene>,
    pub(crate) float_overlays: Vec<FloatOverlayAction>,
    /// Tray label source, carried here because the tray lives with the windows.
    pub(in crate::platform::windows) workspaces: Vec<WorkspaceInfo>,
}

pub(crate) enum FloatOverlayAction {
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

pub(crate) struct MonitorScene {
    pub(crate) monitor_id: MonitorId,
    pub(crate) work_area: PixelRect,
    /// Resolved on the domain side, since the monitor registry does not cross the seam.
    pub(in crate::platform::windows) scale: f32,
    pub(crate) border_thickness: Pixels<Physical>,
    pub(crate) tiling_windows: Vec<TilingWindowShow>,
    pub(crate) float_windows: Vec<FloatWindowPlacement>,
    pub(crate) containers: Vec<ContainerPlacement>,
    /// Parked tiling windows whose on-screen part shows through a DWM thumbnail. The window
    /// thread drops the thumbnail of a window that no monitor lists.
    pub(crate) thumbnails: Vec<ThumbnailShow>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TilingWindowShow {
    pub(crate) placement: TilingWindowPlacement,
    pub(crate) corner_radius: Length<Logical>,
}

/// What the DWM thumbnail of a parked tiling window needs to draw the window's on-screen part.
/// The tile's border is not drawn from it.
pub(crate) struct ThumbnailShow {
    pub(crate) window_id: WindowId,
    pub(crate) source: HwndId,
    /// Where the thumbnail overlay sits, in screen coordinates.
    pub(crate) frame: PixelRect,
    /// The region of `source` the thumbnail draws, relative to the top-left corner of the
    /// window's visible frame.
    pub(crate) source_rect: PixelRect,
}
