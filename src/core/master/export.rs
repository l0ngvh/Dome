use crate::core::{
    PreferredMaster, WindowMatcher,
    hub::HubAccess,
    master::{MasterStrategy, PaneConfig},
    node::{WindowId, WorkspaceId},
};

impl MasterStrategy {
    pub(in crate::core) fn export_workspace(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> PreferredMaster {
        let Some(state) = self.workspaces.get(&ws_id) else {
            panic!("master: export_workspace called for {ws_id} but workspace has no state")
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
