use std::cmp::Ordering;

use crate::config::{LayoutWorkspaceConfig, SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, Direction, WindowId, WorkspaceId};
use crate::core::partition_tree::PartitionTreeStrategy;
use crate::core::strategy::{TilingStrategy, WorkspaceExport};

impl PartitionTreeStrategy {
    pub(super) fn build_preferred_layout(&mut self, tree: &TreeLayoutNode) -> PreferredSlot {
        self.build_preferred_layout_subtree(tree, None)
    }

    pub(super) fn find_window_slot(
        &self,
        root: PreferredSlot,
        metadata: &dyn WindowMetadata,
    ) -> Option<PreferredWindowSlotId> {
        let mut stack = vec![root];
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

    pub(super) fn occupy_window_slot(&mut self, slot: PreferredWindowSlotId, window_id: WindowId) {
        self.window_slots.get_mut(slot).occupied = Some(window_id);
    }

    pub(super) fn clear_window_slot(&mut self, slot: PreferredWindowSlotId) {
        self.window_slots.get_mut(slot).occupied = None;
    }

    pub(super) fn clear_container_slot(&mut self, slot: PreferredContainerSlotId) {
        self.container_slots.get_mut(slot).occupied = None;
    }

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

    pub(super) fn structurally_eq(
        &self,
        root: Option<PreferredSlot>,
        incoming: &LayoutWorkspaceConfig,
    ) -> bool {
        let LayoutWorkspaceConfig::PartitionTree { tree, .. } = incoming else {
            return false;
        };
        let Some(tree) = tree else {
            return root.is_none();
        };
        let Some(self_root) = root else {
            return false;
        };
        let mut other_window_slots: Allocator<PreferredWindowSlot> = Allocator::new();
        let mut other_container_slots: Allocator<PreferredContainerSlot> = Allocator::new();
        let other_root = build_subtree_into(
            &mut other_window_slots,
            &mut other_container_slots,
            tree,
            None,
        );
        let mut stack = vec![(self_root, other_root)];
        for _ in crate::core::bounded_loop() {
            let Some((sa, sb)) = stack.pop() else {
                return true;
            };
            match (sa, sb) {
                (PreferredSlot::Window(a_id), PreferredSlot::Window(b_id)) => {
                    if self.window_slots.get(a_id).matcher != other_window_slots.get(b_id).matcher {
                        return false;
                    }
                }
                (PreferredSlot::Container(a_id), PreferredSlot::Container(b_id)) => {
                    let ca = self.container_slots.get(a_id);
                    let cb = other_container_slots.get(b_id);
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

    pub(super) fn attach_window_to_unoccupied_container(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
        root_slot: PreferredSlot,
    ) {
        tracing::debug!(%window_id, ?slot_id, ?root_slot, "Joining window to existing preferred root");
        let (lowest_common_ancestor, ordering) =
            self.lowest_common_ancestor(PreferredSlot::Window(slot_id), root_slot);
        let anchor = match root_slot {
            PreferredSlot::Window(root_slot_id) => {
                let root_window_id = self.occupied_window(root_slot_id).unwrap();
                Child::Window(root_window_id)
            }
            PreferredSlot::Container(root_container_id) => {
                let root_container = self.occupied_container(root_container_id).unwrap();
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
            self.container_slot_split(lowest_common_ancestor),
        );

        self.occupy_container_slot(lowest_common_ancestor, new_container_id);
        self.occupy_window_slot(slot_id, window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
        self.containers.get_mut(new_container_id).occupy = Some(lowest_common_ancestor);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .occupied_preferred_root = Some(PreferredSlot::Container(lowest_common_ancestor));

        self.compute_placement(hub, ws_id);
        self.set_focus_child(hub, Child::Window(window_id));
    }

    pub(super) fn attach_window_into_occupied_ancestor(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
        ancestor_slot: PreferredContainerSlotId,
    ) {
        let container_id = self.occupied_container(ancestor_slot).unwrap();
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
                self.lowest_common_ancestor(PreferredSlot::Window(slot_id), child_slot);

            if self.is_proper_descendant_of(lca, ancestor_slot) {
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
                    self.container_slot_split(lca),
                );

                self.occupy_container_slot(lca, new_container_id);
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
            let new_occupied_root = self.top_occupied_in(slot_id);
            self.clear_container_slot(slot_id);
            if let Some(ws_state) = self.workspaces.get_mut(&ws_id)
                && ws_state.occupied_preferred_root == Some(PreferredSlot::Container(slot_id))
            {
                ws_state.occupied_preferred_root = new_occupied_root;
            }
            self.containers.get_mut(container_id).occupy = None;
        }
    }

    fn build_preferred_layout_subtree(
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
                let id = self.container_slots.allocate(PreferredContainerSlot {
                    split: *split,
                    children: Vec::new(),
                    occupied: None,
                    parent,
                });
                for c in children {
                    let child_slot = self.build_preferred_layout_subtree(c, Some(id));
                    child_slots.push(child_slot);
                }
                self.container_slots.get_mut(id).children = child_slots;
                PreferredSlot::Container(id)
            }
        }
    }

    fn occupied_window(&self, slot: PreferredWindowSlotId) -> Option<WindowId> {
        self.window_slots.get(slot).occupied
    }

    fn container_slot_split(&self, slot: PreferredContainerSlotId) -> SplitMode {
        self.container_slots
            .get(slot)
            .split
            .unwrap_or(SplitMode::Horizontal)
    }

    fn occupy_container_slot(&mut self, slot: PreferredContainerSlotId, container_id: ContainerId) {
        self.container_slots.get_mut(slot).occupied = Some(container_id);
    }

    fn occupied_container(&self, slot: PreferredContainerSlotId) -> Option<ContainerId> {
        self.container_slots.get(slot).occupied
    }

    fn lowest_common_ancestor(
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

    fn slot_parents(&self, slot: PreferredSlot) -> Vec<PreferredContainerSlotId> {
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

    fn is_proper_descendant_of(
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

    fn mark_slot_occupied_and_focus(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
    ) {
        self.occupy_window_slot(slot_id, window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot_id);
        self.compute_placement(hub, ws_id);
        self.set_focus_child(hub, Child::Window(window_id));
    }

    fn window_slot_matcher(&self, slot: PreferredWindowSlotId) -> &WindowMatcher {
        &self.window_slots.get(slot).matcher
    }

    pub(super) fn build_from_live_tree(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<PreferredSlot> {
        let (root, old_root) = {
            let ws = self.workspaces.get(&ws_id)?;
            (ws.root?, ws.preferred_root)
        };

        let mut stack: Vec<(Option<PreferredContainerSlotId>, Child)> = vec![(None, root)];
        for _ in crate::core::bounded_loop() {
            let Some((parent, child)) = stack.pop() else {
                break;
            };
            match child {
                Child::Window(wid) => {
                    let matcher = {
                        let td = &self.tiling_windows[&wid];
                        if let Some(old) = td.occupy {
                            self.window_slot_matcher(old).clone()
                        } else {
                            hub.windows.get(wid).metadata.to_window_matcher()
                        }
                    };
                    let new_slot = self.window_slots.allocate(PreferredWindowSlot {
                        matcher,
                        occupied: Some(wid),
                        parent,
                    });
                    if let Some(pid) = parent {
                        self.container_slots
                            .get_mut(pid)
                            .children
                            .push(PreferredSlot::Window(new_slot));
                    }
                    self.tiling_windows.get_mut(&wid).unwrap().occupy = Some(new_slot);
                }
                Child::Container(cid) => {
                    let split = {
                        let container = self.containers.get(cid);
                        Some(match container.direction() {
                            Some(Direction::Horizontal) => SplitMode::Horizontal,
                            Some(Direction::Vertical) => SplitMode::Vertical,
                            None => SplitMode::Tabbed,
                        })
                    };
                    let new_slot = self.container_slots.allocate(PreferredContainerSlot {
                        split,
                        children: vec![],
                        occupied: Some(cid),
                        parent,
                    });
                    if let Some(pid) = parent {
                        self.container_slots
                            .get_mut(pid)
                            .children
                            .push(PreferredSlot::Container(new_slot));
                    }
                    self.containers.get_mut(cid).occupy = Some(new_slot);
                    for &c in self.containers.get(cid).children.iter().rev() {
                        stack.push((Some(new_slot), c));
                    }
                }
            }
        }

        if let Some(old) = old_root {
            let mut stack = vec![old];
            for _ in crate::core::bounded_loop() {
                let Some(slot) = stack.pop() else { break };
                match slot {
                    PreferredSlot::Window(id) => self.window_slots.delete(id),
                    PreferredSlot::Container(id) => {
                        let children = self.container_slots.get(id).children.clone();
                        self.container_slots.delete(id);
                        for &c in children.iter().rev() {
                            stack.push(c);
                        }
                    }
                }
            }
        }

        let pref_root = match root {
            Child::Window(wid) => PreferredSlot::Window(self.tiling_windows[&wid].occupy.unwrap()),
            Child::Container(cid) => {
                PreferredSlot::Container(self.containers.get(cid).occupy.unwrap())
            }
        };
        self.workspaces.get_mut(&ws_id).unwrap().preferred_root = Some(pref_root);

        Some(pref_root)
    }

    pub(super) fn build_layout_node(&self, slot: PreferredSlot) -> TreeLayoutNode {
        match slot {
            PreferredSlot::Window(id) => {
                let ws = self.window_slots.get(id);
                TreeLayoutNode::Leaf(ws.matcher.clone())
            }
            PreferredSlot::Container(id) => {
                let cs = self.container_slots.get(id);
                TreeLayoutNode::Container {
                    split: cs.split,
                    children: cs
                        .children
                        .iter()
                        .map(|&c| self.build_layout_node(c))
                        .collect(),
                }
            }
        }
    }

    pub(super) fn export_workspace(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<WorkspaceExport> {
        let root = self.build_from_live_tree(hub, ws_id)?;
        let tree = self.build_layout_node(root);
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(tree),
            ..Default::default()
        })
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
pub(super) struct PreferredWindowSlot {
    matcher: WindowMatcher,
    occupied: Option<WindowId>,
    parent: Option<PreferredContainerSlotId>,
}

impl Node for PreferredWindowSlot {
    type Id = PreferredWindowSlotId;
}

/// A container slot in the preferred layout tree.
#[derive(Debug, Clone)]
pub(super) struct PreferredContainerSlot {
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

fn build_subtree_into(
    window_slots: &mut Allocator<PreferredWindowSlot>,
    container_slots: &mut Allocator<PreferredContainerSlot>,
    node: &TreeLayoutNode,
    parent: Option<PreferredContainerSlotId>,
) -> PreferredSlot {
    match node {
        TreeLayoutNode::Leaf(matcher) => {
            let id = window_slots.allocate(PreferredWindowSlot {
                matcher: matcher.clone(),
                occupied: None,
                parent,
            });
            PreferredSlot::Window(id)
        }
        TreeLayoutNode::Container { split, children } => {
            let mut child_slots = Vec::with_capacity(children.len());
            let id = container_slots.allocate(PreferredContainerSlot {
                split: *split,
                children: Vec::new(),
                occupied: None,
                parent,
            });
            for c in children {
                let child_slot = build_subtree_into(window_slots, container_slots, c, Some(id));
                child_slots.push(child_slot);
            }
            container_slots.get_mut(id).children = child_slots;
            PreferredSlot::Container(id)
        }
    }
}
