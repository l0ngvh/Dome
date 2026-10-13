use super::*;
use crate::platform::windows::Wake;
use crate::platform::windows::dome::events::{HubMessage, WindowCommand};
use std::time::Duration;

#[test]
fn advance_time_without_observation_does_not_retry_placement() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.settle(10);
    env.simulate_resist(window, (100, 100, 600, 400));
    env.clear_moves();
    for _ in 0..3 {
        env.advance_time(Duration::from_secs(2));
        env.deliver_due();
        env.ui_wake();
        env.assert_settled("elapsed time and scene-free UI wakes must not retry geometry");
        assert!(env.deadline(Wake::Ui).is_none());
    }
    env.report_move(window, PixelRect::new(100, 100, 600, 400), 1, env.now());
    assert!(
        env.moved(window),
        "a fresh settled observation must still correct tiled drift"
    );
}

#[test]
fn receive_shutdown_discards_local_scene_and_commands() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.open();
    env.advance_time(Duration::from_millis(100));
    let focus = env.focus_submissions();
    let output = env.output_counts();
    let scene = env.latest_scene();
    env.clear_moves();
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    env.send_ui(HubMessage::Shutdown);
    env.send_ui(HubMessage::Scene(scene));
    env.ui_wake();
    assert!(env.close_submissions().is_empty());
    env.assert_settled("shutdown must skip ordinary placements");
    assert_eq!(env.recovery_calls(), 1);
    assert!(env.deadline(Wake::FocusRequest).is_none());
    env.deliver_due();
    assert_eq!(env.focus_submissions(), focus);
    assert_eq!(env.output_counts(), output);
    env.stop();
    assert_eq!(env.recovery_calls(), 1);
}

#[test]
fn deliver_typed_observation_before_later_limits_pass_and_due_focus() {
    use crate::platform::windows::window_server::{Observation, SettleRead};
    let mut env = TestEnv::new();
    let first = env.open();
    let second = env.open();
    env.settle(10);
    env.hold_reports(true);
    env.clear_trace();
    env.advance_time(Duration::from_millis(100));
    assert!(
        env.trace().is_empty(),
        "advancing time must deliver nothing"
    );
    env.queue_observation(Observation::Settled {
        window: first,
        read: SettleRead {
            observed_at: env.now(),
            minimized: false,
            is_maximized: false,
            placement: Some((PixelRect::new(100, 100, 600, 400), 999)),
        },
    });
    env.send_ui(HubMessage::Command(WindowCommand::Close(first)));
    env.deliver_ready_wakes();
    let trace = env.trace();
    let position = |name| trace.iter().position(|entry| *entry == name).unwrap();
    assert!(position("native intake") < position("result intake"));
    assert!(position("result intake") < position("observations"));
    assert!(position("observations") < position("limits demand"));
    assert!(position("limits demand") < position("close"));
    assert!(trace[position("limits demand")..].contains(&"read pass"));
    assert_eq!(env.focus_submissions(), vec![first]);
    env.clear_trace();
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![first, second]);
    assert_eq!(env.trace(), vec!["external focus"]);
}

#[test]
fn observe_without_scene_neither_repaints_nor_restacks() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.settle(10);
    env.hold_reports(true);
    let output = env.output_counts();
    let orders = env.order_submissions();
    let frames = env.frame_submissions().len();
    env.report_move(window, PixelRect::new(100, 100, 600, 400), 1, env.now());
    assert_eq!(env.frame_submissions().len(), frames + 1);
    assert_eq!(env.output_counts(), output);
    assert_eq!(env.order_submissions(), orders);
    env.layout();
    let after = env.output_counts();
    assert_eq!(after.0, output.0 + 1);
    assert!(after.1 > output.1);
}

