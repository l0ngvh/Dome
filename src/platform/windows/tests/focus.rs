use super::*;
use crate::platform::windows::Wake;
use crate::platform::windows::dome::events::{FocusTarget as SceneFocus, HubMessage};
use std::time::Duration;

#[test]
fn replace_pending_focus_preserves_fixed_submission_boundary() {
    let mut env = TestEnv::new();
    let first = env.open();
    let start = env.now();
    assert_eq!(env.focus_submissions(), vec![first]);
    let second = env.open();
    let mut scene = env.latest_scene();
    scene.focus_request = Some(SceneFocus::Window(second));
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.ui_wake();
    assert_eq!(
        env.deadline(Wake::FocusRequest),
        Some(start + Duration::from_millis(100))
    );
    env.advance_time(Duration::from_millis(99));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![first]);
    scene.focus_request = None;
    env.send_ui(HubMessage::Scene(scene.clone()));
    scene.focus_request = Some(SceneFocus::Window(first));
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.ui_wake();
    assert_eq!(
        env.deadline(Wake::FocusRequest),
        Some(start + Duration::from_millis(100))
    );
    env.advance_time(Duration::from_millis(1));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![first, first]);
    env.send_ui(HubMessage::Scene(scene));
    env.ui_wake();
    env.advance_time(Duration::from_secs(1));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![first, first]);
}

#[test]
fn focus_overlay_preserves_external_submission_interval() {
    let mut env = TestEnv::new();
    let window = env.open();
    let start = env.now();
    let mut scene = env.latest_scene();
    scene.focus_request = Some(SceneFocus::Monitor(scene.monitors[0].id));
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.ui_wake();
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
    assert!(env.deadline(Wake::FocusRequest).is_none());
    scene.focus_request = Some(SceneFocus::Window(window));
    env.send_ui(HubMessage::Scene(scene));
    env.ui_wake();
    assert_eq!(
        env.deadline(Wake::FocusRequest),
        Some(start + Duration::from_millis(100))
    );
    env.advance_time(Duration::from_millis(100));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![window, window]);
    assert_eq!(env.focus_target(), FocusTarget::Window(window));
}

#[test]
fn focus_new_overlay_after_content_before_first_external_request() {
    let mut env = TestEnv::new();
    env.hold_scenes(true);
    env.add_monitor(second_monitor());
    let window = env.open();
    assert!(env.focus_submissions().is_empty());
    let mut scene = env.latest_scene();
    let monitor = scene
        .monitors
        .iter()
        .find(|monitor| monitor.native == 2)
        .unwrap()
        .id;
    assert!(env.tiling_overlay_id_for(monitor).is_none());
    scene.focus_request = Some(SceneFocus::Monitor(monitor));
    env.clear_trace();
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.ui_wake();
    assert!(env.tiling_overlay_id_for(monitor).is_some());
    let trace = env.trace();
    assert!(
        trace.iter().position(|step| *step == "content").unwrap()
            < trace
                .iter()
                .position(|step| *step == "overlay focus")
                .unwrap()
    );
    assert!(env.focus_submissions().is_empty());
    scene.focus_request = Some(SceneFocus::Window(window));
    env.send_ui(HubMessage::Scene(scene));
    env.ui_wake();
    assert_eq!(env.deadline(Wake::FocusRequest), Some(env.now()));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![window]);
}

#[test]
fn suppress_pending_focus_after_exclusive_observation() {
    use crate::platform::windows::window_server::Observation;
    let mut env = TestEnv::new();
    let first = env.open();
    let second = env.open();
    env.hold_reports(true);
    env.inject_observation(Observation::ExclusiveFullscreen(Some(second)));
    env.advance_time(Duration::from_millis(100));
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![first]);
    assert!(env.deadline(Wake::FocusRequest).is_none());
}
