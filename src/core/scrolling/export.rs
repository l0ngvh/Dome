use crate::core::{
    ColumnConfig, hub::HubAccess, node::WorkspaceId, scrolling::ScrollingStrategy,
    strategy::WorkspaceExport,
};

impl ScrollingStrategy {
    pub(super) fn export_workspace(
        &mut self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
    ) -> WorkspaceExport {
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            panic!("scrolling: export_workspace called for {ws_id} but workspace has no state")
        };
        let previous = std::mem::take(&mut state.slots);
        let mut slots: Vec<ColumnConfig> = Vec::new();
        for column in &mut state.columns {
            let matcher = match column.occupy {
                Some(slot) => previous[slot].children[0].clone(),
                None => {
                    let wid = Self::column_window(hub, column.container);
                    hub.windows.get(wid).metadata.to_window_matcher()
                }
            };
            let slot = match slots.iter().position(|s| s.children[0] == matcher) {
                Some(slot) => slot,
                None => {
                    slots.push(ColumnConfig {
                        width: Some(column.width),
                        children: vec![matcher],
                    });
                    slots.len() - 1
                }
            };
            column.occupy = Some(slot);
        }
        state.slots = slots.clone();
        WorkspaceExport {
            strategy: "scrolling".into(),
            columns: slots,
            ..Default::default()
        }
    }
}
