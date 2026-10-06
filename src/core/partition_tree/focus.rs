use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, WindowId, WorkspaceId};
use crate::core::strategy::FocusedChild;

use super::{Parent, PartitionTreeStrategy, WorkspaceTilingState};

impl PartitionTreeStrategy {
    /// The topmost fullscreen window, otherwise the topmost float when float focus is selected
    /// or no tiling window remains, otherwise the remembered tiling child.
    pub(super) fn focused(&self, ws_id: WorkspaceId) -> Option<FocusedChild> {
        let state = self.workspaces.get(&ws_id)?;
        if let Some(id) = state.fullscreen_windows.topmost() {
            return Some(FocusedChild::Fullscreen(id));
        }
        let tiling = state.focused_tiling;
        if let Some(id) = state.float_windows.topmost()
            && (state.float_windows.is_float_focused || tiling.is_none())
        {
            return Some(FocusedChild::Float(id));
        }
        tiling.map(FocusedChild::Tiling)
    }

    pub(super) fn focused_child_in(&self, ws_id: WorkspaceId) -> Option<Child> {
        self.workspaces.get(&ws_id).and_then(|s| s.focused_tiling)
    }

    /// Selecting a tiling child clears float focus.
    pub(super) fn focus_tiling(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, child: Child) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.float_windows.is_float_focused = false;
        self.set_focus(hub, child);
    }

    /// Internal set_focus that works with `Child` (window or container).
    pub(super) fn set_focus(&mut self, hub: &mut HubAccess, child: Child) {
        let ws = self.set_focus_pointer(hub, child);
        self.scroll_into_view(hub, ws);
    }

    /// The state half of `set_focus`, without the placement pass. Returns the workspace
    /// so callers do not re-derive it through `hub`, which can disagree with the tree
    /// mid-surgery.
    pub(super) fn set_focus_pointer(&mut self, hub: &HubAccess, child: Child) -> WorkspaceId {
        let path: Vec<_> = self.ancestors_of(child).collect();
        for (walk_pos, parent_id) in &path {
            if self.tiling_containers.get(parent_id).unwrap().is_tabbed {
                self.set_active_tab_to_child(hub, *parent_id, *walk_pos);
            }
        }
        // Workspace-level focus state lives above the container tree.
        // ancestors_of terminates at the workspace boundary, so handle it here.
        let ws_child = match path.last() {
            Some((_, last_pid)) => Child::Container(*last_pid),
            None => child,
        };
        let Parent::Workspace(ws) = self.parent(ws_child) else {
            panic!("set_focus: top of ancestor path has no workspace parent");
        };
        let state = self.workspaces.get_mut(&ws).unwrap();
        state.focused_tiling = Some(child);
        if let Child::Window(wid) = child {
            state.record_focus(wid);
        }
        ws
    }

    /// Focus target when entering `subtree`. Panics if the history does not cover a
    /// container subtree.
    pub(super) fn focus_target_in(&self, hub: &HubAccess, subtree: Child) -> Child {
        let Child::Container(cid) = subtree else {
            return subtree;
        };
        let ws = hub.containers.get(cid).workspace;
        let wid = self
            .last_focused_window_in(ws, cid)
            .expect("focus history covers every window of an in-tree subtree");
        Child::Window(wid)
    }

    /// Skips history entries that live elsewhere in the workspace.
    fn last_focused_window_in(&self, ws: WorkspaceId, subtree: ContainerId) -> Option<WindowId> {
        let history = &self.workspaces.get(&ws)?.focus_history;
        history.iter().copied().find(|&wid| {
            self.ancestors_of(Child::Window(wid))
                .any(|(_, pid)| pid == subtree)
        })
    }
}

impl WorkspaceTilingState {
    pub(super) fn record_focus(&mut self, window_id: WindowId) {
        self.drop_from_history(window_id);
        self.focus_history.insert(0, window_id);
    }

    /// Enrolls as least recently focused without claiming focus. Idempotent, so a
    /// window that never left the workspace keeps its place.
    pub(super) fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    pub(super) fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(pos) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(pos);
        }
    }
}
