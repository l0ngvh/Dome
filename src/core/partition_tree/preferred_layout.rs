//! Materializes the preferred layout onto the live tiling tree as windows
//! arrive.
//!
//! A preferred layout is a tree of slots. Each leaf is a tiling slot of the
//! workspace that the layout belongs to. A window that holds a leaf positions
//! later windows only while it tiles on that workspace. A container slot is held
//! once a live container materializes it.

use std::cmp::Ordering;

use crate::config::lua::deserializer::{FromLuaValue, LoadContext, as_table};
use crate::core::WindowMatcher;
use crate::core::allocator::{Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::matcher::WindowMode;
use crate::core::node::{Child, ContainerId, WindowId, WorkspaceId};
use crate::core::partition_tree::PartitionTreeStrategy;
use crate::core::partition_tree::SplitMode;
use crate::core::slot::{Slot, SlotId, held_tiling_slot};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TreeLayoutNode {
    Leaf(WindowMatcher),
    Container {
        split: Option<SplitMode>,
        children: Vec<TreeLayoutNode>,
    },
}

/// A matcher's keys never collide with `split` or `children`.
impl FromLuaValue for TreeLayoutNode {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(
            value,
            "a window matcher, a list of children, or a container",
        )?;
        let has_split = table.contains_key("split")?;
        let has_children = table.contains_key("children")?;
        if has_children {
            return Ok(TreeLayoutNode::Container {
                split: cx.field(table, "split"),
                children: cx.field(table, "children"),
            });
        }
        if has_split {
            return Err(mlua::Error::runtime(
                "a container with split must also have children",
            ));
        }
        if table.raw_len() > 0 {
            return Ok(TreeLayoutNode::Container {
                split: None,
                children: Vec::from_lua_value(value, cx)?,
            });
        }
        Ok(TreeLayoutNode::Leaf(WindowMatcher::from_lua_value(
            value, cx,
        )?))
    }
}

