mod actions;
mod config;
mod float;
mod focus;
mod fullscreen;
mod pane;
mod placement;
mod preferred_layout;
mod scroll;
#[cfg(test)]
mod validate;

use rustc_hash::FxHashMap;

pub(crate) use config::{MasterConfig, read_master_count_override, read_master_ratio_override};
pub(crate) use pane::PaneDisplay;
use pane::{Pane, PaneKind};
use placement::WindowState;
pub(crate) use preferred_layout::PaneConfig;

use crate::core::TilingConfig;
use crate::core::float::FloatWindows;
use crate::core::fullscreen::FullscreenWindows;
use crate::core::hub::{HubAccess, MonitorLayout};
use crate::core::node::{
    Child, DisplayMode, LimitObservation, Logical, PixelRect, Pixels, WindowId, WorkspaceId,
};
use crate::core::slot::SlotId;
use crate::core::strategy::{FocusedChild, StrategyAction, TilingStrategy};
use crate::core::{PreferredTiling, PreferredWorkspace, SizeConstraints};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// Each pane scrolls vertically and independently when per-window min heights push the
/// pane's total content past the screen height. Horizontal scroll does not exist in master,
/// so per-window min width is not honored. The split follows master_ratio and each pane
/// fills its share.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: FxHashMap<WorkspaceId, WorkspaceState>,
    window_states: FxHashMap<WindowId, WindowState>,
    master_count: usize,
    master_ratio: f32,
    size_constraints: SizeConstraints,
    tab_bar_height: Pixels<Logical>,
}

