use super::preferred_layout::PreferredContainerSlotId;
use crate::core::allocator::Node;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WorkspaceId};
use crate::core::partition_tree::{Child, Parent, PartitionTreeStrategy, SpawnMode};

impl PartitionTreeStrategy {
    /// Delete a container with exactly one child remaining. Promotes the last
    /// child to grandparent.
    pub(super) fn delete_container(&mut self, container_id: ContainerId) {
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
            Parent::Workspace(ws) => self.workspaces.get_mut(&ws).unwrap().root = Some(last_child),
        }

        self.replace_child_focus(Child::Container(container_id), last_child);

        self.clean_up_occupied_container(container_id);
        self.containers.delete(container_id);
        self.maintain_direction_invariance(grandparent);
    }

    /// Attach child to existing container. Does not change focus.
    pub(super) fn attach_child_to_container(
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
        let container_spawn_mode = self.containers.get(container_id).spawn_mode();
        if let Child::Window(wid) = child {
            self.tiling_windows.get_mut(&wid).unwrap().spawn_mode =
                SpawnMode::without_history(container_spawn_mode);
        }
        self.set_parent(child, Parent::Container(container_id));
        self.maintain_direction_invariance(Parent::Container(container_id));
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
                && self.workspaces.get(&ws).unwrap().focused_tiling == Some(old_child)
            {
                self.workspaces.get_mut(&ws).unwrap().focused_tiling = Some(new_child);
                tracing::debug!(?old_child, ?new_child, "Workspace focus replaced");
            }
        }
    }
}

/// Internal node of the partition tree. Holds an ordered list of `Child`
/// nodes (windows or sub-containers) and either a split direction or the
/// tabbed flag.
///
/// Invariants:
/// 1. `children.len() >= 2`. Containers with one or zero children are
///    collapsed by `tree.rs` on detach.
/// 2. A non-tabbed container's `direction` differs from its non-tabbed
///    parent's direction. A tabbed container is exempt: `direction()`
///    returns `None` for it, so the alternation rule does not apply
///    across a tabbed boundary. `validate_container_direction`
///    (`validate.rs`) enforces this.
/// 3. Focus-chain invariant: every container on the path from workspace
///    root to the focused leaf has `focused` set to the same `Child`
///    value (the focused leaf, or a `Child::Container` after
///    `focus_parent`). See `WorkspaceTilingState::focused_tiling`
///    (`mod.rs`) for the workspace-level pointer that anchors this
///    chain.
#[derive(Debug, Clone)]
pub(crate) struct Container {
    pub(super) parent: Parent,
    pub(super) workspace: WorkspaceId,
    pub(super) children: Vec<Child>,
    /// The focused descendant per invariant 3 above. Not necessarily a
    /// direct child: in a chain `root -> A -> B -> W`, all of
    /// `root.focused`, `A.focused`, `B.focused` equal `Child::Window(W)`.
    pub(super) focused: Child,
    pub(super) dimension: Dimension,
    /// Split axis. Read through `direction()`, which returns `None` when
    /// `is_tabbed` is set. A value is stored while tabbed to keep the field
    /// initialised, but it is unused until the container converts back to split.
    direction: Direction,
    /// Spawn mode for new children inserted under this container. Mutate via
    /// `set_spawn_mode_reset` (drops history) or `set_spawn_mode_keep_history`
    /// (preserves history). Direct field write would lose the `H <-> V <-> Tab`
    /// rotation state.
    spawn_mode: SpawnMode,
    pub(super) is_tabbed: bool,
    pub(super) active_tab_index: usize,
    pub(super) min_width: Length,
    pub(super) min_height: Length,
    /// Preferred container slot this live container materializes, if any.
    pub(super) occupy: Option<PreferredContainerSlotId>,
}

impl Node for Container {
    type Id = ContainerId;
}

