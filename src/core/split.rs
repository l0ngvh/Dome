use crate::core::{
    Child, Container, ContainerId, Hub, SpawnMode, WindowId,
    node::{Direction, DisplayMode, Parent, WorkspaceId},
};

impl Hub {
    pub(super) fn move_in_direction(&mut self, direction: Direction, forward: bool) {
        let current_ws = self.current_workspace();
        let Some(child) = self.focused_split_child_in(current_ws) else {
            return;
        };
        let Parent::Container(direct_parent_id) = self.get_parent(child) else {
            return;
        };

        // Handle swap within same container
        let direct_parent = self.containers.get(direct_parent_id);
        if direct_parent.direction().is_some_and(|d| d == direction) {
            let pos = direct_parent.position_of(child);
            let target_pos = if forward {
                pos + 1
            } else {
                pos.saturating_sub(1)
            };
            if target_pos != pos && target_pos < direct_parent.children.len() {
                tracing::debug!(
                    ?child, from = pos, to = target_pos, %direct_parent_id, "Swapping child position"
                );
                self.containers
                    .get_mut(direct_parent_id)
                    .children
                    .swap(pos, target_pos);
                self.adjust_workspace(current_ws);
                return;
            }
            // At edge, fall through to find ancestor
        }

        let mut current_anchor = Child::Container(direct_parent_id);

        for _ in super::bounded_loop() {
            let parent = self.get_parent(current_anchor);
            match parent {
                Parent::Container(container_id) => {
                    let container = self.containers.get(container_id);

                    if container.direction().is_none_or(|d| d != direction) {
                        current_anchor = Child::Container(container_id);
                        continue;
                    }

                    let pos = container
                        .children
                        .iter()
                        .position(|c| *c == current_anchor)
                        .unwrap();
                    let insert_pos = if forward { pos + 1 } else { pos };

                    tracing::debug!(
                        ?child, from = %direct_parent_id, to = %container_id, insert_pos, "Moving child to ancestor"
                    );
                    self.detach_split_child_from_container(direct_parent_id, child);
                    self.attach_split_child_to_container(child, container_id, Some(insert_pos));
                    self.adjust_workspace(current_ws);
                    self.set_workspace_focus(child);
                    return;
                }
                Parent::Workspace(workspace_id) => {
                    tracing::debug!(?child, %workspace_id, "Moving child to new root container");
                    self.detach_split_child_from_container(direct_parent_id, child);
                    let root = self.workspaces.get(workspace_id).root.unwrap();

                    let children = if forward {
                        vec![root, child]
                    } else {
                        vec![child, root]
                    };
                    let new_root_id = self.replace_anchor_with_container(
                        children,
                        root,
                        SpawnMode::from_direction(direction),
                    );
                    self.workspaces.get_mut(workspace_id).root =
                        Some(Child::Container(new_root_id));

                    self.adjust_workspace(current_ws);
                    self.set_workspace_focus(child);
                    return;
                }
            }
        }
    }

    pub(super) fn toggle_split_direction(&mut self, workspace_id: WorkspaceId) {
        let Some(focused) = self.focused_split_child_in(workspace_id) else {
            return;
        };
        let mut root_id = match focused {
            Child::Container(id) => id,
            Child::Window(_) => {
                let Parent::Container(id) = self.get_parent(focused) else {
                    return;
                };
                id
            }
        };
        for _ in super::bounded_loop() {
            let Parent::Container(parent_id) = self.containers.get(root_id).parent else {
                break;
            };
            if self.containers.get(parent_id).is_tabbed {
                break;
            }
            root_id = parent_id;
        }
        self.containers.get_mut(root_id).toggle_direction();
        self.maintain_direction_invariance(Parent::Container(root_id));
        self.adjust_workspace(workspace_id);
    }

    pub(crate) fn toggle_layout_for_container_with_id(&mut self, container_id: ContainerId) {
        let container = self.containers.get_mut(container_id);
        let ws = container.workspace;
        let direction = container.direction();
        let parent = container.parent;
        container.is_tabbed = !container.is_tabbed;
        tracing::debug!(%container_id, from = ?direction, "Toggled container layout");
        if container.is_tabbed() {
            // Toggled from split to tabbed - find the direct child matching container's focus
            let container = self.containers.get(container_id);
            let active_tab = *container
                .children()
                .iter()
                .find(|c| **c == container.focused || matches!(c, Child::Container(cid) if self.containers.get(*cid).focused == container.focused))
                .unwrap();
            self.containers
                .get_mut(container_id)
                .set_active_tab(active_tab);
        } else {
            // Toggled from tabbed to split
            self.maintain_direction_invariance(Parent::Container(container_id));
        }
        self.maintain_direction_invariance(parent);
        self.adjust_workspace(ws);
    }