impl TilingStrategy for MasterStrategy {
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        entry: &PreferredWorkspace,
    ) {
        let PreferredTiling::Master(layout) = &entry.tiling else {
            unreachable!("master got a {:?} entry", entry.tiling.strategy());
        };
        FullscreenWindows::allocate_slots(hub, ws_id, &entry.fullscreen);
        FloatWindows::allocate_slots(hub, ws_id, &entry.float);
        let master_container = hub.allocate_container(Vec::new(), ws_id);
        let secondary_container = hub.allocate_container(Vec::new(), ws_id);
        let master_slots = Self::allocate_slots(hub, ws_id, &layout.master.children);
        let secondary_slots = Self::allocate_slots(hub, ws_id, &layout.secondary.children);
        let host = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                master: Pane::new(master_container, master_slots, layout.master.display),
                secondary: Pane::new(
                    secondary_container,
                    secondary_slots,
                    layout.secondary.display,
                ),
                focus_history: Vec::new(),
                master_count: layout.master_count,
                master_ratio: layout.master_ratio,
                work_area: host.work_area,
                scale: host.scale,
                float_windows: FloatWindows::default(),
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
        for cid in [state.master.container, state.secondary.container] {
            tiling.extend(Self::pane_windows(hub, cid));
            hub.free_container(cid);
        }
        for &wid in &tiling {
            self.window_states.remove(&wid);
        }
        let floats = state.float_windows.clear().into_iter();
        let fullscreen = state.fullscreen_windows.clear().into_iter();
        tiling
            .into_iter()
            .map(|id| (id, DisplayMode::Tiling))
            .chain(floats.map(|(id, border_box)| (id, DisplayMode::Float { border_box })))
            .chain(fullscreen.map(|id| (id, DisplayMode::Fullscreen)))
            .collect()
    }

    fn export_workspace(&self, hub: &HubAccess, ws_id: WorkspaceId) -> PreferredWorkspace {
        let state = self.workspaces.get(&ws_id).unwrap();
        PreferredWorkspace {
            tiling: PreferredTiling::Master(self.export_panes(hub, ws_id)),
            float: state.float_windows.export(hub),
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
            DisplayMode::Tiling => self.attach_tiling_window(hub, id, ws_id, slot),
            DisplayMode::Float { border_box } => {
                hub.windows.get_mut(id).set_workspace(Some(ws_id));
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.float_windows.attach(id, border_box);
            }
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
        if let Some(border_box) = state.float_windows.detach(id) {
            return DisplayMode::Float { border_box };
        }
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

    fn update_float_rect(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        border_box: PixelRect,
    ) -> bool {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.float_windows.update_float_rect(window_id, border_box)
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if state.fullscreen_windows.focus(window_id) {
            state.float_windows.is_float_focused = false;
        } else if !state.float_windows.focus(window_id) {
            self.focus_tiling(hub, ws_id, window_id);
        }
    }

    fn reset_focus(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.float_windows.is_float_focused = state.float_windows.topmost().is_some();
        self.compute_placement(hub, ws_id);
    }

    fn focused_child(&self, ws_id: WorkspaceId) -> Option<Child> {
        self.focused(ws_id).map(FocusedChild::child)
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
        let float_windows = self.workspaces[&ws_id].float_windows.collect_placements(
            hub,
            ws_id,
            highlighted && matches!(focused, Some(FocusedChild::Float(_))),
        );
        MonitorLayout::Normal {
            tiling_windows,
            float_windows,
            containers,
        }
    }

    fn handle_action(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, action: StrategyAction) {
        let Some(focused) = self.focused(ws_id) else {
            return;
        };
        let tiling_has_focus = matches!(focused, FocusedChild::Tiling(_));
        let layout_is_visible = !matches!(focused, FocusedChild::Fullscreen(_));
        match action {
            StrategyAction::FocusDirection { direction, forward } if tiling_has_focus => {
                self.focus_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::MoveDirection { direction, forward } if tiling_has_focus => {
                self.move_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::ToggleContainerLayout if tiling_has_focus => {
                self.toggle_container_layout(hub, ws_id)
            }
            StrategyAction::FocusTab { forward } if tiling_has_focus => {
                self.focus_tab(hub, ws_id, forward)
            }
            StrategyAction::FocusDirection { .. }
            | StrategyAction::MoveDirection { .. }
            | StrategyAction::ToggleContainerLayout
            | StrategyAction::FocusTab { .. } => {
                tracing::debug!("Tiling action while a float or fullscreen window has focus");
            }
            StrategyAction::TabClicked {
                container_id,
                index,
            } if layout_is_visible => self.tab_clicked(hub, ws_id, container_id, index),
            StrategyAction::Grow if layout_is_visible => self.grow(hub, ws_id),
            StrategyAction::Shrink if layout_is_visible => self.shrink(hub, ws_id),
            StrategyAction::MoreMaster if layout_is_visible => self.more(hub, ws_id),
            StrategyAction::FewerMaster if layout_is_visible => self.fewer(hub, ws_id),
            StrategyAction::TabClicked { .. }
            | StrategyAction::Grow
            | StrategyAction::Shrink
            | StrategyAction::MoreMaster
            | StrategyAction::FewerMaster => {
                tracing::debug!("Layout action while fullscreen hides the layout");
            }
            StrategyAction::ToggleFloat => self.toggle_float(hub, ws_id, focused),
            StrategyAction::ToggleFullscreen => self.toggle_fullscreen(hub, focused),
            StrategyAction::ToggleSpawnMode
            | StrategyAction::ToggleDirection
            | StrategyAction::FocusParent => {
                tracing::debug!("Partition-tree action on a master workspace");
            }
        }
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

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: &TilingConfig) {
        let old_master_count = self.master_count;
        self.master_ratio = tiling.master.master_ratio;
        self.master_count = tiling.master.master_count;
        self.size_constraints = tiling.size_constraints;
        self.tab_bar_height = tiling.partition_tree.tab_bar_height;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            let needs_reconcile = self
                .workspaces
                .get(&ws_id)
                .map(|s| s.master_count.is_none() && old_master_count != self.master_count)
                .unwrap_or(false);
            if needs_reconcile {
                self.reconcile_master_count(hub, ws_id);
            }
            self.compute_placement(hub, ws_id);
        }
    }
}

impl MasterStrategy {
    pub(crate) fn new(tiling: &TilingConfig) -> Self {
        Self {
            master_count: tiling.master.master_count,
            master_ratio: tiling.master.master_ratio,
            size_constraints: tiling.size_constraints,
            tab_bar_height: tiling.partition_tree.tab_bar_height,
            workspaces: FxHashMap::default(),
            window_states: FxHashMap::default(),
        }
    }
}

/// The tiling, float and fullscreen windows of one master-stack workspace, with its layout
/// state.
#[derive(Debug)]
struct WorkspaceState {
    master: Pane,
    secondary: Pane,
    /// Windows of this workspace from most to least recently focused. Always set-equal to
    /// the master pane plus the secondary pane.
    focus_history: Vec<WindowId>,
    master_count: Option<usize>,
    master_ratio: Option<f32>,
    /// The layout bounds, a copy of the host monitor's work area.
    work_area: PixelRect,
    /// A copy of the host monitor's scale.
    scale: f32,
    float_windows: FloatWindows,
    fullscreen_windows: FullscreenWindows,
}
