use super::allocator::{Allocator, NodeId};
use super::matcher::WindowMode;
use super::monitor::{Monitor, ReportedMonitor};
use super::node::{
    Container, ContainerId, Direction, DisplayMode, Length, LimitObservation, MonitorId, PixelRect,
    Pixels, Unit, Window, WindowId, WindowMetadata, WindowRestrictions, WorkspaceId,
};
use super::partition_tree::Child;
use super::preferred_layout::{PreferredLayouts, PreferredWorkspace};
use super::slot::{Slot, find_free_slot};
use super::strategy::{StrategyAction, StrategySet, TilingAction};
use super::tiling::TilingConfig;
use super::workspace::{Attachment, Workspace};
use crate::action::{Action, Actions};
use crate::config::lua::ActionContext;
use crate::config::{Config, KeymapEffects, Keystroke, LuaRuntime, PlatformEffects};

pub(crate) struct VisiblePlacements {
    pub(crate) focused_window: Option<WindowId>,
    pub(crate) focused_monitor: MonitorId,
    pub(crate) monitors: Vec<MonitorPlacements>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TilingWindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    pub(crate) content_box: PixelRect,
    /// `content_box` trimmed to the work area. Zero-area when nothing remains.
    pub(crate) visible_content_box: PixelRect,
    /// Highlighting does not require keyboard focus.
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_direction: Option<Direction>,
}

