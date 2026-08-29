pub mod action;
pub mod event;
pub mod socket;

mod client;

pub use action::{
    Action, FocusTarget, IpcMessage, MasterTarget, MonitorTarget, MoveTarget, Query, TabDirection,
    ToggleTarget, WindowId, WorkspaceInfo,
};
pub use client::DomeClient;
pub use event::ServerEvent;
pub use socket::{socket_name, socket_path};
