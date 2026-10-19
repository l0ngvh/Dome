use dome_ipc::action::{MonitorDetails, MonitorFrame};
use dome_ipc::response::*;

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
    let monitors = vec![
        MonitorDetails {
            cg_display_id: Some(42),
            ..monitor("A")
        },
        MonitorDetails {
            gdi_device: Some(r"\\.\DISPLAY2".into()),
            ..monitor("B")
        },
        monitor("C"),
    ];
    let expected = serde_json::json!([
        {"device_name":"A", "unique_name":"A", "cg_display_id":42, "gdi_device":null,
         "work_area":{"x":0, "y":0, "width":1920, "height":1080}},
        {"device_name":"B", "unique_name":"B", "cg_display_id":null, "gdi_device":r"\\.\DISPLAY2",
         "work_area":{"x":0, "y":0, "width":1920, "height":1080}},
        {"device_name":"C", "unique_name":"C", "cg_display_id":null, "gdi_device":null,
         "work_area":{"x":0, "y":0, "width":1920, "height":1080}}
    ]);
    assert_eq!(serde_json::to_value(&monitors).unwrap(), expected);
    let response = Response::ok(expected);
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
fn into_unit_error_preserves_code_for_downcast() {
    let response = Response::error(ErrorCode::QueryTimedOut, "query timed out");
    let err = response.into_unit().unwrap_err();
    let rpc = err.downcast_ref::<RpcError>().unwrap();
    assert_eq!(rpc.code, ErrorCode::QueryTimedOut);
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
