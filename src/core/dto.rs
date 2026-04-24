use serde::{Deserialize, Serialize};

/// Serializable workspace metadata for IPC queries. External tools (status bars,
/// scripts) consume this as JSON over IPC -- the JSON field names are the
/// stability contract, not this Rust type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct WorkspaceInfo {
    pub name: String,
    pub is_focused: bool,
    pub is_visible: bool,
    pub window_count: usize,
}
