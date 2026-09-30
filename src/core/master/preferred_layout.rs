use crate::config::lua::deserializer::{
    FromLuaValue, LoadContext, Shape, as_table, warn_if_shape_mismatched,
};
use crate::core::WindowMatcher;
use crate::core::allocator::{Node, NodeId};
use crate::core::hub::HubAccess;
use crate::core::master::{MasterStrategy, PaneDisplay, PaneKind};
use crate::core::node::{Child, WindowId, WindowMetadata, WorkspaceId};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PaneConfig {
    pub(crate) display: PaneDisplay,
    pub(crate) children: Vec<WindowMatcher>,
}

impl PaneConfig {
    #[cfg(test)]
    pub(crate) fn tiled(children: Vec<WindowMatcher>) -> Self {
        Self {
            display: PaneDisplay::Tiled,
            children,
        }
    }
}

/// Accepts either a bare list of matchers or a table carrying `display` and
/// `children`.
impl FromLuaValue for PaneConfig {
    fn from_lua_value(value: &mlua::Value, cx: &mut LoadContext) -> mlua::Result<Self> {
        let table = as_table(value, "a list of window matchers, or a pane table")?;
        let keyed = table.contains_key("display")? || table.contains_key("children")?;
        if !keyed {
            return Ok(PaneConfig {
                display: PaneDisplay::Tiled,
                children: Vec::from_lua_value(value, cx)?,
            });
        }
        warn_if_shape_mismatched(table, Shape::Map, cx);
        Ok(PaneConfig {
            display: cx.field(table, "display"),
            children: cx.field(table, "children"),
        })
    }
}

impl MasterStrategy {
    pub(super) fn sort_window_into_pane(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) {
        // Resolve the matching slot before touching a container, because the match borrows the
        // window metadata out of `hub` and the mutations below borrow `hub` exclusively.
        let (effective_count, master, secondary, master_match, secondary_match) = {
            let state = self.workspaces.get(&ws_id).unwrap();
            let metadata = hub.windows.get(window_id).metadata.as_ref();
            (
                state.master_count.unwrap_or(self.master_count),
                state.master.container,
                state.secondary.container,
                self.find_free_slot(&state.master.slots, metadata),
                self.find_free_slot(&state.secondary.slots, metadata),
            )
        };

        if let Some(slot) = master_match {
            if Self::pane_len(hub, master) < effective_count {
                self.hold_slot(slot, window_id);
                self.insert_in_slot_order(hub, ws_id, PaneKind::Master, window_id, slot);
                return;
            }
            // Master is full. Evict an unmatched window if one exists, otherwise let this
            // window fall through to the secondary stack.
            let evict = hub.containers.get(master).children().iter().rposition(|c| {
                matches!(c, Child::Window(w)
                    if self.window_states.get(w).is_some_and(|e| e.held_slot.is_none()))
            });
            if let Some(evict_pos) = evict {
                let evicted = Self::remove_from_pane(hub, master, evict_pos);
                Self::insert_into_pane(hub, secondary, 0, evicted);
                self.hold_slot(slot, window_id);
                self.insert_in_slot_order(hub, ws_id, PaneKind::Master, window_id, slot);
                return;
            }
        }

        if let Some(slot) = secondary_match {
            self.hold_slot(slot, window_id);
            self.insert_in_slot_order(hub, ws_id, PaneKind::Secondary, window_id, slot);
            return;
        }

        self.push_unmatched_window(hub, ws_id, window_id);
    }

    pub(super) fn find_free_slot(
        &self,
        slots: &[SlotId],
        metadata: &dyn WindowMetadata,
    ) -> Option<SlotId> {
        slots
            .iter()
            .copied()
            .find(|&id| self.slots.get(id).is_free_for(metadata))
    }

    /// The window must already have its `WindowState`.
    fn hold_slot(&mut self, slot: SlotId, window_id: WindowId) {
        self.slots.get_mut(slot).window = Some(window_id);
        self.window_states.get_mut(&window_id).unwrap().held_slot = Some(slot);
    }

    /// Must run before `window_states` drops the window. After that, nothing can find the
    /// slot, so it stays held.
    pub(super) fn release_slot(&mut self, window_id: WindowId) {
        let held_slot = self
            .window_states
            .get_mut(&window_id)
            .and_then(|entry| entry.held_slot.take());
        if let Some(slot_id) = held_slot {
            self.slots.get_mut(slot_id).window = None;
        }
    }

    fn insert_in_slot_order(
        &self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        kind: PaneKind,
        window_id: WindowId,
        slot: SlotId,
    ) {
        let pane = self.workspaces.get(&ws_id).unwrap().pane(kind);
        let slot_position = pane.slots.iter().position(|&x| x == slot).unwrap();
        // Insert in preferred-layout order. Moved windows break that order, so placing the
        // window right before the first later slot is acceptable.
        let insert_position = hub
            .containers
            .get(pane.container)
            .children()
            .iter()
            .position(|c| {
                let Child::Window(w) = c else {
                    return false;
                };
                let Some(held) = self.window_states.get(w).unwrap().held_slot else {
                    return false;
                };
                pane.slots
                    .iter()
                    .position(|&s| s == held)
                    .is_some_and(|s| s > slot_position)
            })
            .unwrap_or_else(|| hub.containers.get(pane.container).children().len());
        hub.containers
            .get_mut(pane.container)
            .children
            .insert(insert_position, Child::Window(window_id));
    }
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

#[derive(Debug, Clone)]
pub(super) struct Slot {
    pub(super) matcher: WindowMatcher,
    pub(super) window: Option<WindowId>,
}

impl Node for Slot {
    type Id = SlotId;
}

impl Slot {
    pub(super) fn is_free_for(&self, metadata: &dyn WindowMetadata) -> bool {
        self.window.is_none() && metadata.matches_window_matcher(&self.matcher)
    }
}
