use std::collections::HashMap;

use crate::{
    config::{PaneConfig, WindowMatcher},
    core::{
        hub::HubAccess,
        master::{MasterStrategy, preferred_layout::Slot, preferred_layout::SlotId},
        node::{WindowId, WorkspaceId},
        strategy::WorkspaceExport,
    },
};

impl MasterStrategy {
    pub(super) fn export_workspace(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> WorkspaceExport {
        let Some(state) = self.workspaces.get(&ws_id) else {
            panic!("master: export_workspace called for {ws_id} but workspace has no state")
        };

        let master_ids = Self::pane_windows(hub, state.master.container);
        let secondary_ids = Self::pane_windows(hub, state.secondary.container);
        let master_groups = self.group_pane(hub, &master_ids);
        let secondary_groups = self.group_pane(hub, &secondary_ids);

        let master: Vec<WindowMatcher> = master_groups.iter().map(|g| g.0.clone()).collect();
        let secondary: Vec<WindowMatcher> = secondary_groups.iter().map(|g| g.0.clone()).collect();

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        for &id in &state.master.matchers {
            self.slots.delete(id);
        }
        for &id in &state.secondary.matchers {
            self.slots.delete(id);
        }

        let mut master_slots = Vec::with_capacity(master_groups.len());
        for (matcher, matched, windows) in &master_groups {
            let sid = self.slots.allocate(Slot {
                matcher: matcher.clone(),
                windows: windows.clone(),
            });
            master_slots.push(sid);
            for &wid in windows {
                if let Some(entry) = self.window_states.get_mut(&wid) {
                    entry.occupy = matched.then_some(sid);
                }
            }
        }
        let mut secondary_slots = Vec::with_capacity(secondary_groups.len());
        for (matcher, matched, windows) in &secondary_groups {
            let sid = self.slots.allocate(Slot {
                matcher: matcher.clone(),
                windows: windows.clone(),
            });
            secondary_slots.push(sid);
            for &wid in windows {
                if let Some(entry) = self.window_states.get_mut(&wid) {
                    entry.occupy = matched.then_some(sid);
                }
            }
        }

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master.matchers = master_slots;
        state.secondary.matchers = secondary_slots;

        WorkspaceExport {
            strategy: "master".into(),
            master_ratio: state.master_ratio,
            master_count: state.master_count,
            master: PaneConfig {
                display: state.master.display,
                children: master,
            },
            secondary: PaneConfig {
                display: state.secondary.display,
                children: secondary,
            },
            ..Default::default()
        }
    }

    fn group_pane(
        &self,
        hub: &HubAccess,
        pane: &[WindowId],
    ) -> Vec<(WindowMatcher, bool, Vec<WindowId>)> {
        let mut groups: Vec<(WindowMatcher, bool, Vec<WindowId>)> = Vec::new();
        let mut slot_index: HashMap<SlotId, usize> = HashMap::new();
        for &wid in pane {
            match self.window_states.get(&wid).and_then(|e| e.occupy) {
                Some(sid) => {
                    if let Some(&i) = slot_index.get(&sid) {
                        groups[i].2.push(wid);
                    } else {
                        slot_index.insert(sid, groups.len());
                        groups.push((self.slots.get(sid).matcher.clone(), true, vec![wid]));
                    }
                }
                None => {
                    let matcher = hub.windows.get(wid).metadata.to_window_matcher();
                    groups.push((matcher, false, vec![wid]));
                }
            }
        }
        groups
    }
}
