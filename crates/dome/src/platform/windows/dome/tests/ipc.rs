use super::{PARTITION_CONFIG, fixture::TestDome};
use crate::action::Query;
use crate::platform::messages::HubMessage;
use dome_core::PreferredLayouts;
use futures::channel::oneshot;

fn action(dome: &mut TestDome, command: &str) -> anyhow::Result<()> {
    let (reply, response) = oneshot::channel();
    dome.dome.handle_hub_message(HubMessage::Action {
        action: command.parse().expect("test action"),
        reply,
    });
    futures::executor::block_on(response).expect("action reply")
}

#[test]
fn dispatch_workspace_action_replies_after_query_and_layout_state_change() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("Editor");
    let second = dome.open("Terminal");
    let saved_frames = (dome.frame(first), dome.frame(second));
    action(&mut dome, "focus workspace work").unwrap();
    let (reply, response) = oneshot::channel();
    dome.dome.handle_hub_message(HubMessage::Query {
        query: Query::Workspaces,
        reply,
    });
    let rows: Vec<dome_ipc::action::WorkspaceInfo> =
        serde_json::from_value(futures::executor::block_on(response).expect("workspace reply"))
            .unwrap();
    assert!(rows.iter().any(|row| row.name == "work"
        && row.monitor == "Display 1"
        && row.is_focused
        && row.is_visible));
    assert!(dome.frame(first).x().value() < 0);
    assert!(dome.frame(second).x().value() < 0);
    action(&mut dome, "focus workspace 0").unwrap();
    assert_eq!((dome.frame(first), dome.frame(second)), saved_frames);
}

#[test]
fn dispatch_capture_and_apply_layout_restores_saved_application_frames() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("Editor");
    let second = dome.open("Terminal");
    let saved_frames = (dome.frame(first), dome.frame(second));
    let (reply, response) = oneshot::channel();
    dome.dome
        .handle_hub_message(HubMessage::CaptureLayouts { reply });
    let layouts = futures::executor::block_on(response).expect("capture reply");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("layout.lua");
    layouts.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("Editor") && saved.contains("Terminal"));
    action(&mut dome, "toggle float").unwrap();
    assert_ne!((dome.frame(first), dome.frame(second)), saved_frames);
    let layouts = PreferredLayouts::from_lua(path.to_str().unwrap(), &saved).unwrap();
    let (reply, response) = oneshot::channel();
    dome.dome
        .handle_hub_message(HubMessage::ApplyLayouts { layouts, reply });
    futures::executor::block_on(response).expect("apply reply");
    assert_eq!((dome.frame(first), dome.frame(second)), saved_frames);
}

#[test]
fn dispatch_close_when_native_close_fails_replies_with_error_context() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Terminal");
    dome.server
        .close_failures
        .borrow_mut()
        .insert(window, "window no longer accepts messages".into());
    let failure = action(&mut dome, "close").expect_err("close refusal reaches IPC worker");
    let message = format!("{failure:#}");
    assert!(message.contains("close native window"));
    assert!(message.contains("window no longer accepts messages"));
}
