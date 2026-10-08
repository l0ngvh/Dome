mod actions;
mod column;
mod config;
mod focus;
mod fullscreen;
mod placement;
mod preferred_layout;
mod resize;
mod scroll;
#[cfg(test)]
mod validate;

use rustc_hash::FxHashMap;

pub(crate) use config::ScrollingConfig;
pub(crate) use preferred_layout::ColumnConfig;
use preferred_layout::PreferredColumnSlot;

use crate::core::fullscreen::FullscreenWindows;
use crate::core::hub::{HubAccess, MonitorLayout};
use crate::core::node::{
    Child, ContainerId, Dimension, Direction, DisplayMode, Length, LimitObservation, PixelRect,
    WindowId, WorkspaceId,
};
use crate::core::slot::SlotId;
use crate::core::strategy::{FocusedChild, StrategyAction, TilingStrategy};
use crate::core::{
    PreferredTiling, PreferredWorkspace, SizeConstraint, SizeConstraints, TilingConfig,
};

/// A row of columns that scrolls sideways. Each column stacks its windows top to bottom, and
/// they share the column height. A row narrower than the work area is centered. A wider row runs
/// past the edge, and the workspace's horizontal offset and each column's vertical offset reveal
/// the part outside the work area.
#[derive(Debug)]
pub(crate) struct ScrollingStrategy {
    workspaces: FxHashMap<WorkspaceId, WorkspaceState>,
    window_states: FxHashMap<WindowId, WindowState>,
    column_width: SizeConstraint,
    size_constraints: SizeConstraints,
}

impl TilingStrategy for ScrollingStrategy {
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        entry: &PreferredWorkspace,
    ) {
        let PreferredTiling::Scrolling(layout) = &entry.tiling else {
            unreachable!("scrolling got a {:?} entry", entry.tiling.strategy());
        };
        // The strategy has no float support, so the entry's float matchers take no slot.
        FullscreenWindows::allocate_slots(hub, ws_id, &entry.fullscreen);
        let column_slots = Self::allocate_column_slots(hub, ws_id, &layout.columns);
        let host = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                column_slots,
                column_width: layout.column_width,
                columns: Vec::new(),
                focus_history: Vec::new(),
                x_offset: Length::ZERO,
                work_area: host.work_area,
                scale: host.scale,
                fullscreen_windows: FullscreenWindows::default(),
            },
        );
    }

    fn clear_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> Vec<(WindowId, DisplayMode)> {
        let Some(mut state) = self.workspaces.remove(&ws_id) else {
            return Vec::new();
        };
        let mut tiling = Vec::new();
        for column in &state.columns {
            tiling.extend(Self::column_windows(hub, column.container));
            hub.free_container(column.container);
        }
        for window_id in &tiling {
            self.window_states.remove(window_id);
        }
        let fullscreen = state.fullscreen_windows.clear().into_iter();
        tiling
            .into_iter()
            .map(|id| (id, DisplayMode::Tiling))
            .chain(fullscreen.map(|id| (id, DisplayMode::Fullscreen)))
            .collect()
    }

    fn export_workspace(&self, hub: &HubAccess, ws_id: WorkspaceId) -> PreferredWorkspace {
        let state = self.workspaces.get(&ws_id).unwrap();
        PreferredWorkspace {
            tiling: PreferredTiling::Scrolling(self.export_columns(hub, ws_id)),
            float: Vec::new(),
            fullscreen: state.fullscreen_windows.export(hub),
        }
    }

    fn attach_window(
        &mut self,
        hub: &mut HubAccess,
        id: WindowId,
        ws_id: WorkspaceId,
        mode: DisplayMode,
        slot: Option<SlotId>,
    ) {
        match mode {
            DisplayMode::Tiling => self.attach_tiling_window(hub, ws_id, id, slot),
            // The strategy has no float support, so a float tiles by the spawn rules.
            DisplayMode::Float { .. } => self.attach_tiling_window(hub, ws_id, id, None),
            DisplayMode::Fullscreen => {
                hub.windows.get_mut(id).set_workspace(Some(ws_id));
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.fullscreen_windows.attach(id);
            }
        }
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> DisplayMode {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("detaching window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if state.fullscreen_windows.detach(id) {
            return DisplayMode::Fullscreen;
        }
        self.detach_tiling_window(hub, ws_id, id);
        DisplayMode::Tiling
    }

    fn set_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        self.enter_fullscreen(hub, window_id);
    }

    fn unset_fullscreen(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        self.exit_fullscreen(hub, window_id);
    }

    fn handle_action(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, action: StrategyAction) {
        let Some(focused) = self.focused(ws_id) else {
            return;
        };
        let tiling_has_focus = matches!(focused, FocusedChild::Tiling(_));
        match action {
            StrategyAction::FocusDirection { direction, forward } if tiling_has_focus => {
                self.focus_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::MoveDirection { direction, forward } if tiling_has_focus => {
                self.move_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::ToggleSpawnMode if tiling_has_focus => self.toggle_spawn_mode(ws_id),
            StrategyAction::Grow if tiling_has_focus => {
                self.resize_focused_column(hub, ws_id, true)
            }
            StrategyAction::Shrink if tiling_has_focus => {
                self.resize_focused_column(hub, ws_id, false)
            }
            StrategyAction::FocusDirection { .. }
            | StrategyAction::MoveDirection { .. }
            | StrategyAction::ToggleSpawnMode
            | StrategyAction::Grow
            | StrategyAction::Shrink => {
                tracing::debug!("Tiling action while a fullscreen window has focus");
            }
            StrategyAction::ToggleFullscreen => self.toggle_fullscreen(hub, focused),
            StrategyAction::ToggleFloat => {
                tracing::debug!("Scrolling has no float support");
            }
            StrategyAction::ToggleDirection
            | StrategyAction::ToggleContainerLayout
            | StrategyAction::FocusParent
            | StrategyAction::FocusTab { .. }
            | StrategyAction::TabClicked { .. }
            | StrategyAction::MoreMaster
            | StrategyAction::FewerMaster => {
                tracing::debug!("Action has no effect on a scrolling workspace");
            }
        }
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if !state.fullscreen_windows.focus(window_id) {
            self.focus_tiling(hub, ws_id, window_id);
        }
    }

    fn reset_focus(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.compute_placement(hub, ws_id);
    }

    fn focused_child(&self, ws_id: WorkspaceId) -> Option<Child> {
        self.focused(ws_id).map(FocusedChild::child)
    }

    fn update_work_area(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        work_area: PixelRect,
        scale: f32,
    ) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.work_area = work_area;
        state.scale = scale;
        self.compute_placement(hub, ws_id);
    }

    fn update_window_size_limits(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        observed: LimitObservation,
    ) {
        let window = hub.windows.get_mut(window_id);
        window.limits.apply_observation(observed);
        let ws_id = window
            .workspace()
            .expect("non-minimized window has a workspace");
        self.compute_placement(hub, ws_id);
    }

    fn collect_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> MonitorLayout {
        let focused = self.focused(ws_id);
        if let Some(FocusedChild::Fullscreen(id)) = focused {
            return MonitorLayout::Fullscreen(id);
        }
        let (tiling_windows, containers) = self.collect_tiling_placements(
            hub,
            ws_id,
            highlighted && matches!(focused, Some(FocusedChild::Tiling(_))),
        );
        MonitorLayout::Normal {
            tiling_windows,
            float_windows: Vec::new(),
            containers,
        }
    }

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: &TilingConfig) {
        self.column_width = tiling.scrolling.column_width;
        self.size_constraints = tiling.size_constraints;
        let column_width = self.column_width;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            // A workspace that `layout.lua` names keeps the widths its entry and its resizes set.
            if !has_preferred_layout(hub, ws_id) {
                for column in &mut self.workspaces.get_mut(&ws_id).unwrap().columns {
                    column.width = column_width;
                }
            }
            self.compute_placement(hub, ws_id);
        }
    }
}

