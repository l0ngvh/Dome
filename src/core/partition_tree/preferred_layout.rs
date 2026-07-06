use crate::config::{SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::node::{ContainerId, WindowId};

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

    /// Lowest common ancestor of two slots in the preferred layout. Both must
    /// belong to this preferred layout.
    pub(super) fn lowest_common_ancestor(
        &self,
        a: PreferredSlot,
        b: PreferredSlot,
    ) -> PreferredContainerSlotId {
        let ancestors_a = self.slot_parents(a);
        let ancestors_b = self.slot_parents(b);
        for pa in &ancestors_a {
            if ancestors_b.contains(pa) {
                return *pa;
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
