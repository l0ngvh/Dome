mod actions;
mod config;
mod container;
mod float;
mod focus;
mod fullscreen;
mod placement;
mod preferred_layout;
mod scroll;
mod tree;
#[cfg(test)]
mod validate;

use self::preferred_layout::{PreferredContainerSlot, PreferredContainerSlotId, PreferredSlot};
pub(crate) use crate::core::node::Child;
pub(crate) use config::PartitionTreeConfig;
pub(crate) use container::SplitMode;
use container::TilingContainerData;
pub(crate) use preferred_layout::TreeLayoutNode;
use tree::{Parent, TilingWindowData};

use rustc_hash::FxHashMap;

use crate::core::TilingConfig;
use crate::core::allocator::Allocator;
use crate::core::float::FloatWindows;
use crate::core::fullscreen::FullscreenWindows;
use crate::core::hub::{HubAccess, MonitorLayout};
use crate::core::node::{
    ContainerId, DisplayMode, Length, LimitObservation, Logical, PixelRect, Pixels, WindowId,
    WorkspaceId,
};
use crate::core::slot::SlotId;
use crate::core::strategy::{FocusedChild, StrategyAction, TilingStrategy};
use crate::core::{PreferredTiling, PreferredWorkspace, SizeConstraints};

/// i3-style manual tiling strategy. Manages a container tree where windows are
/// leaves and containers define split direction (horizontal/vertical) or tabbed
/// layout. This is the default tiling strategy.
#[derive(Debug)]
pub(crate) struct PartitionTreeStrategy {
    tiling_containers: FxHashMap<ContainerId, TilingContainerData>,
    tiling_windows: FxHashMap<WindowId, TilingWindowData>,
    workspaces: FxHashMap<WorkspaceId, WorkspaceTilingState>,
    leaf_parents: FxHashMap<SlotId, Option<PreferredContainerSlotId>>,
    container_slots: Allocator<PreferredContainerSlot>,
    tab_bar_height: Pixels<Logical>,
    automatic_tiling: bool,
    size_constraints: SizeConstraints,
}

