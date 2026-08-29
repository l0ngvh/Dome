use serde::{Deserialize, Serialize};

use crate::action::WorkspaceInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerEvent {
    WorkspacesChanged { workspaces: Vec<WorkspaceInfo> },
}
