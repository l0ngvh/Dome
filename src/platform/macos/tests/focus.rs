use std::time::Duration;

use super::*;
use crate::platform::macos::Wake;
use crate::platform::macos::dome::events::{FocusTarget, HubEvent, HubMessage};

fn pair() -> (TestEnv, CGWindowID, CGWindowID) {
    let mut env = TestEnv::new();
    env.setup();
    let first = env.spawn_window(100, "First", "First");
    let second = env.spawn_window(200, "Second", "Second");
    env.discover_windows(&[first, second]);
    env.clear_effects();
    (env, first, second)
}

fn request_scene(env: &mut TestEnv, target: Option<FocusTarget>) {
    let mut scene = env.scene();
    scene.focus_request = target;
    env.send_message(HubMessage::Scene(scene));
    env.ui_pass();
}

fn submissions(env: &TestEnv) -> Vec<String> {
    env.effects()
        .into_iter()
        .filter(|effect| effect.starts_with("focus "))
        .collect()
}

#[test]
fn request_focus_releases_newest_target_at_fixed_boundary() {
    let (mut env, first, second) = pair();
    request_scene(&mut env, Some(FocusTarget::Window(first)));
    assert!(submissions(&env).is_empty());
    env.release_ready_focus();
    assert_eq!(submissions(&env), [format!("focus {first}")]);
    let boundary = env.now() + Duration::from_millis(100);
    env.advance(Duration::from_millis(30));
    request_scene(&mut env, Some(FocusTarget::Window(second)));
    env.release_ready_focus();
    assert_eq!(env.deadline(Wake::FocusRequest), Some(boundary));
    env.advance(Duration::from_millis(69));
    env.focus.request(first);
    env.release_ready_focus();
    assert_eq!(submissions(&env).len(), 1);
    assert_eq!(env.deadline(Wake::FocusRequest), Some(boundary));
    env.advance(Duration::from_millis(1));
    assert_eq!(
        submissions(&env).len(),
        1,
        "time advance must not release work"
    );
    env.release_ready_focus();
    assert_eq!(
        submissions(&env),
        [format!("focus {first}"), format!("focus {first}")]
    );

    request_scene(&mut env, Some(FocusTarget::Window(second)));
    env.advance(Duration::from_millis(100));
    env.release_ready_focus();
    let count = submissions(&env).len();
    request_scene(&mut env, Some(FocusTarget::Window(second)));
    env.advance(Duration::from_millis(100));
    env.release_ready_focus();
    assert_eq!(
        submissions(&env).len(),
        count,
        "equal scene intent is deduplicated"
    );
    request_scene(&mut env, None);
    request_scene(&mut env, Some(FocusTarget::Window(second)));
    env.release_ready_focus();
    assert_eq!(submissions(&env).len(), count + 1);
    env.focus.request(second);
    env.advance(Duration::from_millis(100));
    env.release_ready_focus();
    assert_eq!(
        submissions(&env).len(),
        count + 2,
        "application requests remain repeatable"
    );
}

#[test]
fn request_pending_focus_preserves_submitted_target_until_release() {
    let (mut env, first, second) = pair();
    env.focus.request(first);
    env.release_ready_focus();
    env.focus.request(second);
    assert!(env.focus.is_submitted(first));
    env.focus.clear_pending();
    env.release_ready_focus();
    assert!(env.focus.is_submitted(first));
    env.focus.request(second);
    env.advance(Duration::from_millis(100));
    env.release_ready_focus();
    assert!(!env.focus.is_submitted(first));
    assert!(env.focus.is_submitted(second));
}

#[test]
fn focus_overlay_preserves_external_spacing_and_follows_content() {
    for prior_submission in [false, true] {
        let (mut env, first, second) = pair();
        if prior_submission {
            env.focus.request(first);
            env.release_ready_focus();
        }
        env.focus.request(second);
        let boundary = env.now() + Duration::from_millis(100);
        env.clear_effects();
        let monitor = env.scene().monitors[0].id;
        request_scene(&mut env, Some(FocusTarget::Monitor(monitor)));
        let effects = env.effects();
        let content = effects.iter().position(|effect| effect == "tabs").unwrap();
        let focus = effects
            .iter()
            .position(|effect| effect == "overlay focus")
            .unwrap();
        assert!(content < focus);
        assert!(!env.focus.is_submitted(first));
        assert_eq!(env.deadline(Wake::FocusRequest), None);
        env.focus.request(second);
        env.release_ready_focus();
        if prior_submission {
            assert!(submissions(&env).is_empty());
            assert_eq!(env.deadline(Wake::FocusRequest), Some(boundary));
            env.advance(Duration::from_millis(100));
            env.release_ready_focus();
        }
        assert_eq!(submissions(&env), [format!("focus {second}")]);
    }
}

#[test]
fn report_focus_clears_intent_without_echo_and_policy_can_request_again() {
    let (mut env, first, second) = pair();
    env.deliver_report(HubEvent::FocusReported(first));
    env.pump_scenes();
    env.ui_pass();
    assert_eq!(env.scene().focused_window, Some(first));
    env.deliver_report(HubEvent::FocusReported(second));
    env.pump_scenes();
    env.ui_pass();
    assert_eq!(env.scene().focused_window, Some(second));
    env.deliver_report(HubEvent::FocusReported(0));
    env.pump_scenes();
    env.ui_pass();
    env.release_ready_focus();
    assert!(submissions(&env).is_empty());
    assert_eq!(env.scene().focus_request, None);

    env.run_actions("focus left");
    assert_eq!(env.scene().focus_request, Some(FocusTarget::Window(first)));
    env.deliver_report(HubEvent::TitleChanged {
        key: first,
        title: "Renamed".into(),
    });
    env.pump_scenes();
    env.ui_pass();
    assert_eq!(env.scene().focus_request, Some(FocusTarget::Window(first)));
    env.release_ready_focus();
    assert_eq!(submissions(&env), [format!("focus {first}")]);
}
