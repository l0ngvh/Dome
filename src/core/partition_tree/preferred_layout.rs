use std::cmp::Ordering;

use crate::config::{LayoutWorkspaceConfig, SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, WindowId, WorkspaceId};
use crate::core::partition_tree::PartitionTreeStrategy;
use crate::core::strategy::TilingStrategy;

impl PartitionTreeStrategy {
    /// Insert a new window matching `slot_id` into the preferred tree when the preferred container
    /// containing both `slot_id` and the current preferred layout root `root_slot` is not yet
    /// materialized. In this case, we will materialize the lowest preferred container that houses
    /// both this new window and the current root
    pub(super) fn attach_window_to_unoccupied_container(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
        root_slot: PreferredSlot,
    ) {
        tracing::debug!(%window_id, ?slot_id, ?root_slot, "Joining window to existing preferred root");
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let layout = ws_state.preferred_layout.as_ref().unwrap();
        let (lowest_common_ancestor, ordering) =
            layout.lowest_common_ancestor(PreferredSlot::Window(slot_id), root_slot);
        let anchor = match root_slot {
            PreferredSlot::Window(root_slot_id) => {
                let root_window_id = layout.occupied_window(root_slot_id).unwrap();
                Child::Window(root_window_id)
            }
            PreferredSlot::Container(root_container_id) => {
                let root_container = layout.occupied_container(root_container_id).unwrap();
                Child::Container(root_container)
            }
        };

        let children = if ordering == Ordering::Less {
            vec![Child::Window(window_id), anchor]
        } else {
            vec![anchor, Child::Window(window_id)]
        };

        let new_container_id = self.replace_anchor_with_container(
            hub,
            anchor,
            children,
            layout.container_slot_split(lowest_common_ancestor),
        );

        let ws_state = self.workspaces.get_mut(&ws_id).unwrap();
        let layout = ws_state.preferred_layout.as_mut().unwrap();
        layout.occupy_container_slot(lowest_common_ancestor, new_container_id);
        layout.occupy_window_slot(slot_id, window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
        self.containers.get_mut(new_container_id).occupy = Some(lowest_common_ancestor);
        ws_state.occupied_preferred_root = Some(PreferredSlot::Container(lowest_common_ancestor));

        self.layout_workspace(hub, ws_id);
        self.set_focus_child(hub, Child::Window(window_id));
    }

    /// Insert a window matched to `slot_id` into the subtree rooted at the
    /// already-occupied `ancestor_slot` container. Walks the container's live
    /// children, computing the nearest shared ancestor in the preferred layout
    /// tree, and either creates a new sub-container (when that ancestor is a
    /// inserts the window at the correct position.
    pub(super) fn attach_window_into_occupied_ancestor(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
        ancestor_slot: PreferredContainerSlotId,
    ) {
        let ws_state = self.workspaces.get(&ws_id).unwrap();
        let layout = ws_state.preferred_layout.as_ref().unwrap();
        let container_id = layout.occupied_container(ancestor_slot).unwrap();
        let live_children = self.containers.get(container_id).children.clone();

        let mut insert_pos = 0;

        for (i, child) in live_children.iter().enumerate() {
            let child_slot = match child {
                Child::Window(wid) => {
                    let Some(slot) = self.tiling_windows.get(wid).unwrap().occupy else {
                        continue;
                    };
                    PreferredSlot::Window(slot)
                }
                Child::Container(cid) => {
                    let Some(slot) = self.containers.get(*cid).occupy else {
                        continue;
                    };
                    PreferredSlot::Container(slot)
                }
            };

            let (lca, ordering) =
                layout.lowest_common_ancestor(PreferredSlot::Window(slot_id), child_slot);

            if layout.is_proper_descendant_of(lca, ancestor_slot) {
                tracing::debug!(%window_id, ?slot_id, ?ancestor_slot, ?lca, ?ordering, "Creating sub-container beneath occupied ancestor");
                let children = if ordering == Ordering::Less {
                    vec![Child::Window(window_id), *child]
                } else {
                    vec![*child, Child::Window(window_id)]
                };

                let new_container_id = self.replace_anchor_with_container(
                    hub,
                    *child,
                    children,
                    layout.container_slot_split(lca),
                );

                let ws_state = self.workspaces.get_mut(&ws_id).unwrap();
                let layout = ws_state.preferred_layout.as_mut().unwrap();
                layout.occupy_container_slot(lca, new_container_id);
                self.mark_slot_occupied_and_focus(hub, window_id, ws_id, slot_id);
                self.containers.get_mut(new_container_id).occupy = Some(lca);
                return;
            }

            if ordering == Ordering::Less {
                insert_pos = i;
                break;
            }
            insert_pos = i + 1;
        }

        tracing::debug!(%window_id, ?slot_id, %container_id, insert_pos, "Inserting window into occupied ancestor container");
        self.attach_child_to_container(Child::Window(window_id), container_id, Some(insert_pos));

        self.mark_slot_occupied_and_focus(hub, window_id, ws_id, slot_id);
    }

    pub(super) fn clean_up_occupied_container(&mut self, container_id: ContainerId) {
        if let Some(slot_id) = self.containers.get(container_id).occupy {
            let ws_id = self.containers.get(container_id).workspace;
            if let Some(ws_state) = self.workspaces.get_mut(&ws_id)
                && let Some(layout) = ws_state.preferred_layout.as_mut()
            {
                layout.clear_container_slot(slot_id);
                if ws_state.occupied_preferred_root == Some(PreferredSlot::Container(slot_id)) {
                    ws_state.occupied_preferred_root = layout.top_occupied_in(slot_id);
                }
            }
            self.containers.get_mut(container_id).occupy = None;
        }
    }

    /// Mark `slot_id` as occupied by `window_id`, trigger layout, and set focus.
    fn mark_slot_occupied_and_focus(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
    ) {
        let ws_state = self.workspaces.get_mut(&ws_id).unwrap();
        ws_state
            .preferred_layout
            .as_mut()
            .unwrap()
            .occupy_window_slot(slot_id, window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
        self.layout_workspace(hub, ws_id);
        self.set_focus_child(hub, Child::Window(window_id));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PreferredWindowSlotId(usize);

impl NodeId for PreferredWindowSlotId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for PreferredWindowSlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreferredWindowSlotId({})", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PreferredContainerSlotId(usize);

impl NodeId for PreferredContainerSlotId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for PreferredContainerSlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreferredContainerSlotId({})", self.0)
    }
}

/// A window slot in the preferred layout tree.
#[derive(Debug, Clone)]
struct PreferredWindowSlot {
    matcher: WindowMatcher,
    occupied: Option<WindowId>,
    parent: Option<PreferredContainerSlotId>,
}

impl Node for PreferredWindowSlot {
    type Id = PreferredWindowSlotId;
}

/// A container slot in the preferred layout tree.
#[derive(Debug, Clone)]
struct PreferredContainerSlot {
    split: Option<SplitMode>,
    children: Vec<PreferredSlot>,
    occupied: Option<ContainerId>,
    parent: Option<PreferredContainerSlotId>,
}

impl Node for PreferredContainerSlot {
    type Id = PreferredContainerSlotId;
}

/// Reference to a child slot within the preferred layout tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreferredSlot {
    Window(PreferredWindowSlotId),
    Container(PreferredContainerSlotId),
}

/// Preferred layout tree for a workspace.
#[derive(Debug)]
pub(super) struct PreferredLayout {
    window_slots: Allocator<PreferredWindowSlot>,
    container_slots: Allocator<PreferredContainerSlot>,
    root: Option<PreferredSlot>,
}

impl Default for PreferredLayout {
    fn default() -> Self {
        Self {
            window_slots: Allocator::new(),
            container_slots: Allocator::new(),
            root: None,
        }
    }
}

impl PreferredLayout {
    pub(super) fn from_tree_layout_node(tree: &TreeLayoutNode) -> Self {
        let mut layout = PreferredLayout::default();
        layout.root = Some(layout.build_subtree(tree, None));
        layout
    }