#[test]
fn advance_time_leaves_float_and_hidden_targets_without_retries() {
    for floating in [false, true] {
        let mut env = TestEnv::new();
        let window = env.open();
        if floating {
            env.run_actions("toggle float");
        } else {
            env.run_actions("focus workspace 1");
        }
        env.settle(10);
        env.simulate_resist(window, (100, 100, 600, 400));
        env.clear_moves();
        env.hold_reports(true);
        let frames = env.frame_submissions().len();
        let parks = env.park_submissions().len();
        let output = env.output_counts();
        env.clear_trace();
        for _ in 0..3 {
            env.advance_time(Duration::from_secs(2));
            env.deliver_due();
            env.ui_wake();
        }
        assert_eq!(env.frame_submissions().len(), frames);
        assert_eq!(env.park_submissions().len(), parks);
        assert_eq!(env.output_counts(), output);
        assert!(!env.trace().contains(&"read pass"));
        assert!(env.deadline(Wake::Ui).is_none());
        env.report_move(window, PixelRect::new(100, 100, 600, 400), 1, env.now());
        if floating {
            assert_eq!(env.frame_submissions().len(), frames);
            assert!(matches!(env.take_reports().as_slice(),
                [crate::platform::windows::dome::HubEvent::FloatMoved { key, .. }] if *key == window));
        } else {
            assert_eq!(env.park_submissions().len(), parks + 1);
        }
    }
}

#[test]
fn paint_exact_scene_geometry_clips_tabs_and_radii() {
    let mut env = TestEnv::builder()
        .monitors(vec![scaled_monitor(1.25)])
        .build();
    let first = env.open();
    let second = env.open();
    env.run_actions("toggle layout");
    env.set_corner_radius(first, Length::new(7.0));
    env.set_corner_radius(second, Length::new(13.0));
    env.report_titles(vec![
        (first, Some("First title".into())),
        (second, Some("Second title".into())),
    ]);
    env.layout();
    let scene = env.latest_scene();
    let monitor = &scene.monitors[0];
    let painted = env.painted_windows(0);
    let expected: Vec<_> = monitor
        .tiling_windows
        .iter()
        .filter(|p| !p.content_box.is_empty())
        .collect();
    assert_eq!(painted.len(), expected.len());
    for (actual, expected) in painted.iter().zip(expected) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.border_box, expected.border_box);
        assert_eq!(actual.visible_border_box, expected.visible_border_box);
        assert_eq!(actual.content_box, expected.content_box);
        assert_eq!(actual.is_highlighted, expected.is_highlighted);
        assert_eq!(actual.spawn_direction, expected.spawn_direction);
    }
    assert_eq!(env.painted_border_thickness(0), monitor.border_thickness);
    assert_eq!(env.painted_work_area(0), monitor.work_area);
    let tabs = env.tab_containers(0);
    assert_eq!(tabs.len(), monitor.containers.len());
    assert!(tabs.iter().any(|p| p.is_tabbed));
    for (actual, expected) in tabs.iter().zip(&monitor.containers) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.border_box, expected.border_box);
        assert_eq!(actual.visible_border_box, expected.visible_border_box);
        assert_eq!(actual.tab_bar_band, expected.tab_bar_band);
        assert_eq!(actual.titles, expected.titles);
        assert_eq!(actual.active_tab_index, expected.active_tab_index);
        assert_eq!(actual.spawn_direction, expected.spawn_direction);
        assert_eq!(actual.is_highlighted, expected.is_highlighted);
        assert_eq!(actual.is_tabbed, expected.is_tabbed);
    }
    for (placement, radius) in painted.iter().zip(env.painted_corner_radii(0)) {
        let expected = if placement.id == env.window_id(first).unwrap() {
            7.0
        } else {
            13.0
        };
        assert_eq!(radius, Length::new(expected));
    }
    env.run_actions("toggle float");
    let scene = env.latest_scene();
    let expected = scene.monitors[0].float_windows.first().unwrap();
    let actual = env.painted_float().unwrap();
    assert_eq!(actual.id, expected.id);
    assert_eq!(actual.border_box, expected.border_box);
    assert_eq!(actual.visible_border_box, expected.visible_border_box);
    assert_eq!(actual.content_box, expected.content_box);
    assert_eq!(actual.is_highlighted, expected.is_highlighted);
    assert_eq!(
        env.painted_float_scale_and_thickness(),
        Some((scene.monitors[0].scale, scene.monitors[0].border_thickness))
    );
    assert_eq!(env.painted_float_corner_radius(), Some(Length::new(13.0)));
}
