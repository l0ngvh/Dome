use super::fixture::{PARTITION_CONFIG, TestDome};
use crate::action::{Query, WorkspaceInfo};
use crate::platform::messages::HubMessage;
use dome_core::PixelRect;

#[test]
fn execute_hub_action_updates_frames_and_workspace_query() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("Editor");
    let second = dome.open("Terminal");
    dome.hub_request(|reply| HubMessage::Action {
        action: "focus workspace work".parse().unwrap(),
        reply,
    })
    .unwrap();
    let rows = dome.hub_request(|reply| HubMessage::Query {
        query: Query::Workspaces,
        reply,
    });
    let rows: Vec<WorkspaceInfo> = serde_json::from_value(rows).unwrap();
    assert!(rows.iter().any(|row| row.name == "work"
        && row.monitor == "Display 1"
        && row.is_focused
        && row.is_visible));
    assert_eq!(dome.frame(first), PixelRect::new(99, 19, 48, 18));
    assert_eq!(dome.frame(second), PixelRect::new(99, 19, 48, 18));
    dome.hub_request(|reply| HubMessage::Action {
        action: "focus workspace 0".parse().unwrap(),
        reply,
    })
    .unwrap();
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(second), PixelRect::new(51, 1, 48, 18));
}

#[test]
fn capture_then_apply_hub_layout_restores_native_frames() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("Editor");
    let second = dome.open("Terminal");
    let original = (dome.frame(first), dome.frame(second));
    let layouts = dome.hub_request(|reply| HubMessage::CaptureLayouts { reply });
    dome.hub_request(|reply| HubMessage::Action {
        action: "toggle float".parse().unwrap(),
        reply,
    })
    .unwrap();
    assert_ne!((dome.frame(first), dome.frame(second)), original);
    assert!(dome.overlays().floats.contains_key(&second));
    dome.hub_request(|reply| HubMessage::ApplyLayouts { layouts, reply });
    assert_eq!((dome.frame(first), dome.frame(second)), original);
    assert!(dome.overlays().floats.is_empty());
}

#[test]
fn execute_hub_action_when_window_disappeared_replies_with_failure() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Terminal");
    dome.pending_removals(&[window]);
    let result = dome.hub_request(|reply| HubMessage::Action {
        action: "close".parse().unwrap(),
        reply,
    });
    assert!(format!("{:#}", result.unwrap_err()).contains("window no longer accepts messages"));
}