impl TilingWindowPlacement {
    /// Part of `content_box` lies outside the work area. Also true when `content_box` has no
    /// area, because its visible part is then `PixelRect::ZERO`.
    pub(crate) fn is_partially_off_screen(&self) -> bool {
        self.visible_content_box != self.content_box
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatWindowPlacement {
    pub(crate) id: WindowId,
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    pub(crate) content_box: PixelRect,
    pub(crate) is_highlighted: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ContainerPlacement {
    pub(crate) id: ContainerId,
    pub(crate) border_box: PixelRect,
    pub(crate) visible_border_box: PixelRect,
    /// The tab strip at its configured height from the top of `border_box`, zero-height when
    /// the container is not tabbed. A container shorter than the configured height cuts the
    /// strip off rather than squashing it, so the band can run past the bottom of `border_box`.
    pub(crate) tab_bar_band: PixelRect,
    /// `tab_bar_band` cut to `visible_border_box`, or `PixelRect::ZERO` when no part of the
    /// band is inside it.
    pub(crate) visible_tab_bar_band: PixelRect,
    pub(crate) is_highlighted: bool,
    pub(crate) spawn_direction: Option<Direction>,
    pub(crate) is_tabbed: bool,
    pub(crate) active_tab_index: usize,
    pub(crate) titles: Vec<String>,
}

pub(crate) struct MonitorPlacements {
    pub(crate) monitor_id: MonitorId,
    /// The monitor's work area with its reserved area subtracted.
    pub(crate) work_area: PixelRect,
    pub(crate) border_thickness: Pixels<Unit>,
    pub(crate) layout: MonitorLayout,
}

pub(crate) enum MonitorLayout {
    Normal {
        tiling_windows: Vec<TilingWindowPlacement>,
        float_windows: Vec<FloatWindowPlacement>,
        containers: Vec<ContainerPlacement>,
    },
    Fullscreen(WindowId),
}

/// Categorizes restricted operations by what they do, so each restriction level
/// (BlockAll, ProtectFullscreen) can allow or deny them independently.
pub(super) enum RestrictedAction {
    /// Navigate or rearrange within the current tiling paradigm.
    /// Blocked by: BlockAll.
    TilingNavigation,
    /// Change the window's display mode (float, fullscreen).
    /// Blocked by: BlockAll, ProtectFullscreen.
    DisplayModeChange,
    /// Move the window to a different workspace (same or different monitor).
    /// Blocked by: BlockAll only. ProtectFullscreen does NOT block this -- on macOS
    /// and Windows, fullscreen windows can freely move across workspaces.
    WorkspaceMove,
    /// Move the window to a different monitor's active workspace.
    /// Blocked by: BlockAll, ProtectFullscreen. Fullscreen windows are bound to their
    /// monitor -- moving them cross-monitor would break the fullscreen association.
    MonitorMove,
}

fn restriction_of(action: &StrategyAction) -> RestrictedAction {
    match action {
        StrategyAction::ToggleFloat | StrategyAction::ToggleFullscreen => {
            RestrictedAction::DisplayModeChange
        }
        StrategyAction::FocusDirection { .. }
        | StrategyAction::MoveDirection { .. }
        | StrategyAction::ToggleSpawnMode
        | StrategyAction::ToggleDirection
        | StrategyAction::ToggleContainerLayout
        | StrategyAction::FocusParent
        | StrategyAction::FocusTab { .. }
        | StrategyAction::TabClicked { .. }
        | StrategyAction::Grow
        | StrategyAction::Shrink
        | StrategyAction::MoreMaster
        | StrategyAction::FewerMaster => RestrictedAction::TilingNavigation,
    }
}

/// Non-strategy fields of Hub, extracted so that `TilingStrategy` methods can
/// receive `&mut HubAccess` while Hub holds `&mut strategy` separately.
pub(crate) struct HubAccess {
    pub(super) monitors: Allocator<Monitor>,
    pub(super) focused_monitor: MonitorId,
    /// Re-keyed onto a new primary display rather than replaced, so this id
    /// never dies while any display exists.
    pub(super) primary_monitor: MonitorId,
    pub(super) tiling: TilingConfig,
    pub(super) preferred_layouts: PreferredLayouts,
    pub(super) workspaces: Allocator<Workspace>,
    pub(super) windows: Allocator<Window>,
    pub(super) containers: Allocator<Container>,
    pub(super) slots: Allocator<Slot>,
}

impl HubAccess {
    /// The monitor name a workspace's layout entry is keyed under.
    ///
    /// For a parked workspace this is the origin monitor recorded at unplug,
    /// not the monitor now hosting it.
    pub(super) fn origin_monitor_name(&self, ws_id: WorkspaceId) -> String {
        let ws = self.workspaces.get(ws_id);
        match ws.origin() {
            Some(origin) => origin.to_string(),
            None => self.monitors.get(ws.monitor).unique_name.clone(),
        }
    }

    pub(super) fn allocate_container(
        &mut self,
        children: Vec<Child>,
        workspace: WorkspaceId,
    ) -> ContainerId {
        self.containers.allocate(Container {
            children,
            workspace,
        })
    }

    pub(super) fn free_container(&mut self, id: ContainerId) {
        self.containers.delete(id);
    }

    pub(super) fn containers_preorder(&self, root: ContainerId) -> Vec<ContainerId> {
        let mut stack = vec![root];
        let mut order = Vec::new();
        for _ in super::bounded_loop() {
            let Some(id) = stack.pop() else { break };
            order.push(id);
            for &child in &self.containers.get(id).children {
                if let Child::Container(child_id) = child {
                    stack.push(child_id);
                }
            }
        }
        order
    }

    pub(super) fn children_dfs(&self, root: Child) -> Vec<Child> {
        let mut stack = vec![root];
        let mut order = Vec::new();
        for _ in super::bounded_loop() {
            let Some(child) = stack.pop() else { break };
            order.push(child);
            if let Child::Container(cid) = child {
                for &c in &self.containers.get(cid).children {
                    stack.push(c);
                }
            }
        }
        order
    }

    pub(super) fn take_windows(&mut self, root: Child) -> Vec<WindowId> {
        let mut windows = Vec::new();
        for child in self.children_dfs(root) {
            match child {
                Child::Window(wid) => windows.push(wid),
                Child::Container(cid) => self.free_container(cid),
            }
        }
        windows
    }
}

impl HubAccess {
    /// Rounding here rather than at the call sites is what makes
    /// `border_box - content_box` exactly the thickness on every edge: a
    /// thickness ending in `.5` would otherwise round the two opposite edges
    /// apart by a pixel.
    pub(super) fn border(&self, monitor: MonitorId) -> Pixels<Unit> {
        self.border_for_scale(self.monitors.get(monitor).scale)
    }

    /// Lets a caller that already holds the scale skip the monitor lookup `border` does.
    pub(super) fn border_for_scale(&self, scale: f32) -> Pixels<Unit> {
        Pixels::round(Length::from_pixels(self.tiling.border_size).to_unit(scale))
    }
}

pub(crate) struct Hub {
    pub(super) access: HubAccess,
    pub(super) strategies: StrategySet,
    /// Each minimized window with the display mode it returns in.
    pub(super) minimized_windows: Vec<(WindowId, DisplayMode)>,
    pub(super) runtime: LuaRuntime,
}

impl Hub {
    pub(crate) fn new(
        primary: ReportedMonitor,
        tiling: TilingConfig,
        preferred_layouts: PreferredLayouts,
        runtime: LuaRuntime,
    ) -> Self {
        let strategies = StrategySet::new(&tiling);

        let mut hub = Self {
            access: HubAccess {
                monitors: Allocator::new(),
                // Placeholder ids. Both are set once the primary monitor exists.
                focused_monitor: MonitorId::new(0),
                primary_monitor: MonitorId::new(0),
                tiling,
                preferred_layouts,
                workspaces: Allocator::new(),
                windows: Allocator::new(),
                containers: Allocator::new(),
                slots: Allocator::new(),
            },
            strategies,
            minimized_windows: Vec::new(),
            runtime,
        };

        let primary_id = hub.add_monitor(primary);
        hub.access.focused_monitor = primary_id;
        hub.access.primary_monitor = primary_id;
        hub
    }

    #[tracing::instrument(skip(self, effects, keymap_effects))]
    pub(crate) fn run_binding(
        &mut self,
        keymap: &str,
        keystroke: &Keystroke,
        effects: &mut dyn PlatformEffects,
        keymap_effects: &mut dyn KeymapEffects,
    ) {
        // A reload between the keypress and here can drop the binding, so a miss
        // is expected rather than a bug.
        let Some(binding) = self.runtime.binding(keymap, keystroke) else {
            tracing::warn!(%keymap, %keystroke, "Binding is gone, dropping");
            return;
        };
        let mut cx = ActionContext {
            hub: self,
            effects,
            keymap_effects,
        };
        if let Err(e) = binding.call(&mut cx) {
            tracing::warn!(%keymap, %keystroke, error = %e, "Callback handler errored");
        }
    }

    #[tracing::instrument(skip(self, keymap_effects))]
    pub(crate) fn reload_config(
        &mut self,
        keymap_effects: &mut dyn KeymapEffects,
    ) -> Option<Box<Config>> {
        let config = self.runtime.reload(keymap_effects)?;
        self.sync_configuration(config.tiling.clone());
        // Last, so each monitor whose work area moved is placed from the new work
        // area and the new tiling config.
        self.rederive_monitors();
        Some(config)
    }

    #[tracing::instrument(skip(self, effects, keymap_effects))]
    pub(crate) fn handle_actions(
        &mut self,
        actions: &Actions,
        effects: &mut dyn PlatformEffects,
        keymap_effects: &mut dyn KeymapEffects,
    ) {
        for action in actions {
            match action {
                Action::Focus { target } => self.handle_tiling_action(target),
                Action::Move { target } => self.handle_tiling_action(target),
                Action::Toggle { target } => self.handle_tiling_action(target),
                Action::Master { target } => self.handle_tiling_action(target),
                Action::Grow => self.handle_tiling_action(StrategyAction::Grow),
                Action::Shrink => self.handle_tiling_action(StrategyAction::Shrink),
                Action::Execute { command } => effects.execute(command),
                Action::Exit => {
                    tracing::debug!("Exit action received");
                    effects.exit();
                }
                Action::Close => self.close_focused_window(effects),
                Action::UnminimizeWindow { id } => effects.unminimize(*id),
                Action::Mode { name } => {
                    tracing::debug!(mode = %name, "Switching to mode");
                    keymap_effects.switch_mode(name);
                }
            }
        }
    }

    #[tracing::instrument(skip(self, effects))]
    pub(crate) fn close_focused_window(&self, effects: &mut dyn PlatformEffects) {
        if let Some(id) = self.focused_window(self.current_workspace()) {
            effects.close(id);
        }
    }

    pub(crate) fn current_workspace(&self) -> WorkspaceId {
        self.access
            .monitors
            .get(self.access.focused_monitor)
            .active_workspace
    }

    /// The window that has keyboard focus on the workspace, in any display mode. `None` when the
    /// workspace is empty or a container has focus.
    pub(crate) fn focused_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        match self.strategies.for_workspace(ws_id).focused_child(ws_id)? {
            Child::Window(id) => Some(id),
            Child::Container(_) => None,
        }
    }

    pub(super) fn is_restricted(&self, action: RestrictedAction) -> bool {
        let ws_id = self.current_workspace();
        let Some(id) = self.focused_window(ws_id) else {
            return false;
        };
        let restrictions = self.access.windows.get(id).restrictions;
        match action {
            RestrictedAction::TilingNavigation | RestrictedAction::WorkspaceMove => {
                restrictions == WindowRestrictions::BlockAll
            }
            RestrictedAction::DisplayModeChange | RestrictedAction::MonitorMove => {
                restrictions != WindowRestrictions::None
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn handle_tiling_action(
        &mut self,
        action: impl Into<TilingAction> + std::fmt::Debug,
    ) {
        match action.into() {
            TilingAction::Strategy(action) => {
                if self.is_restricted(restriction_of(&action)) {
                    return;
                }
                let ws_id = self.current_workspace();
                self.strategies.for_workspace_mut(ws_id).handle_action(
                    &mut self.access,
                    ws_id,
                    action,
                );
            }
            TilingAction::FocusWorkspace { name, monitor } => {
                self.focus_workspace(&name, monitor.as_deref())
            }
            TilingAction::MoveToWorkspace { name, monitor } => {
                self.move_focused_to_workspace(&name, monitor.as_deref())
            }
            TilingAction::FocusMonitor { selector } => self.focus_monitor(&selector),
            TilingAction::MoveToMonitor { selector } => self.move_focused_to_monitor(&selector),
        }
    }

    /// Activates tab `index` of `container_id` on the workspace that holds the container, which
    /// can be on a monitor without focus. Does nothing when the focused window blocks tiling
    /// navigation, or when the container no longer exists.
    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_tab_index(&mut self, container_id: ContainerId, index: usize) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        if !self.access.containers.contains(container_id) {
            tracing::debug!("Clicked container no longer exists, dropping the click");
            return;
        }
        let ws_id = self.access.containers.get(container_id).workspace;
        self.strategies.for_workspace_mut(ws_id).handle_action(
            &mut self.access,
            ws_id,
            StrategyAction::TabClicked {
                container_id,
                index,
            },
        );
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_focus(&mut self, window_id: WindowId) {
        tracing::debug!("Setting focus to window");
        let ws = self
            .access
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        self.set_workspace_focus(window_id);
        self.focus_workspace_with_id(ws);
    }

    /// Focus `window_id` within its own workspace, without switching workspace.
    pub(super) fn set_workspace_focus(&mut self, window_id: WindowId) {
        let ws = self
            .access
            .windows
            .get(window_id)
            .workspace()
            .expect("non-minimized window has a workspace");
        self.strategies
            .for_workspace_mut(ws)
            .set_focus(&mut self.access, window_id);
    }

    pub(crate) fn primary_monitor(&self) -> MonitorId {
        self.access.primary_monitor
    }

    pub(crate) fn visible_workspaces(&self) -> Vec<WorkspaceId> {
        self.access
            .monitors
            .sorted_ids()
            .into_iter()
            .map(|id| self.access.monitors.get(id).active_workspace)
            .collect()
    }

    /// Returns each workspace that is visible or holds a window, in creation order.
    pub(crate) fn query_workspaces(&self) -> Vec<crate::action::WorkspaceInfo> {
        let focused_ws = self.current_workspace();
        let visible: Vec<WorkspaceId> = self.visible_workspaces();
        self.access
            .workspaces
            .sorted_ids()
            .into_iter()
            .filter_map(|ws_id| {
                let ws = self.access.workspaces.get(ws_id);
                let is_visible = visible.contains(&ws_id);
                if !is_visible && !self.workspace_has_windows(ws_id) {
                    return None;
                }
                let (monitor, state) = match &ws.attachment {
                    Attachment::Attached => (
                        self.access.monitors.get(ws.monitor).unique_name.clone(),
                        crate::action::WorkspaceState::Attached,
                    ),
                    Attachment::Parked { origin } => {
                        (origin.clone(), crate::action::WorkspaceState::Parked)
                    }
                };
                Some(crate::action::WorkspaceInfo {
                    name: ws.name.clone(),
                    monitor,
                    state,
                    is_focused: ws_id == focused_ws,
                    is_visible,
                })
            })
            .collect()
    }

    /// Returns all active monitors, ordered left to right. `unique_name` ranks
    /// its `#N` suffixes by that same order, so generated config agrees with the
    /// names inside it.
    pub(crate) fn query_monitors(&self) -> Vec<crate::action::MonitorDetails> {
        let mut ids = self.access.monitors.sorted_ids();
        ids.sort_by_key(|&id| {
            let system_work_area = self.access.monitors.get(id).system_work_area;
            (system_work_area.x(), system_work_area.y())
        });
        ids.into_iter()
            .map(|id| {
                let m = self.access.monitors.get(id);
                crate::action::MonitorDetails {
                    device_name: m.device_name.clone(),
                    unique_name: m.unique_name.clone(),
                    cg_display_id: m.cg_display_id,
                    gdi_device: m.gdi_device.clone(),
                    work_area: crate::action::MonitorFrame {
                        x: m.work_area.x().value(),
                        y: m.work_area.y().value(),
                        width: m.work_area.width().value(),
                        height: m.work_area.height().value(),
                    },
                }
            })
            .collect()
    }

    pub(super) fn workspace_has_windows(&self, ws_id: WorkspaceId) -> bool {
        self.strategies
            .for_workspace(ws_id)
            .focused_child(ws_id)
            .is_some()
    }

    pub(crate) fn export_workspace(&self, ws_id: WorkspaceId) -> PreferredWorkspace {
        self.strategies
            .for_workspace(ws_id)
            .export_workspace(&self.access, ws_id)
    }

    pub(crate) fn sync_configuration(&mut self, tiling: TilingConfig) {
        self.access.tiling = tiling.clone();
        self.strategies.apply_config(&mut self.access, &tiling);
    }

    /// Resets every workspace from `preferred_layouts`, including a workspace that no
    /// entry names. A workspace that an entry names but that does not exist yet is
    /// created from that entry.
    #[tracing::instrument(skip(self))]
    pub(crate) fn apply_preferred_layouts(&mut self, preferred_layouts: PreferredLayouts) {
        self.access.preferred_layouts = preferred_layouts;
        self.create_named_workspaces();
        for ws_id in self.access.workspaces.sorted_ids() {
            self.reset_workspace(ws_id);
        }
    }

    #[cfg(test)]
    pub(crate) fn validate(&self) {
        let owners = self.strategies.validate(&self.access);
        for window_id in self.access.windows.sorted_ids() {
            let window = self.access.windows.get(window_id);
            assert_eq!(
                owners.get(&window_id).copied(),
                window.workspace(),
                "{window_id} records workspace {:?} but sits in the strategy of {:?}",
                window.workspace(),
                owners.get(&window_id)
            );
        }
        for &(window_id, mode) in &self.minimized_windows {
            assert!(
                self.access.windows.get(window_id).restrictions == WindowRestrictions::None
                    || mode == DisplayMode::Fullscreen,
                "{window_id} has restrictions but was minimized as {mode}"
            );
        }
        self.validate_slots();
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_window(
        &mut self,
        metadata: Box<dyn WindowMetadata>,
        rect: PixelRect,
        restrictions: WindowRestrictions,
    ) -> Option<WindowId> {
        if let Some(r) = self
            .access
            .tiling
            .ignore
            .iter()
            .find(|r| metadata.matches_window_matcher(r))
        {
            tracing::debug!("Window ignored by rule {r:?}");
            return None;
        }
        let slot = find_free_slot(&self.access.slots, &*metadata, None);
        let (target_ws, slot_mode) = match slot {
            Some(id) => {
                let slot = self.access.slots.get(id);
                (slot.workspace, slot.mode)
            }
            None => (self.current_workspace(), WindowMode::Tiling),
        };
        let mode = if restrictions == WindowRestrictions::None {
            slot_mode
        } else {
            WindowMode::Fullscreen
        };

        let window_id =
            self.access
                .windows
                .allocate(Window::new(target_ws, restrictions, metadata));
        let (mode, tiling_slot) = match mode {
            WindowMode::Tiling => (DisplayMode::Tiling, slot),
            WindowMode::Float => {
                tracing::debug!(%window_id, ?rect, "Inserting float window");
                (DisplayMode::Float { border_box: rect }, None)
            }
            WindowMode::Fullscreen => (DisplayMode::Fullscreen, None),
        };
        self.strategies.for_workspace_mut(target_ws).attach_window(
            &mut self.access,
            window_id,
            target_ws,
            mode,
            tiling_slot,
        );
        if let Some(id) = slot {
            self.access.slots.get_mut(id).hold(window_id);
        }
        self.set_focus(window_id);

        Some(window_id)
    }

    pub(crate) fn set_window_title(&mut self, window_id: WindowId, title: String) -> bool {
        let window = self.access.windows.get_mut(window_id);
        if window.metadata.title() == Some(&title) {
            return false;
        }
        window.metadata.set_title(title);
        true
    }

    pub(crate) fn get_visible_placements(&self) -> VisiblePlacements {
        let current_ws = self.current_workspace();

        let monitors: Vec<MonitorPlacements> = self
            .visible_workspaces()
            .into_iter()
            .map(|ws_id| {
                let monitor_id = self.access.workspaces.get(ws_id).monitor;
                let screen = self.access.monitors.get(monitor_id).work_area;

                // A work area with zero width or height fits no window.
                if screen.is_empty() {
                    return MonitorPlacements {
                        monitor_id,
                        work_area: screen,
                        border_thickness: self.access.border(monitor_id),
                        layout: MonitorLayout::Normal {
                            tiling_windows: Vec::new(),
                            float_windows: Vec::new(),
                            containers: Vec::new(),
                        },
                    };
                }

                MonitorPlacements {
                    monitor_id,
                    work_area: screen,
                    border_thickness: self.access.border(monitor_id),
                    layout: self.strategies.for_workspace(ws_id).collect_placements(
                        &self.access,
                        ws_id,
                        ws_id == current_ws,
                    ),
                }
            })
            .collect();

        let focused_window = self.focused_window(current_ws).filter(|&id| {
            monitors.iter().any(|placements| match &placements.layout {
                MonitorLayout::Fullscreen(fullscreen_id) => *fullscreen_id == id,
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => {
                    tiling_windows.iter().any(|placement| placement.id == id)
                        || float_windows.iter().any(|placement| placement.id == id)
                }
            })
        });

        VisiblePlacements {
            focused_window,
            focused_monitor: self.access.focused_monitor,
            monitors,
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn delete_window(&mut self, id: WindowId) {
        let window = self.access.windows.get(id);
        if window.is_minimized() {
            self.minimized_windows.retain(|&(w, _)| w != id);
        } else {
            let ws_id = window
                .workspace()
                .expect("non-minimized window has a workspace");
            self.strategies
                .for_workspace_mut(ws_id)
                .detach_window(&mut self.access, id);
        }

        self.release_slot(id);
        self.access.windows.delete(id);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn set_window_constraint(
        &mut self,
        window_id: WindowId,
        observed: LimitObservation,
    ) {
        match self.access.windows.get(window_id).workspace() {
            Some(ws_id) => self
                .strategies
                .for_workspace_mut(ws_id)
                .update_window_size_limits(&mut self.access, window_id, observed),
            // A window without a workspace is minimized.
            None => self
                .access
                .windows
                .get_mut(window_id)
                .limits
                .apply_observation(observed),
        }
        tracing::debug!("Window constraint set");
    }
}