    /// Attach child to workspace at focused position. Child must be detached from previous
    /// parent before calling. Sets focus to child.
    pub(super) fn attach_split_child_to_workspace(
        &mut self,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        self.set_workspace(child, workspace_id);
        let insert_anchor = self
            .focused_split_child_in(workspace_id)
            .or(self.workspaces.get(workspace_id).root);
        let Some(insert_anchor) = insert_anchor else {
            self.workspaces.get_mut(workspace_id).root = Some(child);
            self.set_parent(child, Parent::Workspace(workspace_id));
            self.workspaces.get_mut(workspace_id).focused = Some(child);
            self.adjust_workspace(workspace_id);
            return;
        };

        let spawn_mode = match insert_anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };

        if spawn_mode.is_tab()
            && let Some(tabbed_ancestor) = self.find_tabbed_ancestor(insert_anchor)
        {
            let container = self.containers.get(tabbed_ancestor);
            self.attach_split_child_to_container(
                child,
                tabbed_ancestor,
                Some(container.active_tab_index() + 1),
            );
        } else if let Child::Container(cid) = insert_anchor
            && self.containers.get(cid).can_accomodate(spawn_mode)
        {
            self.attach_split_child_to_container(child, cid, None);
        } else {
            match self.get_parent(insert_anchor) {
                Parent::Container(container_id) => {
                    self.try_attach_split_child_to_container_next_to(
                        child,
                        container_id,
                        insert_anchor,
                    );
                }
                Parent::Workspace(workspace_id) => {
                    self.attach_split_child_next_to_workspace_root(child, workspace_id);
                }
            }
        }

