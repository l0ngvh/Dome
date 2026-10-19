use serde::{Deserialize, Serialize};

/// The envelope every RPC reply travels in. `data` carries a query payload, or
/// `null` for an action that returns nothing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { data: serde_json::Value },
    Error { error: RpcError },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcError {
    pub code: ErrorCode,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidMessage,
    ActionFailed,
    QueryTimedOut,
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Response::Ok { data }
    }

    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Response::Error {
            error: RpcError {
                code,
                message: message.into(),
            },
        }
    }

    pub fn into_data(self) -> anyhow::Result<serde_json::Value> {
        match self {
            Response::Ok { data } => Ok(data),
            Response::Error { error } => Err(error.into()),
        }
    }

    pub fn into_unit(self) -> anyhow::Result<()> {
        match self {
            Response::Ok { .. } => Ok(()),
            Response::Error { error } => Err(error.into()),
        }
    }
}
