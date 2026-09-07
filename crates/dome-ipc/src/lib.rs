pub mod action;
pub mod event;
pub mod response;
pub mod socket;

mod client;

pub use action::{
    Action, FocusTarget, IpcMessage, MasterTarget, MinimizedWindow, MonitorDetails, MonitorTarget,
    MoveTarget, Query, TabDirection, ToggleTarget, WindowId, WorkspaceInfo,
};
pub use client::DomeClient;
pub use event::ServerEvent;
pub use response::{ErrorCode, Response, RpcError};
pub use socket::{socket_name, socket_path};
