use crate::core::{
    Hub, PreferredWorkspace,
    allocator::Node,
    hub::RestrictedAction,
    node::{Child, MonitorId, PixelRect, WindowId, WorkspaceId},
    slot::SlotId,
};

/// Lifecycle state of a workspace relative to its origin monitor.
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
    /// Always identifies a live monitor, even while the origin monitor is unplugged.
    pub(super) monitor: MonitorId,
    pub(super) attachment: Attachment,
}

impl Node for Workspace {
    type Id = WorkspaceId;
}

impl Workspace {
    pub(super) fn new(name: String, monitor: MonitorId) -> Self {
        Self {
            name,
            monitor,
            attachment: Attachment::Attached,
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
        self.move_focused_across_workspaces(current_ws, target_ws);
    }

    /// Creates each workspace that the layout file names under a connected monitor.
    pub(super) fn create_named_workspaces(&mut self) {
        let named: Vec<(MonitorId, String)> = self
            .access
            .preferred_layouts
            .entries()
            .filter_map(|(monitor, name, _)| {
                let monitor_id = self.monitor_id_by_disambiguated_name(monitor)?;
                Some((monitor_id, name.to_string()))
            })
            .collect();
        for (monitor_id, name) in named {
            self.get_or_create_workspace_on(&name, Some(monitor_id));
        }
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
        self.create_workspace(name.to_string(), target)
    }

    /// Allocates a workspace on `monitor` and loads its layout entry, so no workspace exists
    /// without one.
    pub(super) fn create_workspace(&mut self, name: String, monitor: MonitorId) -> WorkspaceId {
        let ws_id = self
            .access
            .workspaces
            .allocate(Workspace::new(name, monitor));
        self.load_layout_entry(ws_id);
        ws_id
    }

    /// Expects the old strategy state and slots to be cleared.
    pub(super) fn rebuild_workspace(
        &mut self,
        ws_id: WorkspaceId,
        windows: Vec<(WindowId, PixelRect)>,
    ) {
        let mut free_slots = self.load_layout_entry(ws_id);
        for (window_id, border_box) in windows {
            let metadata = self.access.windows.get(window_id).metadata.as_ref();
            let slot_position = free_slots.iter().position(|&id| {
                metadata.matches_window_matcher(&self.access.slots.get(id).matcher)
            });
            let slot = slot_position.map(|position| free_slots[position]);
            self.attach_window_to_layout(window_id, ws_id, border_box, slot);
            if let Some(position) = slot_position {
                free_slots.swap_remove(position);
            }
        }
        self.strategies
            .for_workspace_mut(ws_id)
            .reset_focus(&mut self.access, ws_id);
    }

    /// Moves the focused child of `from` to `to`, preserving display mode for windows.
    #[tracing::instrument(skip(self))]
    pub(super) fn move_focused_across_workspaces(&mut self, from: WorkspaceId, to: WorkspaceId) {
        if from == to {
            return;
        }
        let Some(child) = self.strategies.for_workspace(from).focused_child(from) else {
            return;
        };
        match child {
            Child::Window(window_id) => {
                let mode = self
                    .strategies
                    .for_workspace_mut(from)
                    .detach_window(&mut self.access, window_id);
                self.strategies.for_workspace_mut(to).attach_window(
                    &mut self.access,
                    window_id,
                    to,
                    mode,
                    None,
                );
                self.set_workspace_focus(window_id);
                tracing::debug!("Moved to workspace");
            }
            Child::Container(container_id) => {
                self.strategies.for_workspace_mut(from).detach_container(
                    &mut self.access,
                    container_id,
                    from,
                );
                self.strategies.for_workspace_mut(to).attach_container(
                    &mut self.access,
                    container_id,
                    to,
                );
            }
        }
    }

    fn parked_workspace_by_origin(&self, name: &str, origin: &str) -> Option<WorkspaceId> {
        self.access
            .workspaces
            .find(|w| w.name == name && w.origin() == Some(origin))
    }

    /// Loads the workspace's entry in the layout file, or an empty entry for the default
    /// strategy when the file does not name the workspace. Expects a workspace with no
    /// strategy state and no slots, either new or just cleared. Returns its newly allocated
    /// window slots for matching before any windows attach.
    fn load_layout_entry(&mut self, ws_id: WorkspaceId) -> Vec<SlotId> {
        let monitor = self.access.origin_monitor_name(ws_id);
        let name = &self.access.workspaces.get(ws_id).name;
        let entry = self
            .access
            .preferred_layouts
            .workspace(&monitor, name)
            .cloned()
            .unwrap_or_else(|| PreferredWorkspace::empty(self.access.tiling.layout));
        self.strategies
            .prepare_workspace(&mut self.access, ws_id, &entry)
    }
}