impl PartitionTreeStrategy {
    pub(super) fn build_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        tree: &TreeLayoutNode,
    ) -> PreferredSlot {
        self.build_preferred_layout_subtree(hub, ws_id, tree, None)
    }

    fn first_held_ancestor(&self, slot: SlotId) -> Option<PreferredContainerSlotId> {
        let mut current = self.leaf_parents[&slot];
        for _ in crate::core::bounded_loop() {
            let Some(parent_id) = current else {
                break;
            };
            let cs = self.container_slots.get(parent_id);
            if cs.container.is_some() {
                return Some(parent_id);
            }
            current = cs.parent;
        }
        None
    }

    /// The first slot in a preorder walk of the preferred layout that a live window or
    /// container of the workspace holds. When no window has moved or come back since the
    /// layout placed it, every other such slot is below this one.
    pub(super) fn held_preferred_root(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<PreferredSlot> {
        let mut stack: Vec<PreferredSlot> = self
            .workspaces
            .get(&ws_id)?
            .preferred_root
            .into_iter()
            .collect();
        for _ in crate::core::bounded_loop() {
            let slot = stack.pop()?;
            if self.child_of_preferred_slot(hub, slot).is_some() {
                return Some(slot);
            }
            if let PreferredSlot::Container(id) = slot {
                stack.extend(self.container_slots.get(id).children.iter().rev().copied());
            }
        }
        None
    }

    /// The live window or container that holds `slot`, or `None` when the slot is free or its
    /// window does not tile on the slot's workspace.
    fn child_of_preferred_slot(&self, hub: &HubAccess, slot: PreferredSlot) -> Option<Child> {
        match slot {
            PreferredSlot::Window(id) => {
                let leaf = hub.slots.get(id);
                let window_id = leaf.window?;
                let tiles_on_leaf_workspace = self.tiling_windows.contains_key(&window_id)
                    && hub.windows.get(window_id).workspace() == Some(leaf.workspace);
                tiles_on_leaf_workspace.then_some(Child::Window(window_id))
            }
            PreferredSlot::Container(id) => {
                self.container_slots.get(id).container.map(Child::Container)
            }
        }
    }

    /// The live tree grows toward the preferred layout one matched window at a time. A new
    /// window joins the part of the layout that other windows already hold, at the place the
    /// layout gives `slot_id`:
    ///
    /// - When a held container slot contains `slot_id`, the window joins that container, or a
    ///   new sub-container inside it, in layout order.
    /// - When the held part sits in another subtree, the held part and the window become the
    ///   two children of a new container, at the lowest common ancestor of `slot_id` and the
    ///   held part.
    /// - When no slot in the workspace is held, the spawn direction places the window.
    pub(super) fn attach_window_to_slot(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: SlotId,
    ) {
        hub.windows.get_mut(window_id).set_workspace(Some(ws_id));
        self.workspaces
            .get_mut(&ws_id)
            .unwrap()
            .add_to_history(window_id);
        if let Some(ancestor_slot) = self.first_held_ancestor(slot_id) {
            self.attach_window_into_held_ancestor(hub, window_id, ws_id, slot_id, ancestor_slot);
        } else if let Some(root_slot) = self.held_preferred_root(hub, ws_id) {
            self.attach_window_to_free_ancestor(hub, window_id, ws_id, slot_id, root_slot);
        } else {
            self.attach_child_according_to_spawn_direction(hub, Child::Window(window_id), ws_id);
        }
    }

    /// `ordering` tells whether the new window's slot comes before or after the
    /// anchor's slot among the children of `lca`.
    fn materialize_container_slot(
        &mut self,
        hub: &mut HubAccess,
        lca: PreferredContainerSlotId,
        anchor: Child,
        window_id: WindowId,
        ordering: Ordering,
    ) {
        let window = Child::Window(window_id);
        let children = if ordering == Ordering::Less {
            vec![window, anchor]
        } else {
            vec![anchor, window]
        };
        let split = self.container_slot_split(lca);
        let container_id = self.replace_anchor_with_container(hub, anchor, children, split);
        self.hold_container_slot(lca, container_id);
    }

    /// Called when the lowest common ancestor of the inserted window and the held preferred
    /// root is not yet a live container.
    fn attach_window_to_free_ancestor(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: SlotId,
        root_slot: PreferredSlot,
    ) {
        tracing::debug!(%window_id, ?slot_id, ?root_slot, "Joining window to existing preferred root");
        let (lca, ordering) =
            self.lowest_common_ancestor(PreferredSlot::Window(slot_id), root_slot);
        let anchor = self
            .child_of_preferred_slot(hub, root_slot)
            .expect("the held preferred root is held");
        self.materialize_container_slot(hub, lca, anchor, window_id, ordering);
        self.compute_placement(hub, ws_id);
    }

    fn attach_window_into_held_ancestor(
        &mut self,
        hub: &mut HubAccess,
        window_id: WindowId,
        ws_id: WorkspaceId,
        slot_id: SlotId,
        ancestor_slot: PreferredContainerSlotId,
    ) {
        let container_id = self.container_slots.get(ancestor_slot).container.unwrap();
        let live_children = hub.containers.get(container_id).children.clone();

        let mut insert_pos = 0;

        for (i, &child) in live_children.iter().enumerate() {
            let Some(child_slot) = self.preferred_slot_of_child(hub, ws_id, child) else {
                continue;
            };
            let (lca, ordering) =
                self.lowest_common_ancestor(PreferredSlot::Window(slot_id), child_slot);

            if self.is_proper_descendant_of(lca, ancestor_slot) {
                self.materialize_container_slot(hub, lca, child, window_id, ordering);
                self.compute_placement(hub, ws_id);
                return;
            }

            if ordering == Ordering::Less {
                insert_pos = i;
                break;
            }
            insert_pos = i + 1;
        }

        tracing::debug!(%window_id, ?slot_id, %container_id, insert_pos, "Inserting window into held ancestor container");
        self.attach_child_to_container(
            hub,
            Child::Window(window_id),
            container_id,
            Some(insert_pos),
        );

        self.compute_placement(hub, ws_id);
    }

    pub(super) fn release_container_slots_in(&mut self, hub: &HubAccess, subtree: Child) {
        for node in hub.children_dfs(subtree) {
            if let Child::Container(cid) = node {
                self.release_container_slot(cid);
            }
        }
    }

    pub(super) fn free_preferred_subtree(&mut self, root: PreferredSlot) {
        let mut stack = vec![root];
        for _ in crate::core::bounded_loop() {
            let Some(slot) = stack.pop() else { break };
            match slot {
                PreferredSlot::Window(id) => {
                    self.leaf_parents.remove(&id);
                }
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

    pub(super) fn release_container_slot(&mut self, container_id: ContainerId) {
        let data = self.tiling_containers.get_mut(&container_id).unwrap();
        let Some(slot) = data.held_slot.take() else {
            return;
        };
        self.container_slots.get_mut(slot).container = None;
    }

    pub(super) fn export_tree(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<TreeLayoutNode> {
        self.workspaces
            .get(&ws_id)
            .and_then(|ws| ws.root)
            .map(|root| self.live_layout_node(hub, root))
    }

    fn live_layout_node(&self, hub: &HubAccess, root: Child) -> TreeLayoutNode {
        enum Step {
            Visit(Child),
            Close { split: SplitMode, len: usize },
        }
        let mut steps = vec![Step::Visit(root)];
        let mut built: Vec<TreeLayoutNode> = Vec::new();
        for _ in crate::core::bounded_loop() {
            let Some(step) = steps.pop() else { break };
            match step {
                Step::Visit(Child::Window(wid)) => {
                    let matcher = hub.windows.get(wid).metadata.to_window_matcher();
                    built.push(TreeLayoutNode::Leaf(matcher));
                }
                Step::Visit(Child::Container(cid)) => {
                    let split = self.tiling_containers[&cid]
                        .direction()
                        .map_or(SplitMode::Tabbed, SplitMode::from);
                    let children = &hub.containers.get(cid).children;
                    steps.push(Step::Close {
                        split,
                        len: children.len(),
                    });
                    steps.extend(children.iter().rev().map(|&c| Step::Visit(c)));
                }
                Step::Close { split, len } => {
                    let children = built.split_off(built.len() - len);
                    built.push(TreeLayoutNode::Container {
                        split: Some(split),
                        children,
                    });
                }
            }
        }
        built.pop().expect("the walk builds one root")
    }

    fn build_preferred_layout_subtree(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        node: &TreeLayoutNode,
        parent: Option<PreferredContainerSlotId>,
    ) -> PreferredSlot {
        match node {
            TreeLayoutNode::Leaf(matcher) => {
                let id = hub
                    .slots
                    .allocate(Slot::new(matcher.clone(), ws_id, WindowMode::Tiling));
                self.leaf_parents.insert(id, parent);
                PreferredSlot::Window(id)
            }
            TreeLayoutNode::Container { split, children } => {
                let mut child_slots = Vec::with_capacity(children.len());
                let id = self.container_slots.allocate(PreferredContainerSlot {
                    split: *split,
                    children: Vec::new(),
                    container: None,
                    parent,
                });
                for c in children {
                    let child_slot = self.build_preferred_layout_subtree(hub, ws_id, c, Some(id));
                    child_slots.push(child_slot);
                }
                self.container_slots.get_mut(id).children = child_slots;
                PreferredSlot::Container(id)
            }
        }
    }

    fn preferred_slot_of_child(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        child: Child,
    ) -> Option<PreferredSlot> {
        match child {
            Child::Window(wid) => {
                held_tiling_slot(&hub.slots, wid, ws_id).map(PreferredSlot::Window)
            }
            Child::Container(cid) => self
                .tiling_containers
                .get(&cid)
                .unwrap()
                .held_slot
                .map(PreferredSlot::Container),
        }
    }

    fn container_slot_split(&self, slot: PreferredContainerSlotId) -> SplitMode {
        self.container_slots
            .get(slot)
            .split
            .unwrap_or(SplitMode::Horizontal)
    }

    fn hold_container_slot(&mut self, slot: PreferredContainerSlotId, container_id: ContainerId) {
        self.container_slots.get_mut(slot).container = Some(container_id);
        self.tiling_containers
            .get_mut(&container_id)
            .unwrap()
            .held_slot = Some(slot);
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
            PreferredSlot::Window(id) => self.leaf_parents[&id],
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

/// A container slot in the preferred layout tree.
#[derive(Debug, Clone)]
pub(super) struct PreferredContainerSlot {
    split: Option<SplitMode>,
    pub(super) children: Vec<PreferredSlot>,
    container: Option<ContainerId>,
    parent: Option<PreferredContainerSlotId>,
}

impl Node for PreferredContainerSlot {
    type Id = PreferredContainerSlotId;
}

/// Reference to a child slot within the preferred layout tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreferredSlot {
    Window(SlotId),
    Container(PreferredContainerSlotId),
}
