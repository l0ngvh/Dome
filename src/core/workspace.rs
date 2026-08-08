use crate::core::{
    Hub,
    allocator::Node,
    hub::RestrictedAction,
    matcher::FloatFullscreenMatcherId,
    node::{DisplayMode, MonitorId, WindowId, WorkspaceId},
    partition_tree::Child,
};

/// Lifecycle state of a workspace relative to its origin monitor. The enum
/// carries no monitor id: the always-live `Workspace.monitor` field holds the
/// present id, and the `origin` string in the non-Attached variants is a
/// monitor's stored `unique_name`, frozen at unplug time.
#[derive(Debug, Clone)]
pub(super) enum Attachment {
    /// `monitor` is the workspace's origin and present. Normal state.
    Attached,
    /// Origin monitor is gone. `monitor` is rented to the primary so it stays a
    /// live present id; `origin` is the origin monitor's stored `unique_name`,
    /// frozen at unplug for the replug match. Hidden unless it is its rental
    /// host's active workspace, in which case it is temporarily shown (a visit).
    Parked { origin: String },
}

#[derive(Debug, Clone)]
pub(super) struct Workspace {
    pub(super) name: String,
    pub(super) monitor: MonitorId,
    pub(super) attachment: Attachment,
    /// When true, the focused window is float_windows.last().
    /// Wouldn't have any effect when any fullscreen window is present, but for consistency would be
    /// set to false in that case
    pub(super) is_float_focused: bool,
    /// Float ids in this workspace, ordered by z-index (last is topmost).
    /// Each id's screen-absolute rect lives on the window itself, in
    /// `DisplayMode::Float`. Focusing a float moves it to the end.
    pub(super) float_windows: Vec<WindowId>,
    /// All fullscreen windows in this workspace, order by z-index with the last is the top most
    /// window. Only the top most fullscreen window is displayed.
    pub(super) fullscreen_windows: Vec<WindowId>,
    pub(super) float_matchers: Vec<FloatFullscreenMatcherId>,
    pub(super) fullscreen_matchers: Vec<FloatFullscreenMatcherId>,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    pub(super) fn new(name: String, monitor: MonitorId) -> Self {
        Self {
            is_float_focused: false,
            name,
            monitor,
            attachment: Attachment::Attached,
            float_windows: Vec::new(),
            fullscreen_windows: Vec::new(),
            float_matchers: Vec::new(),
            fullscreen_matchers: Vec::new(),
        }
    }

    pub(super) fn is_attached(&self) -> bool {
        matches!(self.attachment, Attachment::Attached)
    }

    pub(super) fn origin(&self) -> Option<&str> {
        match &self.attachment {
            Attachment::Attached => None,
            Attachment::Parked { origin } => Some(origin),
        }
    }
}