    /// Find the first free window slot whose matcher matches `metadata`.
    ///
    /// Walks the preferred layout tree in preorder (config order). Skips occupied
    /// window slots. Returns `None` when no free slot matches.
    pub(super) fn find_window_slot(
        &self,
        metadata: &dyn WindowMetadata,
    ) -> Option<PreferredWindowSlotId> {
        let root = self.root.as_ref()?;
        let mut stack = vec![*root];
        for _ in crate::core::bounded_loop() {
            let slot = stack.pop()?;
            match slot {
                PreferredSlot::Window(id) => {
                    let ws = self.window_slots.get(id);
                    if ws.occupied.is_none() && metadata.matches_window_matcher(&ws.matcher) {
                        return Some(id);
                    }
                }
                PreferredSlot::Container(id) => {
                    let cs = self.container_slots.get(id);
                    for &child in cs.children.iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }
        None
    }

    /// Walk up from `slot` to find the first container slot with `occupied` set.
    /// Returns `None` when no ancestor is occupied.
    pub(super) fn first_occupied_ancestor(
        &self,
        slot: PreferredWindowSlotId,
    ) -> Option<PreferredContainerSlotId> {
        let mut current = self.window_slots.get(slot).parent;
        for _ in crate::core::bounded_loop() {
            let Some(parent_id) = current else {
                break;
            };
            let cs = self.container_slots.get(parent_id);
            if cs.occupied.is_some() {
                return Some(parent_id);
            }
            current = cs.parent;
        }
        None
    }

    /// Mark a window slot as occupied by `window_id`.
    pub(super) fn occupy_window_slot(&mut self, slot: PreferredWindowSlotId, window_id: WindowId) {
        self.window_slots.get_mut(slot).occupied = Some(window_id);
    }

    /// Return the window ID occupying `slot`, if any.
    pub(super) fn occupied_window(&self, slot: PreferredWindowSlotId) -> Option<WindowId> {
        self.window_slots.get(slot).occupied
    }

    /// Clear a window slot's occupation.
    pub(super) fn clear_window_slot(&mut self, slot: PreferredWindowSlotId) {
        self.window_slots.get_mut(slot).occupied = None;
    }

    /// Clear a container slot's occupation.
    pub(super) fn clear_container_slot(&mut self, slot: PreferredContainerSlotId) {
        self.container_slots.get_mut(slot).occupied = None;
    }

    /// When a container is cleared, return the first highest occupied node, if any remaining.
    ///
    /// Removing a container can create a situation where multiple occupied children are still
    /// present but their lowest common ancestor isn't manifested, so we must not make any
    /// assumption about the existence of a lowest common ancestor. This, however, can only happen
    /// when users move the occupied children out of this container, causing the container to be
    /// cleaned up while their occupied children are still present.
    ///
    /// Since the return might not be lowest common ancestor of all the remaining occupied
    /// children/descendants, we can't no longer guarantee that all subsequent matched windows will
    /// be inserted forming the intended layout tree.
    ///
    /// This function can return none if all occupied children are removed, leaving only non
    /// preferred children in this container
    pub(super) fn top_occupied_in(
        &self,
        container_id: PreferredContainerSlotId,
    ) -> Option<PreferredSlot> {
        let cs = self.container_slots.get(container_id);
        let mut stack: Vec<PreferredSlot> = cs.children.iter().rev().copied().collect();
        for _ in crate::core::bounded_loop() {
            let slot = stack.pop()?;
            match slot {
                PreferredSlot::Window(wid) => {
                    if self.window_slots.get(wid).occupied.is_some() {
                        return Some(PreferredSlot::Window(wid));
                    }
                }
                PreferredSlot::Container(cid) => {
                    let child_cs = self.container_slots.get(cid);
                    if child_cs.occupied.is_some() {
                        return Some(PreferredSlot::Container(cid));
                    }
                    for &child in child_cs.children.iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }
        None
    }

    /// Return the configured split for a container slot.
    pub(super) fn container_slot_split(&self, slot: PreferredContainerSlotId) -> SplitMode {
        self.container_slots
            .get(slot)
            .split
            .unwrap_or(SplitMode::Horizontal)
    }

    /// Mark a container slot as occupied by `container_id`.
    pub(super) fn occupy_container_slot(
        &mut self,
        slot: PreferredContainerSlotId,
        container_id: ContainerId,
    ) {
        self.container_slots.get_mut(slot).occupied = Some(container_id);
    }

    /// Return the container ID occupying `slot`, if any.
    pub(super) fn occupied_container(&self, slot: PreferredContainerSlotId) -> Option<ContainerId> {
        self.container_slots.get(slot).occupied
    }

    /// Lowest common ancestor of two slots in the preferred layout, and
    /// whether `a` comes before `b` in the LCA's children order. Both
    /// slots must belong to this preferred layout.
    pub(super) fn lowest_common_ancestor(
        &self,
        a: PreferredSlot,
        b: PreferredSlot,
    ) -> (PreferredContainerSlotId, Ordering) {
        let ancestors_a = self.slot_parents(a);
        let ancestors_b = self.slot_parents(b);
        for (i, pa) in ancestors_a.iter().enumerate() {
            if let Some(j) = ancestors_b.iter().position(|pb| pb == pa) {
                let lca = *pa;
                let child_a = if i == 0 {
                    a
                } else {
                    PreferredSlot::Container(ancestors_a[i - 1])
                };
                let child_b = if j == 0 {
                    b
                } else {
                    PreferredSlot::Container(ancestors_b[j - 1])
                };
                let lca_children = &self.container_slots.get(lca).children;
                let pos_a = lca_children.iter().position(|c| *c == child_a).unwrap();
                let pos_b = lca_children.iter().position(|c| *c == child_b).unwrap();
                return (
                    lca,
                    if pos_a < pos_b {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    },
                );
            }
        }
        unreachable!()
    }

    /// Walk up from `slot` to the root, collecting all ancestor container
    /// slot IDs in order (closest ancestor first, root last).
    pub(super) fn slot_parents(&self, slot: PreferredSlot) -> Vec<PreferredContainerSlotId> {
        let mut ancestors = Vec::new();
        let mut current = match slot {
            PreferredSlot::Window(id) => self.window_slots.get(id).parent,
            PreferredSlot::Container(id) => self.container_slots.get(id).parent,
        };
        for _ in crate::core::bounded_loop() {
            let Some(parent_id) = current else {
                break;
            };
            ancestors.push(parent_id);
            current = self.container_slots.get(parent_id).parent;
        }
        ancestors
    }

    /// Check whether `descendant` is a proper descendant of `ancestor` (not
    /// equal). Both must be container slots.
    pub(super) fn is_proper_descendant_of(
        &self,
        descendant: PreferredContainerSlotId,
        ancestor: PreferredContainerSlotId,
    ) -> bool {
        if descendant == ancestor {
            return false;
        }
        let mut current = descendant;
        for _ in crate::core::bounded_loop() {
            match self.container_slots.get(current).parent {
                Some(p) if p == ancestor => return true,
                Some(p) => current = p,
                None => return false,
            }
        }
        false
    }

    /// Structural equality: true when `incoming` describes the same tree
    /// shape, splits, and matchers. Runtime occupation state is ignored.
    pub(super) fn structurally_eq(&self, incoming: &LayoutWorkspaceConfig) -> bool {
        let LayoutWorkspaceConfig::PartitionTree { tree, .. } = incoming else {
            return false;
        };
        let Some(tree) = tree else {
            return self.root.is_none();
        };
        let other = PreferredLayout::from_tree_layout_node(tree);
        match (self.root, other.root) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                let mut stack = vec![(a, b)];
                for _ in crate::core::bounded_loop() {
                    let Some((sa, sb)) = stack.pop() else {
                        return true;
                    };
                    match (sa, sb) {
                        (PreferredSlot::Window(a_id), PreferredSlot::Window(b_id)) => {
                            if self.window_slots.get(a_id).matcher
                                != other.window_slots.get(b_id).matcher
                            {
                                return false;
                            }
                        }
                        (PreferredSlot::Container(a_id), PreferredSlot::Container(b_id)) => {
                            let ca = self.container_slots.get(a_id);
                            let cb = other.container_slots.get(b_id);
                            if ca.split != cb.split || ca.children.len() != cb.children.len() {
                                return false;
                            }
                            for (ac, bc) in ca.children.iter().zip(cb.children.iter()).rev() {
                                stack.push((*ac, *bc));
                            }
                        }
                        _ => return false,
                    }
                }
                true
            }
            _ => false,
        }
    }

    // It should be fine to build a subtree recursively, since the tree layout is parsed from config
    // file, so it should be a standard tree with no loop
    fn build_subtree(
        &mut self,
        node: &TreeLayoutNode,
        parent: Option<PreferredContainerSlotId>,
    ) -> PreferredSlot {
        match node {
            TreeLayoutNode::Leaf(matcher) => {
                let id = self.window_slots.allocate(PreferredWindowSlot {
                    matcher: matcher.clone(),
                    occupied: None,
                    parent,
                });
                PreferredSlot::Window(id)
            }
            TreeLayoutNode::Container { split, children } => {
                let mut child_slots = Vec::with_capacity(children.len());
                // Allocate the container slot first so we have its ID for children.
                let id = self.container_slots.allocate(PreferredContainerSlot {
                    split: *split,
                    children: Vec::new(),
                    occupied: None,
                    parent,
                });
                for c in children {
                    let child_slot = self.build_subtree(c, Some(id));
                    child_slots.push(child_slot);
                }
                // Update children now that they are built.
                self.container_slots.get_mut(id).children = child_slots;
                PreferredSlot::Container(id)
            }
        }
    }
}