impl TilingStrategy for PartitionTreeStrategy {
    fn prepare_workspace(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        entry: &PreferredWorkspace,
    ) {
        let PreferredTiling::PartitionTree { tree } = &entry.tiling else {
            unreachable!("partition tree got a {:?} entry", entry.tiling.strategy());
        };
        FullscreenWindows::allocate_slots(hub, ws_id, &entry.fullscreen);
        FloatWindows::allocate_slots(hub, ws_id, &entry.float);
        let preferred_root = tree
            .as_ref()
            .map(|t| self.build_preferred_layout(hub, ws_id, t));
        let host = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
        self.workspaces.insert(
            ws_id,
            WorkspaceTilingState {
                root: None,
                focused_tiling: None,
                focus_history: Vec::new(),
                preferred_root,
                viewport_offset: (Length::ZERO, Length::ZERO),
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
        let mut tiling = match state.root {
            Some(root) => self.free_container_subtree(hub, root),
            None => Vec::new(),
        };
        if let Some(preferred_root) = state.preferred_root {
            self.free_preferred_subtree(preferred_root);
        }
        for wid in &tiling {
            self.tiling_windows.remove(wid);
        }
        tiling.reverse();
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
            tiling: PreferredTiling::PartitionTree {
                tree: self.export_tree(hub, ws_id),
            },
            float: state.float_windows.export(hub),
            fullscreen: state.fullscreen_windows.export(hub),
        }
    }

    fn attach_window(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        mode: DisplayMode,
        slot: Option<SlotId>,
    ) {
        match mode {
            DisplayMode::Tiling => self.attach_tiling_window(hub, window_id, ws_id, slot),
            DisplayMode::Float { border_box } => {
                hub.windows.get_mut(window_id).set_workspace(Some(ws_id));
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.float_windows.attach(window_id, border_box);
            }
            DisplayMode::Fullscreen => {
                hub.windows.get_mut(window_id).set_workspace(Some(ws_id));
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.fullscreen_windows.attach(window_id);
            }
        }
    }

    fn attach_container(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        ws_id: WorkspaceId,
    ) {
        // Reversed because a preorder walk yields parents first, and a container must
        // exist before its parent links to it. The root's parent is a placeholder,
        // overwritten by the attach below.
        for cid in hub.containers_preorder(container_id).into_iter().rev() {
            self.tiling_containers.insert(
                cid,
                TilingContainerData::new(Parent::Workspace(ws_id), SplitMode::Horizontal),
            );
            for &member in hub.containers.get(cid).children() {
                match member {
                    Child::Window(wid) => {
                        self.tiling_windows
                            .insert(wid, TilingWindowData::in_container(cid));
                    }
                    Child::Container(nested) => {
                        self.tiling_containers.get_mut(&nested).unwrap().parent =
                            Parent::Container(cid);
                    }
                }
            }
        }
        // Every container was rebuilt with the same default direction, so a nested
        // subtree arrives with a container inside a same-direction container.
        self.maintain_direction_invariance(hub, Parent::Container(container_id));
        let child = Child::Container(container_id);
        self.attach_child_according_to_spawn_direction(hub, child, ws_id);
        self.focus_tiling(hub, ws_id, child);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, window_id: WindowId) -> DisplayMode {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("detaching window has a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        if let Some(border_box) = state.float_windows.detach(window_id) {
            return DisplayMode::Float { border_box };
        }
        if state.fullscreen_windows.detach(window_id) {
            return DisplayMode::Fullscreen;
        }
        self.detach_tiling_window(hub, window_id);
        DisplayMode::Tiling
    }

    fn detach_container(
        &mut self,
        hub: &mut HubAccess,
        container_id: ContainerId,
        _ws_id: WorkspaceId,
    ) {
        let subtree = Child::Container(container_id);
        self.detach_child(hub, subtree);

        // Ordered after the detach, which still reads the state being dropped.
        for node in hub.children_dfs(subtree) {
            match node {
                Child::Window(wid) => {
                    self.tiling_windows.remove(&wid);
                }
                Child::Container(cid) => {
                    self.tiling_containers.remove(&cid);
                }
            }
        }
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

    fn handle_action(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, action: StrategyAction) {
        let Some(focused) = self.focused(ws_id) else {
            return;
        };
        let tiling_has_focus = matches!(focused, FocusedChild::Tiling(_));
        match action {
            StrategyAction::FocusDirection { direction, forward } if tiling_has_focus => {
                self.focus_in_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::MoveDirection { direction, forward } if tiling_has_focus => {
                self.move_in_direction(hub, ws_id, direction, forward)
            }
            StrategyAction::ToggleSpawnMode if tiling_has_focus => self.toggle_spawn_mode(ws_id),
            StrategyAction::ToggleDirection if tiling_has_focus => {
                self.toggle_focused_layout_direction(hub, ws_id)
            }
            StrategyAction::ToggleContainerLayout if tiling_has_focus => {
                self.toggle_focused_container_layout(hub, ws_id)
            }
            StrategyAction::FocusParent if tiling_has_focus => self.focus_parent(hub, ws_id),
            StrategyAction::FocusTab { forward } if tiling_has_focus => {
                self.focus_tab_in_direction(hub, ws_id, forward)
            }
            StrategyAction::FocusDirection { .. }
            | StrategyAction::MoveDirection { .. }
            | StrategyAction::ToggleSpawnMode
            | StrategyAction::ToggleDirection
            | StrategyAction::ToggleContainerLayout
            | StrategyAction::FocusParent
            | StrategyAction::FocusTab { .. } => {
                tracing::debug!("Tiling action while a float or fullscreen window has focus");
            }
            StrategyAction::TabClicked {
                container_id,
                index,
            } => {
                if let FocusedChild::Fullscreen(_) = focused {
                    tracing::debug!("Fullscreen hides the clicked tab bar, dropping the click");
                } else {
                    self.focus_tab_index(hub, ws_id, container_id, index);
                }
            }
            StrategyAction::ToggleFloat => self.toggle_float(hub, ws_id, focused),
            StrategyAction::ToggleFullscreen => self.toggle_fullscreen(hub, focused),
            StrategyAction::GrowMaster
            | StrategyAction::ShrinkMaster
            | StrategyAction::MoreMaster
            | StrategyAction::FewerMaster => {
                tracing::debug!("Master action on a partition-tree workspace");
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
        if state.fullscreen_windows.focus(window_id) {
            state.float_windows.is_float_focused = false;
        } else if !state.float_windows.focus(window_id) {
            self.focus_tiling(hub, ws_id, Child::Window(window_id));
        }
    }

    fn reset_focus(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.float_windows.is_float_focused = state.float_windows.topmost().is_some();
        if let Some(&first) = state.focus_history.first() {
            self.set_focus_pointer(hub, Child::Window(first));
        }
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

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: &TilingConfig) {
        self.tab_bar_height = tiling.partition_tree.tab_bar_height;
        self.automatic_tiling = tiling.partition_tree.automatic_tiling;
        self.size_constraints = tiling.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            self.compute_placement(hub, ws_id);
        }
    }
}

impl PartitionTreeStrategy {
    pub(crate) fn new(tiling: &TilingConfig) -> Self {
        Self {
            tiling_containers: FxHashMap::default(),
            tiling_windows: FxHashMap::default(),
            workspaces: FxHashMap::default(),
            leaf_parents: FxHashMap::default(),
            container_slots: Allocator::new(),
            tab_bar_height: tiling.partition_tree.tab_bar_height,
            automatic_tiling: tiling.partition_tree.automatic_tiling,
            size_constraints: tiling.size_constraints,
        }
    }
}

/// The tiling, float and fullscreen windows of one workspace, with its layout state.
#[derive(Debug)]
struct WorkspaceTilingState {
    root: Option<Child>,
    /// Tiling focus pointer. Usually a `Child::Window` (the focused window). Can be
    /// `Child::Container` for container-highlight mode. `None` in an empty workspace, and in a
    /// nonempty one until its first focus request.
    focused_tiling: Option<Child>,
    /// Windows of this workspace from most to least recently focused. Covers every
    /// tiling window of the workspace.
    focus_history: Vec<WindowId>,
    /// Root of the static preferred layout tree. `None` when no layout is configured.
    preferred_root: Option<PreferredSlot>,
    viewport_offset: (Length, Length),
    /// The layout bounds, a copy of the host monitor's work area.
    work_area: PixelRect,
    /// A copy of the host monitor's scale.
    scale: f32,
    float_windows: FloatWindows,
    fullscreen_windows: FullscreenWindows,
}
