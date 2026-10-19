use dome_ipc::action::*;
use std::str::FromStr;

#[test]
fn serde_wire_format() {
    let cases = vec![
        (
            Action::Focus {
                target: FocusTarget::Up,
            },
            r#"{"type":"focus","target":"up"}"#,
        ),
        (
            Action::Move {
                target: MoveTarget::Workspace {
                    name: "1".into(),
                    monitor: None,
                },
            },
            r#"{"type":"move","target":{"workspace":{"name":"1","monitor":null}}}"#,
        ),
        (
            Action::Toggle {
                target: ToggleTarget::Float,
            },
            r#"{"type":"toggle","target":"float"}"#,
        ),
        (
            Action::Master {
                target: MasterTarget::More,
            },
            r#"{"type":"master","target":"more"}"#,
        ),
        (Action::Grow, r#"{"type":"grow"}"#),
        (Action::Shrink, r#"{"type":"shrink"}"#),
        (
            Action::Execute {
                command: "open -a Terminal".into(),
            },
            r#"{"type":"execute","command":"open -a Terminal"}"#,
        ),
        (Action::Exit, r#"{"type":"exit"}"#),
        (Action::Close, r#"{"type":"close"}"#),
        (
            Action::UnminimizeWindow {
                id: serde_json::from_value(serde_json::json!(7)).unwrap(),
            },
            r#"{"type":"unminimize_window","id":7}"#,
        ),
        (
            Action::Mode {
                name: "resize".into(),
            },
            r#"{"type":"mode","name":"resize"}"#,
        ),
        (
            Action::Focus {
                target: FocusTarget::Tab {
                    direction: TabDirection::Next,
                },
            },
            r#"{"type":"focus","target":{"tab":{"direction":"next"}}}"#,
        ),
    ];
    for (action, expected) in &cases {
        let json = serde_json::to_string(action).unwrap();
        assert_eq!(&json, expected, "serialize {action}");
        let round_trip: Action = serde_json::from_str(expected).unwrap();
        assert_eq!(
            &serde_json::to_string(&round_trip).unwrap(),
            expected,
            "round-trip {action}"
        );
    }
}

#[test]
fn ipc_message_serde() {
    let cases = vec![
        (
            IpcMessage::Action {
                action: Action::Exit,
            },
            r#"{"type":"action","action":{"type":"exit"}}"#,
        ),
        (
            IpcMessage::Action {
                action: Action::Focus {
                    target: FocusTarget::Up,
                },
            },
            r#"{"type":"action","action":{"type":"focus","target":"up"}}"#,
        ),
        (
            IpcMessage::Query {
                query: Query::Workspaces,
            },
            r#"{"type":"query","query":{"type":"workspaces"}}"#,
        ),
        (
            IpcMessage::Query {
                query: Query::MinimizedWindows,
            },
            r#"{"type":"query","query":{"type":"minimized_windows"}}"#,
        ),
        (
            IpcMessage::Query {
                query: Query::Monitors,
            },
            r#"{"type":"query","query":{"type":"monitors"}}"#,
        ),
        (IpcMessage::SaveLayout, r#"{"type":"save_layout"}"#),
        (IpcMessage::ApplyLayout, r#"{"type":"apply_layout"}"#),
    ];
    for (msg, expected) in &cases {
        let json = serde_json::to_string(msg).unwrap();
        assert_eq!(&json, expected, "serialize {msg:?}");
        let round_trip: IpcMessage = serde_json::from_str(expected).unwrap();
        assert_eq!(
            &serde_json::to_string(&round_trip).unwrap(),
            expected,
            "round-trip {msg:?}"
        );
    }
}

#[test]
fn action_from_str_round_trip() {
    let cases = [
        "focus up",
        "focus down",
        "focus left",
        "focus right",
        "focus parent",
        "focus tab next",
        "focus tab prev",
        "focus workspace 3",
        "focus workspace 3 --monitor DELL U2720Q #1",
        "focus monitor left",
        "focus monitor foo",
        "move up",
        "move down",
        "move left",
        "move right",
        "move workspace 3",
        "move workspace 3 --monitor HDMI",
        "move monitor left",
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "toggle float",
        "toggle fullscreen",
        "grow",
        "shrink",
        "master more",
        "master fewer",
        "exit",
        "close",
        "mode resize",
        "execute open -a Terminal",
    ];
    for input in cases {
        let action =
            Action::from_str(input).unwrap_or_else(|e| panic!("FromStr failed for {input:?}: {e}"));
        let formatted = action.to_string();
        assert_eq!(
            formatted, input,
            "round-trip mismatch: from_str({input:?}).to_string() = {formatted:?}"
        );
    }
}

#[test]
fn old_master_ratio_strings_are_rejected() {
    for input in ["master grow", "master shrink"] {
        assert!(Action::from_str(input).is_err(), "{input:?} still parses");
    }
    for json in [
        r#"{"type":"master","target":"grow"}"#,
        r#"{"type":"master","target":"shrink"}"#,
    ] {
        assert!(
            serde_json::from_str::<Action>(json).is_err(),
            "{json} still deserializes"
        );
    }
}

#[test]
fn workspace_selector_parse() {
    for (selector, name, monitor) in [
        ("3", "3", None),
        ("my ws --monitor DELL #1", "my ws", Some("DELL #1")),
    ] {
        for verb in ["focus", "move"] {
            let action: Action = format!("{verb} workspace {selector}").parse().unwrap();
            let expected_name = name.to_owned();
            let expected_monitor = monitor.map(str::to_owned);
            let expected = if verb == "focus" {
                Action::Focus {
                    target: FocusTarget::Workspace {
                        name: expected_name,
                        monitor: expected_monitor,
                    },
                }
            } else {
                Action::Move {
                    target: MoveTarget::Workspace {
                        name: expected_name,
                        monitor: expected_monitor,
                    },
                }
            };
            assert_eq!(
                serde_json::to_value(action).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }
}

#[test]
fn unminimize_window_display_uses_space() {
    let id: WindowId = serde_json::from_value(serde_json::json!(7)).unwrap();
    let action = Action::UnminimizeWindow { id };
    assert_eq!(action.to_string(), "unminimize window WindowId(7)");
}

#[test]
fn serialize_workspace_preserves_external_keys_and_state_values() {
    for (state, value) in [
        (WorkspaceState::Attached, "attached"),
        (WorkspaceState::Parked, "parked"),
    ] {
        let workspace = WorkspaceInfo {
            name: "editor".into(),
            monitor: "Left".into(),
            state,
            is_focused: true,
            is_visible: false,
        };
        let expected = serde_json::json!({
            "name": "editor", "monitor": "Left", "state": value,
            "is_focused": true, "is_visible": false,
        });
        assert_eq!(serde_json::to_value(&workspace).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<WorkspaceInfo>(expected).unwrap(),
            workspace
        );
    }
}

#[test]
fn minimized_window_wire_format() {
    let id: WindowId = serde_json::from_value(serde_json::json!(7)).unwrap();
    let cases = vec![
        (
            MinimizedWindow {
                id,
                title: "draft.md".into(),
                app_name: Some("Zed".into()),
                bundle_id: Some("dev.zed.Zed".into()),
                executable_path: None,
            },
            r#"{"id":7,"title":"draft.md","app_name":"Zed","bundle_id":"dev.zed.Zed","executable_path":null}"#,
        ),
        (
            MinimizedWindow {
                id,
                title: "foo".into(),
                app_name: None,
                bundle_id: None,
                executable_path: Some("C:\\Windows\\System32\\notepad.exe".into()),
            },
            r#"{"id":7,"title":"foo","app_name":null,"bundle_id":null,"executable_path":"C:\\Windows\\System32\\notepad.exe"}"#,
        ),
        (
            MinimizedWindow {
                id,
                title: "foo".into(),
                app_name: None,
                bundle_id: None,
                executable_path: None,
            },
            r#"{"id":7,"title":"foo","app_name":null,"bundle_id":null,"executable_path":null}"#,
        ),
    ];
    for (window, expected) in &cases {
        let json = serde_json::to_string(window).unwrap();
        assert_eq!(&json, expected, "serialize {window:?}");
        let round_trip: MinimizedWindow = serde_json::from_str(expected).unwrap();
        let re_json = serde_json::to_string(&round_trip).unwrap();
        assert_eq!(&re_json, expected, "round-trip {window:?}");
    }
}