        self.adjust_workspace(workspace_id);
        self.set_workspace_focus(child);
    }

    /// Attach child to workspace at focused position. Child must be detached from previous
    /// parent before calling. Sets focus to child.
    pub(super) fn reattach_float_window_as_split(&mut self, window_id: WindowId) {
        let window = self.windows.get_mut(window_id);
        window.mode = DisplayMode::Tiling;
        let ws = window.workspace;
        self.attach_split_child_to_workspace(Child::Window(window_id), ws);
    }

    pub(super) fn detach_split_child_from_workspace(&mut self, child: Child) {
        let parent = self.get_parent(child);
        match parent {
            Parent::Container(parent_id) => {
                let workspace_id = self.containers.get(parent_id).workspace;
                self.detach_split_child_from_container(parent_id, child);
                self.adjust_workspace(workspace_id);
            }
            Parent::Workspace(workspace_id) => {
                self.workspaces.get_mut(workspace_id).root = None;

                // Set focus to fullscreen, then float, otherwise None
                let ws = self.workspaces.get(workspace_id);
                let new_focus = ws
                    .fullscreen_windows
                    .last()
                    .or(ws.float_windows.last())
                    .map(|&f| Child::Window(f));
                self.workspaces.get_mut(workspace_id).focused = new_focus;

                self.adjust_workspace(workspace_id);
            }
        }
    }

    pub(super) fn focus_in_direction(&mut self, direction: Direction, forward: bool) {
        let Some(focused) = self.focused_split_child() else {
            return;
        };

        let mut current = focused;

        for _ in super::bounded_loop() {
            let Parent::Container(container_id) = self.get_parent(current) else {
                return;
            };
            if self
                .containers
                .get(container_id)
                .direction()
                .is_none_or(|d| d != direction)
            {
                current = Child::Container(container_id);
                continue;
            }
            let container = self.containers.get(container_id);
            let pos = container.position_of(current);
            let has_sibling = if forward {
                pos + 1 < container.children.len()
            } else {
                pos > 0
            };
            if has_sibling {
                let sibling_pos = if forward { pos + 1 } else { pos - 1 };
                let sibling = container.children[sibling_pos];
                let focus_target = match sibling {
                    Child::Window(_) => sibling,
                    Child::Container(id) => self.containers.get(id).focused,
                };
                tracing::debug!(?direction, forward, from = ?focused, to = ?focus_target, "Changing focus");
                self.set_workspace_focus(focus_target);
                return;
            }
            current = Child::Container(container_id);
        }
    }

    pub(super) fn focus_tab(&mut self, forward: bool) {
        let Some(focused) = self.focused_split_child() else {
            return;
        };
        let Some(container_id) = self.find_tabbed_ancestor(focused) else {
            return;
        };
        let container = self.containers.get_mut(container_id);
        let new_child = container.switch_tab(forward).unwrap();
        let focus_target = match new_child {
            Child::Window(_) => new_child,
            Child::Container(id) => self.containers.get(id).focused,
        };
        tracing::debug!(forward, %container_id, ?focus_target, "Focusing tab");
        self.set_workspace_focus(focus_target);
    }

    pub(super) fn focus_split_parent(&mut self) {
        let Some(focused) = self.focused_split_child() else {
            return;
        };
        let Parent::Container(container_id) = self.get_parent(focused) else {
            tracing::debug!("Cannot focus parent of workspace root, ignoring");
            return;
        };
        tracing::debug!(parent = %container_id, %focused, "Focusing parent");
        self.set_workspace_focus(Child::Container(container_id));
    }

    /// Attach `child` next to `anchor` in container with id `container_id`, or create a new parent
    /// to house both `child` and `anchor` if any Invariances are violated
    fn try_attach_split_child_to_container_next_to(
        &mut self,
        child: Child,
        container_id: ContainerId,
        anchor: Child,
    ) {
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };
        let parent_container = self.containers.get(container_id);
        if parent_container.can_accomodate(spawn_mode) {
            let anchor_index = self.containers.get(container_id).position_of(anchor);
            self.attach_split_child_to_container(child, container_id, Some(anchor_index + 1));
        } else {
            let new_container_id =
                self.replace_anchor_with_container(vec![anchor, child], anchor, spawn_mode);
            self.containers
                .get_mut(container_id)
                .replace_child(anchor, Child::Container(new_container_id));
        }
    }

    /// Attach child to existing container. Does not change focus.
    fn attach_split_child_to_container(
        &mut self,
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
        match child {
            Child::Window(wid) => {
                self.windows
                    .get_mut(wid)
                    .set_spawn_mode(parent.spawn_mode());
                self.windows.get_mut(wid).parent = Parent::Container(container_id);
            }
            Child::Container(cid) => {
                self.containers.get_mut(cid).parent = Parent::Container(container_id);
            }
        }
        self.maintain_direction_invariance(Parent::Container(container_id));
    }

    fn attach_split_child_next_to_workspace_root(
        &mut self,
        child: Child,
        workspace_id: WorkspaceId,
    ) {
        let ws = self.workspaces.get(workspace_id);
        let anchor = ws.root.unwrap();
        let spawn_mode = match anchor {
            Child::Container(id) => self.containers.get(id).spawn_mode(),
            Child::Window(id) => self.windows.get(id).spawn_mode(),
        };
        let new_container_id =
            self.replace_anchor_with_container(vec![anchor, child], anchor, spawn_mode);
        self.workspaces.get_mut(workspace_id).root = Some(Child::Container(new_container_id));
    }

    /// Replace anchor with a new container containing children.
    /// Gets parent, workspace, and dimension from anchor.
    fn replace_anchor_with_container(
        &mut self,
        children: Vec<Child>,
        anchor: Child,
        spawn_mode: SpawnMode,
    ) -> ContainerId {
        let (parent, workspace_id, dimension) = match anchor {
            Child::Window(wid) => {
                let w = self.windows.get(wid);
                (w.parent, w.workspace, w.dimension)
            }
            Child::Container(cid) => {
                let c = self.containers.get(cid);
                (c.parent, c.workspace, c.dimension)
            }
        };
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
                match child {
                    Child::Window(wid) => {
                        let window = self.windows.get_mut(wid);
                        window.set_spawn_mode(spawn_mode);
                        window.parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
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
                        self.windows.get_mut(wid).set_spawn_mode(spawn_mode);
                        self.windows.get_mut(wid).parent = Parent::Container(container_id);
                    }
                    Child::Container(cid) => {
                        self.containers.get_mut(cid).set_spawn_mode(spawn_mode);
                        self.containers.get_mut(cid).parent = Parent::Container(container_id);
                    }
                }
            }
            container_id
        };
        self.maintain_direction_invariance(Parent::Container(container_id));
        container_id
    }

    /// Detach child from container and replace focus to sibling.
    /// Deletes container if only one child remains.
    fn detach_split_child_from_container(&mut self, container_id: ContainerId, child: Child) {
        tracing::debug!(%child, %container_id, "Detaching child from container");
        // Focus preceded/following sibling if detaching focused window
        let children = &self.containers.get(container_id).children;
        let pos = children.iter().position(|c| *c == child).unwrap();
        let sibling = if pos > 0 {
            children[pos - 1]
        } else {
            children[pos + 1]
        };
        let new_focus = match sibling {
            Child::Window(_) => sibling,
            Child::Container(c) => self.containers.get(c).focused,
        };
        self.replace_split_child_focus(child, new_focus);

        self.containers.get_mut(container_id).remove_child(child);
        if self.containers.get(container_id).children.len() == 1 {
            self.delete_container(container_id);
        }
    }

    /// Delete a container with exactly one child remaining. Promotes the last child to
    /// grandparent.
    fn delete_container(&mut self, container_id: ContainerId) {
        debug_assert_eq!(self.containers.get(container_id).children.len(), 1);
        let container = self.containers.get(container_id);
        let grandparent = container.parent;
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
                .replace_child(Child::Container(container_id), last_child),
            Parent::Workspace(ws) => self.workspaces.get_mut(ws).root = Some(last_child),
        }

        // If this container was being focused, changing focus to last_child regardless of whether
        // it's a container makes sense. Don't need to focus just window here
        self.replace_split_child_focus(Child::Container(container_id), last_child);

        self.containers.delete(container_id);
        self.maintain_direction_invariance(grandparent);
    }

    /// Returns the focused split child (window or container) in the current workspace.
    /// Returns None if no focus or if focused is a float window.
    fn focused_split_child(&self) -> Option<Child> {
        self.focused_split_child_in(self.current_workspace())
    }

    /// Returns the focused split child in the given workspace.
    fn focused_split_child_in(&self, ws_id: WorkspaceId) -> Option<Child> {
        let ws = self.workspaces.get(ws_id);
        match ws.focused {
            Some(Child::Window(id)) if self.windows.get(id).mode != DisplayMode::Tiling => None,
            focused => focused,
        }
    }

    /// Replace all references of old_child, but don't take primary focus unless old_child was the
    /// focus. Given that `A container's focus must either match a child's focus or point directly
    /// to a child`, we can find the highest focusing container
    fn replace_split_child_focus(&mut self, old_child: Child, new_child: Child) {
        let mut current = old_child;
        let mut highest_focusing_container = None;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    if self.containers.get(cid).focused == old_child {
                        highest_focusing_container = Some(cid)
                    } else {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(_) => {
                    highest_focusing_container = None;
                    break;
                }
            }
        }

        let mut current = new_child;
        for _ in super::bounded_loop() {
            match self.get_parent(current) {
                Parent::Container(cid) => {
                    let container = self.containers.get_mut(cid);
                    if container.focused == old_child {
                        if container.is_tabbed {
                            container.set_active_tab(current);
                        }
                        container.focused = new_child;
                    }

                    if highest_focusing_container.is_some_and(|c| c == cid) {
                        break;
                    }
                    current = Child::Container(cid);
                }
                Parent::Workspace(ws) => {
                    let workspace = self.workspaces.get_mut(ws);
                    if workspace.focused == Some(old_child) {
                        workspace.focused = Some(new_child);
                        tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
                    }
                    break;
                }
            }
        }
    }

    /// Ensures all child containers have different direction than their parent.
    /// Skips tabbed containers.
    fn maintain_direction_invariance(&mut self, parent: Parent) {
        let container_id = match parent {
            Parent::Container(id) => id,
            Parent::Workspace(ws_id) => match self.workspaces.get(ws_id).root {
                Some(Child::Container(id)) => id,
                _ => return,
            },
        };
        let mut stack = vec![container_id];
        for _ in super::bounded_loop() {
            let Some(id) = stack.pop() else {
                return;
            };
            let Some(direction) = self.containers.get(id).direction() else {
                continue; // Tabbed container, no invariant needed
            };

            for &child in &self.containers.get(id).children.clone() {
                if let Child::Container(child_id) = child {
                    if self.containers.get(child_id).has_direction(direction) {
                        self.containers.get_mut(child_id).toggle_direction();
                    }
                    stack.push(child_id);
                }
            }
        }
    }

    fn find_tabbed_ancestor(&self, child: Child) -> Option<ContainerId> {
        let mut current = child;
        for _ in super::bounded_loop() {
            if let Child::Container(id) = current
                && self.containers.get(id).is_tabbed
            {
                return Some(id);
            }
            match self.get_parent(current) {
                Parent::Container(id) => current = Child::Container(id),
                Parent::Workspace(_) => return None,
            }
        }
        unreachable!()
    }

    fn set_workspace(&mut self, child: Child, workspace_id: WorkspaceId) {
        let mut stack = vec![child];
        for _ in super::bounded_loop() {
            let Some(current) = stack.pop() else { break };
            match current {
                Child::Window(wid) => {
                    self.windows.get_mut(wid).workspace = workspace_id;
                }
                Child::Container(cid) => {
                    self.containers.get_mut(cid).workspace = workspace_id;
                    stack.extend(self.containers.get(cid).children.iter().copied());
                }
            }
        }
    }

    fn set_parent(&mut self, child: Child, parent: Parent) {
        match child {
            Child::Window(id) => self.windows.get_mut(id).parent = parent,
            Child::Container(id) => self.containers.get_mut(id).parent = parent,
        }
    }
}
