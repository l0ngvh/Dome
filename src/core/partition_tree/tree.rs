use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Length, WorkspaceId};
use crate::core::partition_tree::{Child, Parent, SpawnMode};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn ancestors_of(
        &self,
        start: Child,
    ) -> impl Iterator<Item = (Child, ContainerId)> + '_ {
        let mut current = Some(start);
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let child = current?;
            match self.parent(child) {
                Parent::Container(pid) => {
                    current = Some(Child::Container(pid));
                    Some((child, pid))
                }
                Parent::Workspace(_) => {
                    current = None;
                    None
                }
            }
        })
    }

    pub(super) fn containers_preorder(
        &self,
        root: ContainerId,
    ) -> impl Iterator<Item = ContainerId> + '_ {
        let mut stack = vec![root];
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let id = stack.pop()?;
            for &child in &self.containers.get(id).children {
                if let Child::Container(child_id) = child {
                    stack.push(child_id);
                }
            }
            Some(id)
        })
    }

    pub(super) fn children_dfs(&self, root: Child) -> impl Iterator<Item = Child> + '_ {
        let mut stack = vec![root];
        let mut bound = crate::core::bounded_loop();
        std::iter::from_fn(move || {
            bound.next()?;
            let child = stack.pop()?;
            if let Child::Container(cid) = child {
                for &c in &self.containers.get(cid).children {
                    stack.push(c);
                }
            }
            Some(child)
        })
    }

    pub(super) fn parent(&self, child: Child) -> Parent {
        match child {
            Child::Window(id) => self.tiling_data(id).parent,
            Child::Container(id) => self.containers.get(id).parent,
        }
    }

    pub(super) fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.tiling_data_mut(id).parent = parent,
            Child::Container(id) => self.containers.get_mut(id).parent = parent,
        }
    }

    pub(super) fn child_dimension(&self, child: Child) -> Dimension {
        match child {
            Child::Window(id) => self.tiling_data(id).dimension,
            Child::Container(id) => self.containers.get(id).dimension,
        }
    }

    pub(super) fn child_workspace(&self, hub: &HubAccess, child: Child) -> WorkspaceId {
        match child {
            Child::Window(id) => hub
                .windows
                .get(id)
                .workspace()
                .expect("tiling window has a workspace"),
            Child::Container(id) => self.containers.get(id).workspace,
        }
    }

    pub(super) fn child_spawn_mode(&self, child: Child) -> SpawnMode {
        match child {
            Child::Window(id) => self.tiling_data(id).spawn_mode,
            Child::Container(id) => self.containers.get(id).spawn_mode(),
        }
    }

    /// Window mins are raw f32 platform hints. Wraps at this seam so every
    /// caller works in `Length`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "only called from validate.rs which is #[cfg(test)]"
        )
    )]
    pub(super) fn child_min_size(&self, hub: &HubAccess, child: Child) -> (Length, Length) {
        match child {
            Child::Window(id) => {
                let (w, h) = hub.windows.get(id).min_size();
                (Length::new(w), Length::new(h))
            }
            Child::Container(id) => self.containers.get(id).min_size(),
        }
    }

    /// One step down the focus chain. Does not recurse.
    pub(super) fn descend_to_focused(&self, child: Child) -> Child {
        match child {
            Child::Window(_) => child,
            Child::Container(id) => self.containers.get(id).focused,
        }
    }

    pub(super) fn assign_subtree_to_workspace(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let nodes: Vec<_> = self.children_dfs(child).collect();
        for node in nodes {
            match node {
                Child::Window(wid) => {
                    hub.windows.get_mut(wid).set_workspace(Some(workspace_id));
                }
                Child::Container(cid) => {
                    self.containers.get_mut(cid).workspace = workspace_id;
                }
            }
        }
    }

    pub(super) fn find_tabbed_self_or_ancestor(&self, child: Child) -> Option<ContainerId> {
        if let Child::Container(id) = child
            && self.containers.get(id).is_tabbed
        {
            return Some(id);
        }
        self.ancestors_of(child)
            .map(|(_, pid)| pid)
            .find(|&pid| self.containers.get(pid).is_tabbed)
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    pub(super) fn maintain_direction_invariance(&mut self, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.ws_state(ws_id).root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let order: Vec<_> = self.containers_preorder(container_id).collect();
        for id in order {
            let Some(direction) = self.containers.get(id).direction() else {
                continue;
            };
            for &child in &self.containers.get(id).children.clone() {
                if let Child::Container(child_id) = child
                    && self.containers.get(child_id).has_direction(direction)
                {
                    self.containers.get_mut(child_id).toggle_direction();
                }
            }
        }
    }

    /// Replace anchor with a new container containing children.
    /// Gets parent, workspace, and dimension from anchor.
    pub(super) fn replace_anchor_with_container(
        &mut self,
        hub: &mut HubAccess,
        children: Vec<Child>,
        anchor: Child,
        spawn_mode: SpawnMode,
    ) -> ContainerId {
        use crate::core::partition_tree::Container;

        let parent = self.parent(anchor);
        let workspace_id = self.child_workspace(hub, anchor);
        let dimension = self.child_dimension(anchor);
        let container_id = if let Some(direction) = spawn_mode.as_direction() {
            let container_id = self.containers.allocate(Container::split(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
                direction,
            ));
            for child in children {
                if let Child::Window(wid) = child {
                    self.tiling_data_mut(wid).spawn_mode = SpawnMode::without_history(spawn_mode);
                }
                self.set_parent(child, Parent::Container(container_id));
            }
            container_id
        } else {
            let container_id = self.containers.allocate(Container::tabbed(
                parent,
                workspace_id,
                children.clone(),
                anchor,
                dimension,
            ));
            for child in children {
                match child {
                    Child::Window(wid) => {
                        self.tiling_data_mut(wid).spawn_mode =
                            SpawnMode::without_history(spawn_mode);
                    }
                    Child::Container(cid) => {
                        self.containers
                            .get_mut(cid)
                            .set_spawn_mode_reset(spawn_mode);
                    }
                }
                self.set_parent(child, Parent::Container(container_id));
            }
            container_id
        };
        self.maintain_direction_invariance(Parent::Container(container_id));
        container_id
    }

    /// Attach child to existing container. Does not change focus.
    pub(super) fn attach_child_to_container(
        &mut self,
        _hub: &mut HubAccess,
        child: Child,
        container_id: ContainerId,
        insert_pos: Option<usize>,
    ) {
        let parent = self.containers.get_mut(container_id);
        if let Some(pos) = insert_pos {
            parent.children.insert(pos, child);
        } else {
            parent.children.push(child);
        }
        let container_spawn_mode = self.containers.get(container_id).spawn_mode();
        if let Child::Window(wid) = child {
            self.tiling_data_mut(wid).spawn_mode = SpawnMode::without_history(container_spawn_mode);
        }
        self.set_parent(child, Parent::Container(container_id));
        self.maintain_direction_invariance(Parent::Container(container_id));
    }

    /// Attach `child` next to `anchor` in container, or create a new parent
    /// to house both if the container can't accommodate the spawn mode.
    pub(super) fn attach_child_next_to_anchor(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        container_id: ContainerId,
        anchor: Child,
    ) {
        let spawn_mode = self.child_spawn_mode(anchor);
        let parent_container = self.containers.get(container_id);
        if parent_container.can_accommodate(spawn_mode) {
            let anchor_index = self.containers.get(container_id).position_of(anchor);
            self.attach_child_to_container(hub, child, container_id, Some(anchor_index + 1));
        } else {
            let new_container_id =
                self.replace_anchor_with_container(hub, vec![anchor, child], anchor, spawn_mode);
            self.containers
                .get_mut(container_id)
                .replace_child_if_present(anchor, Child::Container(new_container_id));
        }
    }

    pub(super) fn attach_child_next_to_workspace_root(
        &mut self,
        hub: &mut HubAccess,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let anchor = self.ws_state(workspace_id).root.unwrap();
        let spawn_mode = self.child_spawn_mode(anchor);
        let new_container_id =
            self.replace_anchor_with_container(hub, vec![anchor, child], anchor, spawn_mode);
        self.ws_state_mut(workspace_id).root = Some(Child::Container(new_container_id));
    }

    /// Maintain invariant 3 of `Container` after replacing `old_child` with
    /// `new_child` in the tree. Two-walk algorithm:
    ///
    /// Walk 1 (scope): from `old_child`, find the highest ancestor still
    /// focusing `old_child`. Stops at the first ancestor that does not focus
    /// `old_child`. If the walk reaches the workspace, scope is the entire
    /// path (and `focused_tiling` is also checked in walk 2).
    ///
    /// Walk 2 (replace): from `new_child`, rewrite `focused = new_child` and
    /// update active tabs on every ancestor that still has `focused ==
    /// old_child`. Stops at the scope boundary from walk 1. If scope reached
    /// the workspace, also replaces `focused_tiling` if it pointed to
    /// `old_child`.
    fn replace_child_focus(&mut self, old_child: Child, new_child: Child) {
        // Walk 1 (scope): find how far up old_child's focus extends.
        // focus_chain_top = None means scope reaches the workspace.
        let mut focus_chain_top = None;
        let mut reached_workspace = true;
        for (_, parent_id) in self.ancestors_of(old_child) {
            if self.containers.get(parent_id).focused == old_child {
                focus_chain_top = Some(parent_id);
            } else {
                reached_workspace = false;
                break;
            }
        }
        if reached_workspace {
            focus_chain_top = None;
        }

        // Walk 2 (replace): walk up from new_child, replacing focus references.
        let path: Vec<_> = self.ancestors_of(new_child).collect();
        let mut hit_boundary = false;
        for (walk_pos, parent_id) in &path {
            let container = self.containers.get_mut(*parent_id);
            if container.focused == old_child {
                if container.is_tabbed {
                    container.set_active_tab_to_child(*walk_pos);
                }
                container.focused = new_child;
            }
            if focus_chain_top.is_some_and(|c| c == *parent_id) {
                hit_boundary = true;
                break;
            }
        }
        // If scope reached workspace and walk 2 didn't hit a boundary, update workspace focus.
        if !hit_boundary {
            let ws_child = match path.last() {
                Some((_, last_pid)) => Child::Container(*last_pid),
                None => new_child,
            };
            if let Parent::Workspace(ws) = self.parent(ws_child)
                && self.ws_state(ws).focused_tiling == Some(old_child)
            {
                self.ws_state_mut(ws).focused_tiling = Some(new_child);
                tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
            }
        }
    }

    /// Detach child from container and replace focus to sibling.
    /// Deletes container if only one child remains.
    pub(super) fn detach_child_from_container(&mut self, container_id: ContainerId, child: Child) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        let children = &self.containers.get(container_id).children;
        let pos = children.iter().position(|c| *c == child).unwrap();
        let sibling = if pos > 0 {
            children[pos - 1]
        } else {
            children[pos + 1]
        };
        let new_focus = self.descend_to_focused(sibling);
        self.replace_child_focus(child, new_focus);

        self.containers.get_mut(container_id).remove_child(child);
        if self.containers.get(container_id).children.len() == 1 {
            self.delete_container(container_id);
        }
    }

    /// Delete a container with exactly one child remaining. Promotes the last
    /// child to grandparent.
    fn delete_container(&mut self, container_id: ContainerId) {
        debug_assert_eq!(self.containers.get(container_id).children.len(), 1);
        let grandparent = self.containers.get(container_id).parent;
        let last_child = self
            .containers
            .get_mut(container_id)
            .children
            .pop()
            .unwrap();

        tracing::debug!(%container_id, %last_child, "Container has one child left, cleaning up");
        self.set_parent(last_child, grandparent);
        match grandparent {
            Parent::Container(gp) => self
                .containers
                .get_mut(gp)
                .replace_child_if_present(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.ws_state_mut(ws).root = Some(last_child),
        }

        self.replace_child_focus(Child::Container(container_id), last_child);

        self.containers.delete(container_id);
        self.maintain_direction_invariance(grandparent);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Container, TilingWindowData, WorkspaceTilingState};
    use super::*;
    use crate::core::allocator::NodeId;
    use crate::core::node::{ContainerId, Dimension, Direction, WindowId, WorkspaceId};
    use crate::core::partition_tree::{Child, Parent, SpawnMode};

    fn ws_id() -> WorkspaceId {
        WorkspaceId::new(0)
    }

    /// Build: workspace -> container A (H) -> container B (V) -> window W
    fn fixture_linear() -> PartitionTreeStrategy {
        let mut s = PartitionTreeStrategy::new(Length::ZERO, false);
        let ws = ws_id();

        let wid = WindowId::new(0);
        let dim = Dimension::default();

        let b = s.containers.allocate(Container::split(
            Parent::Workspace(ws),
            ws,
            vec![Child::Window(wid)],
            Child::Window(wid),
            dim,
            Direction::Vertical,
        ));
        let a = s.containers.allocate(Container::split(
            Parent::Workspace(ws),
            ws,
            vec![Child::Container(b)],
            Child::Window(wid),
            dim,
            Direction::Horizontal,
        ));
        s.containers.get_mut(b).parent = Parent::Container(a);

        s.tiling_windows.insert(
            wid,
            TilingWindowData {
                parent: Parent::Container(b),
                dimension: dim,
                spawn_mode: SpawnMode::horizontal(),
            },
        );
        s.workspaces.insert(
            ws,
            WorkspaceTilingState {
                root: Some(Child::Container(a)),
                focused_tiling: Some(Child::Window(wid)),
                ..Default::default()
            },
        );
        s
    }

    /// Build a wider tree for preorder/dfs tests:
    ///   workspace -> root(H) -> [mid(V) -> [W0, W1], W2, leaf(H) -> [W3]]
    /// 3 containers (root, mid, leaf), 4 windows (W0..W3)
    fn fixture_wide() -> (PartitionTreeStrategy, ContainerId, ContainerId, ContainerId) {
        let mut s = PartitionTreeStrategy::new(Length::ZERO, false);
        let ws = ws_id();
        let dim = Dimension::default();

        let w0 = WindowId::new(0);
        let w1 = WindowId::new(1);
        let w2 = WindowId::new(2);
        let w3 = WindowId::new(3);

        let mid = s.containers.allocate(Container::split(
            Parent::Workspace(ws),
            ws,
            vec![Child::Window(w0), Child::Window(w1)],
            Child::Window(w0),
            dim,
            Direction::Vertical,
        ));
        let leaf = s.containers.allocate(Container::split(
            Parent::Workspace(ws),
            ws,
            vec![Child::Window(w3)],
            Child::Window(w3),
            dim,
            Direction::Horizontal,
        ));
        let root = s.containers.allocate(Container::split(
            Parent::Workspace(ws),
            ws,
            vec![
                Child::Container(mid),
                Child::Window(w2),
                Child::Container(leaf),
            ],
            Child::Window(w0),
            dim,
            Direction::Horizontal,
        ));

        s.containers.get_mut(mid).parent = Parent::Container(root);
        s.containers.get_mut(leaf).parent = Parent::Container(root);

        for (wid, parent) in [
            (w0, Parent::Container(mid)),
            (w1, Parent::Container(mid)),
            (w2, Parent::Container(root)),
            (w3, Parent::Container(leaf)),
        ] {
            s.tiling_windows.insert(
                wid,
                TilingWindowData {
                    parent,
                    dimension: dim,
                    spawn_mode: SpawnMode::horizontal(),
                },
            );
        }

        s.workspaces.insert(
            ws,
            WorkspaceTilingState {
                root: Some(Child::Container(root)),
                focused_tiling: Some(Child::Window(w0)),
                ..Default::default()
            },
        );

        (s, root, mid, leaf)
    }

    #[test]
    fn ancestors_of_walks_up_to_workspace() {
        let s = fixture_linear();
        let wid = WindowId::new(0);
        let b = ContainerId::new(0);
        let a = ContainerId::new(1);

        let path: Vec<_> = s.ancestors_of(Child::Window(wid)).collect();
        assert_eq!(
            path,
            vec![(Child::Window(wid), b), (Child::Container(b), a)]
        );
    }

    #[test]
    fn containers_preorder_visits_every_container_under_root() {
        let (s, root, mid, leaf) = fixture_wide();

        let ids: Vec<_> = s.containers_preorder(root).collect();

        // Preorder with Vec::pop visits last-pushed children first.
        // Root's children pushed in order [mid, W2, leaf], so leaf pops first.
        assert_eq!(ids, vec![root, leaf, mid]);
    }

    #[test]
    fn children_dfs_visits_every_child_in_preorder() {
        let (s, root, mid, leaf) = fixture_wide();
        let w0 = WindowId::new(0);
        let w1 = WindowId::new(1);
        let w2 = WindowId::new(2);
        let w3 = WindowId::new(3);

        let items: Vec<_> = s.children_dfs(Child::Container(root)).collect();

        // Vec::pop reverses push order at each level.
        // root pushes [mid, W2, leaf] -> pops leaf, W2, mid.
        // leaf pushes [W3] -> pops W3.
        // mid pushes [W0, W1] -> pops W1, W0.
        assert_eq!(
            items,
            vec![
                Child::Container(root),
                Child::Container(leaf),
                Child::Window(w3),
                Child::Window(w2),
                Child::Container(mid),
                Child::Window(w1),
                Child::Window(w0),
            ]
        );
    }
}