impl ScrollingStrategy {
    pub(crate) fn new(tiling: &TilingConfig) -> Self {
        Self {
            workspaces: FxHashMap::default(),
            window_states: FxHashMap::default(),
            column_width: tiling.scrolling.column_width,
            size_constraints: tiling.size_constraints,
        }
    }
}

/// Whether `layout.lua` names the workspace, under its origin monitor.
fn has_preferred_layout(hub: &HubAccess, ws_id: WorkspaceId) -> bool {
    let name = &hub.workspaces.get(ws_id).name;
    hub.preferred_layouts
        .workspace(&hub.origin_monitor_name(ws_id), name)
        .is_some()
}

/// The columns and fullscreen windows of one scrolling workspace, with its layout state.
#[derive(Debug)]
struct WorkspaceState {
    /// The `columns` list of this workspace's `layout.lua` entry, left to right.
    column_slots: Vec<PreferredColumnSlot>,
    /// The `column_width` of this workspace's `layout.lua` entry. `None` takes
    /// `scrolling.column_width`.
    column_width: Option<SizeConstraint>,
    /// Left to right.
    columns: Vec<Column>,
    /// Tiling windows from most to least recently focused. Always set-equal to the windows of
    /// `columns`.
    focus_history: Vec<WindowId>,
    /// Left edge of the viewport, in unscrolled workspace space.
    x_offset: Length,
    /// The layout bounds, a copy of the host monitor's work area.
    work_area: PixelRect,
    /// A copy of the host monitor's scale.
    scale: f32,
    fullscreen_windows: FullscreenWindows,
}

#[derive(Debug)]
struct Column {
    /// A hub container holding the column's windows top to bottom. Never empty.
    container: ContainerId,
    width: SizeConstraint,
    /// Top edge of the viewport over this column.
    y_offset: Length,
    /// The index into `WorkspaceState::column_slots` of the column slot that opened this column.
    /// `None` for a column that the spawn rules or a move opened.
    occupy: Option<usize>,
}

#[derive(Debug)]
struct WindowState {
    /// Border-box dimension, in layout space.
    dimension: Dimension,
    /// Where the next window opens while this window has focus.
    spawn_direction: Direction,
}
