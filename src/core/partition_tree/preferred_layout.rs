//! Materializes the preferred layout onto the live tiling tree as windows
//! arrive.
//!
//! A preferred layout is a tree of named slots. Each window slot has a
//! matcher. A container slot wraps children and sets a split direction. A
//! slot holds any number of matching windows. Three terminal windows that
//! all match the same slot will sit next to each other.
//!
//! The workspace records which part of the configured tree is already
//! materialized. When a new window arrives the dispatcher picks the first
//! matching branch:
//!
//! - No preferred root is configured, or the window does not match any
//!   slot. The window lands through the workspace's ordinary spawn mode
//!   and the preferred layout is not involved.
//! - No slot is occupied yet. This is the first matching window
//!   workspace-wide. It lands through spawn mode, the slot becomes the
//!   occupied root, and the workspace records it as
//!   `occupied_preferred_root`.
//! - There are other windows matched this slot. The window joins the existing
//!   same-slot cluster through `attach_window_into_same_slot`, which
//!   inserts it after the most recent sibling in that cluster. If that sibling
//!   is alone, the two are wrapped in a fresh container, occupying the lowest
//!   container slot housing this window slot, ready to houses next windows
//!   matching this or other child window slots.
//! - The matched slot has an occupied ancestor in the preferred tree.
//!   `attach_window_into_occupied_ancestor` looks at the ancestor's
//!   direct children for one whose preferred slot and the new window's
//!   slot share a lowest common ancestor that is a strict proper
//!   descendant of the occupied ancestor.
//!   - None found. The window is inserted into the ancestor's live
//!     container at the position that keeps the preferred tree order.
//!   - Found. The direct child housing that picked slot is moved with the new
//!     window into a fresh sub-container at that same lowest common ancestor.
//! - None of the matched slot ancestor's has been occupied. This means that the
//!   matched slot and the current occupied preferred root lives in two different
//!   subtree of the preferred layout. Their lowest common ancestor is then
//!   materialized through `attach_window_to_unoccupied_container`.

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::config::{LayoutWorkspaceConfig, SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::node::{Child, ContainerId, Direction, WindowId, WorkspaceId};
use crate::core::partition_tree::Parent;
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
                    if metadata.matches_window_matcher(&ws.matcher) {
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
        self.window_slots.get_mut(slot).windows.push(window_id);
        self.tiling_windows.get_mut(&window_id).unwrap().occupy = Some(slot);
    }

    pub(super) fn clear_window_slot(&mut self, slot: PreferredWindowSlotId, window_id: WindowId) {
        let ws = self.window_slots.get_mut(slot);
        ws.windows.retain(|w| w != &window_id);
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
                    if !self.window_slots.get(wid).windows.is_empty() {
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

    pub(super) fn attach_window_into_same_slot(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
    ) {
        tracing::debug!("Attaching window {window_id} to shared {slot_id} on {ws_id}");
        let slot = self.window_slots.get(slot_id);
        let last_sibling_wid = slot.windows.last().copied().unwrap();
        let last_sibling = Child::Window(last_sibling_wid);
        let new_child = Child::Window(window_id);

        if slot.windows.len() == 1 {
            let children = vec![last_sibling, new_child];
            if let Some(parent_slot) = slot.parent {
                // Forming the lowest housing container, for all windows in this slot, and all other slots
                // inside this container
                let split = self.container_slot_split(parent_slot);
                let c_id = self.replace_anchor_with_container(hub, last_sibling, children, split);
                self.occupy_container_slot(parent_slot, c_id);
            } else {
                let split_mode = self.child_spawn_mode(last_sibling).into();
                self.replace_anchor_with_container(hub, last_sibling, children, split_mode);
            }
        } else {
            // 2 or more windows in this workspace, so this window's parent must be a container.
            let Parent::Container(parent_cid) = self.parent(last_sibling) else {
                unreachable!();
            };
            let pos = self.containers.get(parent_cid).position_of(last_sibling) + 1;
            self.attach_child_to_container(new_child, parent_cid, Some(pos));
        }
        self.occupy_window_slot(slot_id, window_id);
        self.compute_placement(hub, ws_id);
        self.set_focus(hub, new_child);
    }

    /// When lowest common ancestor of the being inserted window and the current preferred root is
    /// not yet constructed.
    pub(super) fn attach_window_to_unoccupied_container(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: PreferredWindowSlotId,
        root_slot: PreferredSlot,
    ) {
        tracing::debug!(%window_id, ?slot_id, ?root_slot, "Joining window to existing preferred root");
        let (lca, ordering) =
            self.lowest_common_ancestor(PreferredSlot::Window(slot_id), root_slot);
        let split = self.container_slot_split(lca);

        match root_slot {
            PreferredSlot::Window(root_slot_id) => {
                let slot = self.window_slots.get(root_slot_id);
                let first_matched_window_id = slot.windows.first().copied().unwrap();
                let first_matched_window =
                    self.tiling_windows.get(&first_matched_window_id).unwrap();
                let anchor = if slot.windows.len() == 1 {
                    Child::Window(first_matched_window_id)
                } else {
                    match first_matched_window.parent {
                        // Since the preferred root is still a window, this mean this is a bare
                        // preferred window slot (with no preferred container)
                        Parent::Container(container_id) => Child::Container(container_id),
                        // 2 or more windows in this workspace, so this window's parent must be a container.
                        Parent::Workspace(_) => unreachable!(),
                    }
                };

                let children = if ordering == Ordering::Less {
                    vec![Child::Window(window_id), anchor]
                } else {
                    vec![anchor, Child::Window(window_id)]
                };
                let c_id = self.replace_anchor_with_container(hub, anchor, children, split);
                self.occupy_container_slot(lca, c_id);
            }
            PreferredSlot::Container(root_container_id) => {
                let anchor_cid = self
                    .occupied_container(root_container_id)
                    .expect("occupied preferred root");
                let children = if ordering == Ordering::Less {
                    vec![Child::Window(window_id), Child::Container(anchor_cid)]
                } else {
                    vec![Child::Container(anchor_cid), Child::Window(window_id)]
                };
                let t_id = self.replace_anchor_with_container(
                    hub,
                    Child::Container(anchor_cid),
                    children,
                    split,
                );
                self.occupy_container_slot(lca, t_id);
            }
        }

        self.occupy_window_slot(slot_id, window_id);
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .occupied_preferred_root = Some(PreferredSlot::Container(lca));

        self.compute_placement(hub, ws_id);
        self.set_focus(hub, Child::Window(window_id));
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

        for (i, &child) in live_children.iter().enumerate() {
            let Some(child_slot) = self.preferred_slot_of_child(child) else {
                continue;
            };
            let (lca, ordering) =
                self.lowest_common_ancestor(PreferredSlot::Window(slot_id), child_slot);

            if self.is_proper_descendant_of(lca, ancestor_slot) {
                let children = if ordering == Ordering::Less {
                    vec![Child::Window(window_id), child]
                } else {
                    vec![child, Child::Window(window_id)]
                };

                let new_container_id = self.replace_anchor_with_container(
                    hub,
                    child,
                    children,
                    self.container_slot_split(lca),
                );
                self.occupy_container_slot(lca, new_container_id);
                self.mark_slot_occupied_and_focus(hub, window_id, ws_id, slot_id);
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

    pub(super) fn detach_preferred_slot(&mut self, workspace_id: WorkspaceId, child: Child) {
        let children: Vec<_> = self.children_dfs(child).collect();
        for child in children {
            match child {
                Child::Window(wid) => {
                    let slot_id = self.tiling_windows.get(&wid).unwrap().occupy;
                    if let Some(slot_id) = slot_id {
                        self.clear_window_slot(slot_id, wid);
                        self.tiling_windows.get_mut(&wid).unwrap().occupy = None;
                        let slot_empty = self.window_slots.get(slot_id).windows.is_empty();
                        let fallback_root =
                            match self.workspaces.get(&workspace_id).unwrap().preferred_root {
                                Some(PreferredSlot::Container(cs_id)) => {
                                    self.top_occupied_in(cs_id)
                                }
                                _ => None,
                            };
                        let ws_state = self.workspaces.get_mut(&workspace_id).unwrap();
                        if slot_empty
                            && ws_state.occupied_preferred_root
                                == Some(PreferredSlot::Window(slot_id))
                        {
                            ws_state.occupied_preferred_root = fallback_root;
                        }
                    }
                }
                Child::Container(cid) => {
                    self.clean_up_occupied_container(cid);
                }
            }
        }
    }

    pub(super) fn clean_up_occupied_container(&mut self, container_id: ContainerId) {
        if let Some(slot_id) = self.containers.get(container_id).occupy {
            let ws_id = self.containers.get(container_id).workspace;
            let new_occupied_root = self.top_occupied_in(slot_id);
            self.clear_container_slot(slot_id);
            self.containers.get_mut(container_id).occupy = None;
            if let Some(ws_state) = self.workspaces.get_mut(&ws_id)
                && ws_state.occupied_preferred_root == Some(PreferredSlot::Container(slot_id))
            {
                ws_state.occupied_preferred_root = new_occupied_root;
            }
        }
    }

    pub(super) fn export_workspace(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> WorkspaceExport {
        let tree = self
            .build_from_live_tree(hub, ws_id)
            .map(|root| self.build_layout_node(root));
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree,
            ..Default::default()
        }
    }

    pub(super) fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&LayoutWorkspaceConfig>,
    ) {
        let Some(incoming) = incoming else {
            return;
        };
        let incoming_tree = match incoming {
            LayoutWorkspaceConfig::PartitionTree {
                tree: Some(tree), ..
            } => Some(tree),
            _ => None,
        };
        let current_root = self.workspaces.get(&ws_id).and_then(|ws| ws.preferred_root);
        let changed = match (current_root, incoming_tree) {
            (Some(root), Some(tree)) => self.build_layout_node(root) != *tree,
            (None, None) => false,
            _ => true,
        };

        if !changed {
            return;
        }

        tracing::debug!(%ws_id, "PartitionTree preferred layout changed, reloading");

        // Phase: immutable snapshot — collect windows, old root, and focus.
        // Mutable work (detach_child, container deletion) happens below.
        let (tiling_windows, old_root) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            let windows: Vec<WindowId> = state
                .root
                .map(|r| {
                    self.children_dfs(r)
                        .filter_map(|c| match c {
                            Child::Window(id) => Some(id),
                            Child::Container(_) => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            (windows, state.root)
        };

        let focused = self.focused_tiling_window(ws_id);

        // Phase: mutable — detach root (clears bookmarks + occupation,
        // triggers one layout on the now-empty workspace).
        if let Some(root) = old_root {
            self.detach_child(hub, root);
        }

        // Set the new preferred layout.
        let new_root = match incoming {
            LayoutWorkspaceConfig::PartitionTree { tree, .. } => {
                tree.as_ref().map(|t| self.build_preferred_layout(t))
            }
            _ => None,
        };
        self.workspaces.get_mut(&ws_id).unwrap().preferred_root = new_root;
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .occupied_preferred_root = None;

        // Reattach windows under the new layout.
        for &wid in &tiling_windows {
            self.attach_window(hub, wid, ws_id);
        }

        if let Some(f) = focused {
            self.set_focus(hub, Child::Window(f));
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
                    windows: Vec::new(),
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

    fn preferred_slot_of_child(&self, child: Child) -> Option<PreferredSlot> {
        match child {
            Child::Window(wid) => self
                .tiling_windows
                .get(&wid)?
                .occupy
                .map(PreferredSlot::Window),
            Child::Container(cid) => self
                .containers
                .get(cid)
                .occupy
                .map(PreferredSlot::Container),
        }
    }

    fn container_slot_split(&self, slot: PreferredContainerSlotId) -> SplitMode {
        self.container_slots
            .get(slot)
            .split
            .unwrap_or(SplitMode::Horizontal)
    }

    fn occupy_container_slot(&mut self, slot: PreferredContainerSlotId, container_id: ContainerId) {
        self.container_slots.get_mut(slot).occupied = Some(container_id);
        self.containers.get_mut(container_id).occupy = Some(slot);
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
        self.compute_placement(hub, ws_id);
        self.set_focus(hub, Child::Window(window_id));
    }

    fn build_from_live_tree(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<PreferredSlot> {
        let (root, old_root) = {
            let ws = self.workspaces.get(&ws_id)?;
            (ws.root?, ws.preferred_root)
        };

        let mut emitted_matchers: HashSet<WindowMatcher> = HashSet::new();
        let mut pref_root: Option<PreferredSlot> = None;

        let mut stack: Vec<(Option<PreferredContainerSlotId>, Child)> = vec![(None, root)];
        for _ in crate::core::bounded_loop() {
            let Some((parent_cs, child)) = stack.pop() else {
                break;
            };
            match child {
                Child::Window(wid) => {
                    let matcher = {
                        let td = &self.tiling_windows[&wid];
                        if let Some(old) = td.occupy {
                            self.window_slots.get(old).matcher.clone()
                        } else {
                            hub.windows.get(wid).metadata.to_window_matcher()
                        }
                    };
                    if !emitted_matchers.insert(matcher.clone()) {
                        continue;
                    }
                    let slot = self.window_slots.allocate(PreferredWindowSlot {
                        matcher,
                        windows: vec![wid],
                        parent: parent_cs,
                    });
                    if let Some(pid) = parent_cs {
                        self.container_slots
                            .get_mut(pid)
                            .children
                            .push(PreferredSlot::Window(slot));
                    } else if pref_root.is_none() {
                        pref_root = Some(PreferredSlot::Window(slot));
                    }
                    self.tiling_windows.get_mut(&wid).unwrap().occupy = Some(slot);
                }
                Child::Container(cid) => {
                    let children = self.containers.get(cid).children.clone();

                    let split = {
                        let container = self.containers.get(cid);
                        Some(match container.direction() {
                            Some(Direction::Horizontal) => SplitMode::Horizontal,
                            Some(Direction::Vertical) => SplitMode::Vertical,
                            None => SplitMode::Tabbed,
                        })
                    };
                    let cs = self.container_slots.allocate(PreferredContainerSlot {
                        split,
                        children: vec![],
                        occupied: Some(cid),
                        parent: parent_cs,
                    });
                    if let Some(pid) = parent_cs {
                        self.container_slots
                            .get_mut(pid)
                            .children
                            .push(PreferredSlot::Container(cs));
                    } else if pref_root.is_none() {
                        pref_root = Some(PreferredSlot::Container(cs));
                    }
                    self.containers.get_mut(cid).occupy = Some(cs);
                    for &c in children.iter().rev() {
                        stack.push((Some(cs), c));
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

        let pref_root = pref_root?;
        self.workspaces.get_mut(&ws_id).unwrap().preferred_root = Some(pref_root);
        Some(pref_root)
    }

    /// It's acceptable to use recursion here, because if the tree has any circle we would have
    /// panicked in the previous step
    fn build_layout_node(&self, slot: PreferredSlot) -> TreeLayoutNode {
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
    pub(super) windows: Vec<WindowId>,
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
