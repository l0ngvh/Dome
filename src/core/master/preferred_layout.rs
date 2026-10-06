use crate::config::lua::deserializer::{
    FromLuaValue, LoadContext, Shape, as_table, warn_if_shape_mismatched,
};
use crate::core::hub::HubAccess;
use crate::core::master::{MasterStrategy, PaneDisplay, PaneKind};
use crate::core::matcher::WindowMode;
use crate::core::node::{Child, WindowId, WorkspaceId};
use crate::core::slot::{Slot, SlotId, held_tiling_slot};
use crate::core::{PreferredMaster, WindowMatcher};

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
        slot: Option<SlotId>,
    ) {
        let Some(slot) = slot else {
            self.push_unmatched_window(hub, ws_id, window_id);
            return;
        };
        let state = self.workspaces.get(&ws_id).unwrap();
        if state.secondary.slots.contains(&slot) {
            self.insert_in_slot_order(hub, ws_id, PaneKind::Secondary, window_id, slot);
            return;
        }
        assert!(
            state.master.slots.contains(&slot),
            "{slot:?} is a slot of neither pane of master workspace {ws_id}"
        );
        let effective_count = state.master_count.unwrap_or(self.master_count);
        let (master, secondary) = (state.master.container, state.secondary.container);
        if Self::pane_len(hub, master) >= effective_count {
            let evict = hub.containers.get(master).children().iter().rposition(|c| {
                matches!(c, Child::Window(w) if held_tiling_slot(&hub.slots, *w, ws_id).is_none())
            });
            match evict {
                Some(evict_pos) => {
                    let evicted = Self::remove_from_pane(hub, master, evict_pos);
                    Self::insert_into_pane(hub, secondary, 0, evicted);
                }
                None => {
                    self.workspaces.get_mut(&ws_id).unwrap().master_count =
                        Some(effective_count + 1);
                }
            }
        }
        self.insert_in_slot_order(hub, ws_id, PaneKind::Master, window_id, slot);
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
                let Some(held) = held_tiling_slot(&hub.slots, *w, ws_id) else {
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

    pub(super) fn allocate_slots(
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        matchers: &[WindowMatcher],
    ) -> Vec<SlotId> {
        matchers
            .iter()
            .map(|m| {
                hub.slots
                    .allocate(Slot::new(m.clone(), ws_id, WindowMode::Tiling))
            })
            .collect()
    }

    pub(super) fn export_panes(&self, hub: &HubAccess, ws_id: WorkspaceId) -> PreferredMaster {
        let Some(state) = self.workspaces.get(&ws_id) else {
            panic!("master: export_panes called for {ws_id} but workspace has no state")
        };
        let master = Self::pane_windows(hub, state.master.container);
        let secondary = Self::pane_windows(hub, state.secondary.container);
        PreferredMaster {
            master_ratio: state.master_ratio,
            master_count: state.master_count,
            master: PaneConfig {
                display: state.master.display,
                children: Self::export_pane(hub, &master),
            },
            secondary: PaneConfig {
                display: state.secondary.display,
                children: Self::export_pane(hub, &secondary),
            },
        }
    }

    fn export_pane(hub: &HubAccess, pane: &[WindowId]) -> Vec<WindowMatcher> {
        pane.iter()
            .map(|&wid| hub.windows.get(wid).metadata.to_window_matcher())
            .collect()
    }
}
