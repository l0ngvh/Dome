use rustc_hash::FxHashMap;

use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::matcher::{WindowMatcher, WindowMode};
use crate::core::node::{WindowId, WindowMetadata, WorkspaceId};

/// One window matcher from a workspace entry in `layout.lua`.
#[derive(Debug, Clone)]
pub(super) struct Slot {
    pub(super) matcher: WindowMatcher,
    /// The workspace whose layout entry declares the slot.
    pub(super) workspace: WorkspaceId,
    pub(super) mode: WindowMode,
    window: Option<WindowId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SlotId(usize);

impl NodeId for SlotId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl Node for Slot {
    type Id = SlotId;
}

impl Slot {
    pub(super) fn window(&self) -> Option<WindowId> {
        self.window
    }
}

pub(super) struct Slots {
    allocator: Allocator<Slot>,
    held_slots: FxHashMap<WindowId, SlotId>,
}

impl Slots {
    pub(super) fn new() -> Self {
        Self {
            allocator: Allocator::new(),
            held_slots: FxHashMap::default(),
        }
    }

    pub(super) fn allocate(
        &mut self,
        matcher: WindowMatcher,
        workspace: WorkspaceId,
        mode: WindowMode,
    ) -> SlotId {
        self.allocator.allocate(Slot {
            matcher,
            workspace,
            mode,
            window: None,
        })
    }

    pub(super) fn clear(&mut self) {
        self.allocator.clear();
        self.held_slots.clear();
    }

    pub(super) fn get(&self, id: SlotId) -> &Slot {
        self.allocator.get(id)
    }

    /// Any free slot matching the window, across every workspace and mode.
    pub(super) fn find_free_slot(&self, metadata: &dyn WindowMetadata) -> Option<SlotId> {
        self.allocator
            .find(|slot| slot.window.is_none() && metadata.matches_window_matcher(&slot.matcher))
    }

    /// The slot held by this window, across every workspace and mode.
    pub(super) fn held_slot(&self, window: WindowId) -> Option<SlotId> {
        self.held_slots.get(&window).copied()
    }

    /// Panics when the slot is already held.
    pub(super) fn hold(&mut self, id: SlotId, window: WindowId) {
        let slot = self.allocator.get_mut(id);
        assert!(
            slot.window.is_none(),
            "slot already held by {:?}, cannot hold it for {window}",
            slot.window
        );
        slot.window = Some(window);
        self.held_slots.insert(window, id);
    }

    /// Releases the window's slot on whichever workspace declares it.
    pub(super) fn release(&mut self, window: WindowId) {
        if let Some(id) = self.held_slot(window) {
            self.allocator.get_mut(id).window = None;
            self.held_slots.remove(&window);
        }
    }
}