impl Hub {
    #[tracing::instrument(skip(self))]
    pub(super) fn focus_workspace_with_id(&mut self, workspace_id: WorkspaceId) {
        tracing::debug!("Focusing workspace");
        let current_ws = self.current_workspace();
        if workspace_id == current_ws {
            return;
        }
        let target_monitor = self.access.workspaces.get(workspace_id).monitor;
        self.access.focused_monitor = target_monitor;
        self.access
            .monitors
            .get_mut(target_monitor)
            .active_workspace = workspace_id;
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn focus_workspace(&mut self, name: &str, monitor: Option<&str>) {
        if self.is_restricted(RestrictedAction::TilingNavigation) {
            return;
        }
        match monitor {
            None => {
                let ws_id = self.get_or_create_workspace_on(name, None);
                self.focus_workspace_with_id(ws_id);
            }
            Some(m) if let Some(mon) = self.monitor_id_by_disambiguated_name(m) => {
                let ws_id = self.get_or_create_workspace_on(name, Some(mon));
                self.focus_workspace_with_id(ws_id);
            }
            Some(m) => {
                // A detached monitor selector brings that monitor's parked
                // workspace into view on the primary it parked onto, by pointing
                // the rental host's active workspace at it. No match means nothing
                // to do, because a workspace cannot be created on a monitor that
                // is gone.
                if let Some(ws_id) = self.parked_workspace_by_origin(name, m) {
                    let host = self.access.workspaces.get(ws_id).monitor;
                    self.access.monitors.get_mut(host).active_workspace = ws_id;
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    #[tracing::instrument(skip(self))]
    pub(crate) fn move_focused_to_workspace(&mut self, target: &str, monitor: Option<&str>) {
        if self.is_restricted(RestrictedAction::WorkspaceMove) {
            return;
        }
        let current_ws = self.current_workspace();
        let target_ws = match monitor {
            None => Some(self.get_or_create_workspace_on(target, None)),
            Some(m) if let Some(mon) = self.monitor_id_by_disambiguated_name(m) => {
                Some(self.get_or_create_workspace_on(target, Some(mon)))
            }
            // A detached monitor selector deposits into that monitor's parked
            // workspace, so the window travels back when the monitor returns. No
            // match means nothing to do, because there is nowhere to put it.
            Some(m) => self.parked_workspace_by_origin(target, m),
        };
        let Some(target_ws) = target_ws else {
            return;
        };
        if let Some(window_id) = self.focused_window(current_ws) {
            self.move_child_to_workspace_with_id(window_id, target_ws);
        } else {
            self.move_focused_across_workspaces(current_ws, target_ws);
        }
    }

    // A parked workspace keeps its origin monitor's name frozen in its origin
    // field, which is how a detached monitor selector is resolved after the
    // monitor itself is gone from the live list.
    fn parked_workspace_by_origin(&self, name: &str, origin: &str) -> Option<WorkspaceId> {
        self.access
            .workspaces
            .find(|w| w.name == name && w.origin() == Some(origin))
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn move_child_to_workspace_with_id(
        &mut self,
        window_id: WindowId,
        target_ws: WorkspaceId,
    ) {
        let current_ws = self.current_workspace();
        if current_ws == target_ws {
            return;
        }

        let window = self.access.windows.get(window_id);
        if window.is_minimized() {
            panic!("Minimized window can't be moved");
        }
        match window.mode {
            DisplayMode::Fullscreen { .. } => {
                self.detach_fullscreen_from_workspace(window_id);
                self.attach_fullscreen_to_workspace(target_ws, window_id, None);
                self.access.workspaces.get_mut(target_ws).is_float_focused = false;
            }
            DisplayMode::Float { .. } => {
                // Cross-workspace hop: drop occupy so the destination does not
                // export the origin workspace's authored matcher.
                let dim = self.detach_float_from_workspace(window_id);
                self.attach_float_to_workspace(target_ws, window_id, dim, None);
            }
            DisplayMode::Tiling => {
                self.move_focused_across_workspaces(current_ws, target_ws);
            }
        }

        tracing::debug!("Moved to workspace");
    }

    // A move destination is always an attached workspace on the target monitor,
    // never a parked one, so a name that collides with a hidden parked workspace
    // still lands on (or creates) the target monitor's own attached workspace.
    // `monitor` is a resolved live id, or None for the
    // focused monitor; the caller resolves any disambiguated name to an id
    // before calling, so this never sees an invalid name.
    pub(super) fn get_or_create_workspace_on(
        &mut self,
        name: &str,
        monitor: Option<MonitorId>,
    ) -> WorkspaceId {
        let target = monitor.unwrap_or(self.access.focused_monitor);
        if let Some(id) = self
            .access
            .workspaces
            .find(|w| w.name == name && w.monitor == target && w.is_attached())
        {
            return id;
        }
        let ws_id = self
            .access
            .workspaces
            .allocate(Workspace::new(name.to_string(), target));
        self.strategies.register(&mut self.access, ws_id);
        ws_id
    }

    pub(super) fn move_focused_across_workspaces(&mut self, from: WorkspaceId, to: WorkspaceId) {
        let strategy = self.strategies.for_workspace_mut(from);
        let child = strategy.detach_focused_child(&mut self.access, from);
        let Some(child) = child else {
            return;
        };
        if strategy.tiling_window_count(&self.access, from) == 0 {
            let ws = self.access.workspaces.get_mut(from);
            if ws.fullscreen_windows.is_empty() {
                ws.is_float_focused = !ws.float_windows.is_empty();
            }
        }
        self.strategies
            .for_workspace_mut(to)
            .reattach_child(&mut self.access, child, to);
        if let Child::Window(window_id) = child {
            self.set_workspace_focus(window_id);
        }
    }
}