impl Container {
    /// Build a split container. `focused` must be one of `children` (or a
    /// descendant under one of them) so invariant 3 holds at construction.
    /// `direction` seeds `spawn_mode` to match.
    pub(super) fn new(
        parent: Parent,
        workspace: WorkspaceId,
        children: Vec<Child>,
        focused: Child,
        split_mode: crate::config::SplitMode,
    ) -> Self {
        match split_mode {
            crate::config::SplitMode::Horizontal => Self {
                children,
                focused,
                parent,
                workspace,
                dimension: Dimension::default(),
                direction: Direction::Horizontal,
                spawn_mode: SpawnMode::horizontal(),
                is_tabbed: false,
                active_tab_index: 0,
                min_width: Length::ZERO,
                min_height: Length::ZERO,
                occupy: None,
            },
            crate::config::SplitMode::Vertical => Self {
                children,
                focused,
                parent,
                workspace,
                dimension: Dimension::default(),
                direction: Direction::Vertical,
                spawn_mode: SpawnMode::vertical(),
                is_tabbed: false,
                active_tab_index: 0,
                min_width: Length::ZERO,
                min_height: Length::ZERO,
                occupy: None,
            },
            crate::config::SplitMode::Tabbed => Self {
                children,
                focused,
                parent,
                workspace,
                dimension: Dimension::default(),
                direction: Direction::Horizontal,
                spawn_mode: SpawnMode::tabbed(),
                is_tabbed: true,
                active_tab_index: 0,
                min_width: Length::ZERO,
                min_height: Length::ZERO,
                occupy: None,
            },
        }
    }

    pub(crate) fn is_tabbed(&self) -> bool {
        self.is_tabbed
    }

    pub(crate) fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub(crate) fn active_tab(&self) -> Option<Child> {
        if self.is_tabbed {
            Some(self.children[self.active_tab_index])
        } else {
            None
        }
    }

    pub(crate) fn set_active_tab_to_child(&mut self, child: Child) {
        if !self.is_tabbed {
            panic!("Calling set_active_tab_to_child on split container");
        }
        self.active_tab_index = self.children.iter().position(|c| *c == child).unwrap();
    }

    pub(super) fn switch_tab(&mut self, forward: bool) -> Option<Child> {
        if !self.is_tabbed {
            return None;
        }
        let len = self.children.len();
        let current = self.active_tab_index;
        let new_tab = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.active_tab_index = new_tab;
        Some(self.children[new_tab])
    }

    pub(super) fn set_active_tab_by_index(&mut self, index: usize) -> Option<Child> {
        if !self.is_tabbed || index >= self.children.len() {
            return None;
        }
        self.active_tab_index = index;
        Some(self.children[index])
    }

    pub(crate) fn children(&self) -> &[Child] {
        &self.children
    }

    pub(crate) fn min_size(&self) -> (Length, Length) {
        (self.min_width, self.min_height)
    }

    pub(super) fn direction(&self) -> Option<Direction> {
        if self.is_tabbed {
            None
        } else {
            Some(self.direction)
        }
    }

    pub(super) fn can_accommodate(&self, spawn_mode: SpawnMode) -> bool {
        spawn_mode
            .as_direction()
            .is_some_and(|d| self.has_direction(d))
            || (spawn_mode.is_tab() && self.is_tabbed())
    }

    pub(super) fn has_direction(&self, direction: Direction) -> bool {
        if self.is_tabbed {
            false
        } else {
            self.direction == direction
        }
    }

    pub(crate) fn spawn_mode(&self) -> SpawnMode {
        self.spawn_mode
    }

    pub(super) fn set_spawn_mode_reset(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = SpawnMode::without_history(spawn_mode)
    }

    pub(crate) fn set_spawn_mode_keep_history(&mut self, spawn_mode: SpawnMode) {
        self.spawn_mode = self.spawn_mode.switch_to(spawn_mode)
    }

    pub(super) fn position_of(&self, child: Child) -> usize {
        self.children.iter().position(|c| *c == child).unwrap()
    }

    pub(super) fn remove_child(&mut self, child: Child) {
        let pos = self.children.iter().position(|c| *c == child).unwrap();
        self.children.remove(pos);
        if self.is_tabbed && pos <= self.active_tab_index {
            self.active_tab_index = self.active_tab_index.saturating_sub(1);
        }
    }

    pub(super) fn replace_child_if_present(&mut self, old: Child, new: Child) {
        if let Some(pos) = self.children.iter().position(|c| *c == old) {
            self.children[pos] = new;
        }
    }

    pub(super) fn toggle_direction(&mut self) -> Direction {
        self.direction = match self.direction {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        };
        self.direction
    }
}
