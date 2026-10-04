use crate::core::allocator::{Allocator, Node, NodeId};
use crate::core::hub::Hub;
use crate::core::matcher::{WindowMatcher, WindowMode};
use crate::core::node::{WindowId, WindowMetadata, WorkspaceId};

/// One window matcher from a workspace entry in `layout.lua`.
#[derive(Debug, Clone)]
pub(super) struct Slot {
    pub(super) matcher: WindowMatcher,
    /// The workspace whose layout entry declares the slot.
    pub(super) workspace: WorkspaceId,
    pub(super) mode: WindowMode,
    pub(super) window: Option<WindowId>,
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
    pub(super) fn new(matcher: WindowMatcher, workspace: WorkspaceId, mode: WindowMode) -> Self {
        Self {
            matcher,
            workspace,
            mode,
            window: None,
        }
    }

    /// Panics when the slot is already held.
    pub(super) fn hold(&mut self, window: WindowId) {
        assert!(
            self.window.is_none(),
            "slot already held by {:?}, cannot hold it for {window}",
            self.window
        );
        self.window = Some(window);
    }

    pub(super) fn release(&mut self) {
        self.window = None;
    }

    pub(super) fn is_tiling_on(&self, ws_id: WorkspaceId) -> bool {
        self.workspace == ws_id && matches!(self.mode, WindowMode::Tiling)
    }
}

/// The first free matching slot in arena id order.
pub(super) fn find_free_slot(
    slots: &Allocator<Slot>,
    metadata: &dyn WindowMetadata,
    filter: Option<&dyn Fn(&Slot) -> bool>,
) -> Option<SlotId> {
    slots.sorted_ids().into_iter().find(|&id| {
        let slot = slots.get(id);
        slot.window.is_none()
            && filter.is_none_or(|f| f(slot))
            && metadata.matches_window_matcher(&slot.matcher)
    })
}

/// The slot held by this window, across every workspace and mode.
pub(super) fn held_slot(slots: &Allocator<Slot>, window: WindowId) -> Option<SlotId> {
    slots.find(|slot| slot.window == Some(window))
}

/// The tiling slot of `ws_id` that the window holds.
pub(super) fn held_tiling_slot(
    slots: &Allocator<Slot>,
    window: WindowId,
    ws_id: WorkspaceId,
) -> Option<SlotId> {
    held_slot(slots, window).filter(|&id| slots.get(id).is_tiling_on(ws_id))
}

/// The tiling slots of each workspace in arena id order.
#[cfg(test)]
pub(super) fn tiling_slots_by_workspace(
    slots: &Allocator<Slot>,
) -> rustc_hash::FxHashMap<WorkspaceId, Vec<SlotId>> {
    let mut by_workspace: rustc_hash::FxHashMap<WorkspaceId, Vec<SlotId>> =
        rustc_hash::FxHashMap::default();
    for id in slots.sorted_ids() {
        let slot = slots.get(id);
        if matches!(slot.mode, WindowMode::Tiling) {
            by_workspace.entry(slot.workspace).or_default().push(id);
        }
    }
    by_workspace
}

impl Hub {
    /// Deletes every slot of the workspace, in all modes.
    pub(super) fn remove_slots(&mut self, ws_id: WorkspaceId) {
        self.access.slots.retain(|slot| slot.workspace != ws_id);
    }

    /// Releases the window's slot on whichever workspace declares it.
    pub(super) fn release_slot(&mut self, window_id: WindowId) {
        if let Some(id) = held_slot(&self.access.slots, window_id) {
            self.access.slots.get_mut(id).release();
        }
    }

    #[cfg(test)]
    pub(super) fn validate_slots(&self) {
        let rank = |mode: WindowMode| match mode {
            WindowMode::Fullscreen => 0,
            WindowMode::Float => 1,
            WindowMode::Tiling => 2,
        };
        let mut holders = rustc_hash::FxHashSet::default();
        let mut last_mode = rustc_hash::FxHashMap::default();
        for id in self.access.slots.sorted_ids() {
            let slot = self.access.slots.get(id);
            assert!(
                self.access.workspaces.contains(slot.workspace),
                "{id:?} belongs to {}, which does not exist",
                slot.workspace
            );
            if let Some(window_id) = slot.window {
                assert!(
                    self.access.windows.contains(window_id),
                    "{id:?} holds window {window_id}, which is closed"
                );
                assert!(
                    holders.insert(window_id),
                    "window {window_id} holds more than one slot"
                );
            }
            if let Some(previous) = last_mode.insert(slot.workspace, slot.mode) {
                assert!(
                    rank(previous) <= rank(slot.mode),
                    "{}: {:?} slot {id:?} comes after a {previous:?} slot",
                    slot.workspace,
                    slot.mode
                );
            }
        }
    }
}
