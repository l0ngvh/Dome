use crate::{
    config::WindowMatcher,
    core::{hub::HubAccess, master::MasterStrategy, node::WorkspaceId, strategy::WorkspaceExport},
};

impl MasterStrategy {
    pub(super) fn do_export_workspace(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> Option<WorkspaceExport> {
        let state = self.workspaces.get(&ws_id)?;

        let master: Vec<WindowMatcher> = state
            .master
            .iter()
            .map(
                |&wid| match self.window_states.get(&wid).and_then(|e| e.occupy) {
                    Some(mid) => self.matchers.get(mid).clone(),
                    None => hub.windows.get(wid).metadata.to_window_matcher(),
                },
            )
            .collect();
        let secondary: Vec<WindowMatcher> = state
            .secondary
            .iter()
            .map(
                |&wid| match self.window_states.get(&wid).and_then(|e| e.occupy) {
                    Some(mid) => self.matchers.get(mid).clone(),
                    None => hub.windows.get(wid).metadata.to_window_matcher(),
                },
            )
            .collect();

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let old_master_ids = state.master_matchers.clone();
        let old_secondary_ids = state.secondary_matchers.clone();

        for &id in &state.master_matchers {
            self.matchers.delete(id);
        }
        for &id in &state.secondary_matchers {
            self.matchers.delete(id);
        }

        state.master_matchers = master
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();
        state.secondary_matchers = secondary
            .iter()
            .map(|m| self.matchers.allocate(m.clone()))
            .collect();

        for &wid in &state.master {
            let new_occupy = self
                .window_states
                .get(&wid)
                .and_then(|e| e.occupy)
                .and_then(|old_id| {
                    old_master_ids
                        .iter()
                        .position(|&x| x == old_id)
                        .and_then(|slot| state.master_matchers.get(slot).copied())
                        .or_else(|| {
                            old_secondary_ids
                                .iter()
                                .position(|&x| x == old_id)
                                .and_then(|slot| state.secondary_matchers.get(slot).copied())
                        })
                });
            if let Some(entry) = self.window_states.get_mut(&wid) {
                entry.occupy = new_occupy;
            }
        }
        for &wid in &state.secondary {
            let new_occupy = self
                .window_states
                .get(&wid)
                .and_then(|e| e.occupy)
                .and_then(|old_id| {
                    old_master_ids
                        .iter()
                        .position(|&x| x == old_id)
                        .and_then(|slot| state.master_matchers.get(slot).copied())
                        .or_else(|| {
                            old_secondary_ids
                                .iter()
                                .position(|&x| x == old_id)
                                .and_then(|slot| state.secondary_matchers.get(slot).copied())
                        })
                });
            if let Some(entry) = self.window_states.get_mut(&wid) {
                entry.occupy = new_occupy;
            }
        }

        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: state.master_ratio,
            master_count: state.master_count,
            master,
            secondary,
            ..Default::default()
        })
    }
}
