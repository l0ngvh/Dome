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
            Response::Error { error } => anyhow::bail!("{}", error.message),
        }
    }

    pub fn into_unit(self) -> anyhow::Result<()> {
        match self {
            Response::Ok { .. } => Ok(()),
            Response::Error { error } => anyhow::bail!("{}", error.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{MonitorDetails, MonitorFrame};

    fn monitor(name: &str) -> MonitorDetails {
        MonitorDetails {
            device_name: name.to_string(),
            unique_name: name.to_string(),
            cg_display_id: None,
            gdi_device: None,
            work_area: MonitorFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        }
    }

    #[test]
    fn into_data_decodes_query_payload() {
        let monitors = vec![monitor("A"), monitor("B")];
        let response = Response::ok(serde_json::to_value(&monitors).unwrap());
        let value = response.into_data().unwrap();
        let decoded: Vec<MonitorDetails> = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, monitors);
    }

    #[test]
    fn into_unit_bails_with_error_message() {
        let response = Response::error(ErrorCode::ActionFailed, "no focused window");
        let err = response.into_unit().unwrap_err();
        assert_eq!(err.to_string(), "no focused window");
    }

    #[test]
    fn ok_serializes_with_data() {
        let json = serde_json::to_string(&Response::ok(serde_json::Value::Null)).unwrap();
        assert_eq!(json, r#"{"status":"ok","data":null}"#);
    }

    #[test]
    fn error_serializes_with_code_and_message() {
        let json = serde_json::to_string(&Response::error(
            ErrorCode::QueryTimedOut,
            "query timed out",
        ))
        .unwrap();
        assert_eq!(
            json,
            r#"{"status":"error","error":{"code":"query_timed_out","message":"query timed out"}}"#
        );
    }
}
