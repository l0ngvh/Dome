use super::*;

#[test]
fn deliver_unchanged_scene_after_minimize_or_fullscreen_report_does_not_undo_observation() {
    use crate::platform::macos::dome::events::{
        FullscreenKind, HubEvent, HubMessage, WindowPlacement,
    };

    for fullscreen in [false, true] {
        let mut env = TestEnv::new();
        env.setup();
        let key = env.spawn_window(100, "App", "Window");
        env.discover_windows(&[key]);
        env.settle(10);
        let held = env.scene();
        env.hold_scenes(true);
        env.clear_moves();
        env.clear_effects();
        if fullscreen {
            env.set_window_frame(key, PixelRect::new(0, 0, 1920, 1080));
        } else {
            env.set_minimized(key, true);
        }
        env.queue_current_facts(key);
        env.ui_pass();
        let reports = env.take_reports();
        if fullscreen {
            assert!(matches!(
                &reports[..],
                [HubEvent::FullscreenChanged {
                    fullscreen: Some(FullscreenKind::Borderless),
                    ..
                }]
            ));
        } else {
            assert!(
                matches!(&reports[..], [HubEvent::WindowMinimized(reported)] if *reported == key)
            );
        }
        for report in reports {
            env.deliver_report(report);
        }
        env.pump_scenes();
        env.send_message(HubMessage::Scene(held));
        env.ui_pass();
        assert!(env.recorded_moves().is_empty());
        assert!(!env.effects().contains(&format!("unminimize {key}")));
        assert_eq!(env.is_minimized(key), !fullscreen);
        let response = env.take_held_scenes().pop().unwrap();
        assert!(
            matches!(
                response.windows[&key].placement,
                WindowPlacement::Fullscreen { .. }
            ) == fullscreen
        );
        env.send_message(HubMessage::Scene(response));
        env.ui_pass();
        assert!(env.recorded_moves().is_empty());
    }
}

#[test]
fn observe_restored_window_reports_restore_before_fullscreen_or_float_change() {
    use crate::platform::macos::dome::events::{FullscreenKind, HubEvent};

    for fullscreen in [false, true] {
        let mut env = TestEnv::new();
        env.setup();
        let key = env.spawn_window(100, "App", "Window");
        env.discover_windows(&[key]);
        if !fullscreen {
            env.run_actions("toggle float");
        }
        env.settle(10);
        env.user_minimize(key);
        env.settle(10);
        env.set_minimized(key, false);
        let frame = if fullscreen {
            PixelRect::new(0, 0, 1920, 1080)
        } else {
            PixelRect::new(200, 150, 600, 400)
        };
        env.set_window_frame(key, frame);
        env.queue_current_facts(key);
        env.ui_pass();
        let reports = env.take_reports();
        assert!(
            matches!(reports.first(), Some(HubEvent::WindowRestored(reported)) if *reported == key)
        );
        if fullscreen {
            assert!(matches!(
                reports.get(1),
                Some(HubEvent::FullscreenChanged {
                    fullscreen: Some(FullscreenKind::Borderless),
                    ..
                })
            ));
        } else {
            assert!(
                matches!(reports.get(1), Some(HubEvent::FloatMoved { key: reported, frame: moved }) if *reported == key && *moved == frame)
            );
        }
        for report in reports {
            env.deliver_report(report);
        }
        env.pump_scenes();
        env.ui_pass();
        assert!(!env.is_minimized(key));
        assert_eq!(
            env.window_frame(key),
            (
                frame.x().value(),
                frame.y().value(),
                frame.width().value(),
                frame.height().value()
            )
        );
    }
}

#[test]
fn deliver_held_scene_after_float_report_waits_for_response_target() {
    use crate::platform::macos::dome::events::{HubEvent, HubMessage};
    use crate::platform::macos::window_server::PositionReadOrigin;

    let mut env = TestEnv::new();
    env.setup();
    let key = env.spawn_window(100, "App", "Float");
    env.discover_windows(&[key]);
    env.run_actions("toggle float");
    env.settle(10);
    let unchanged = env.scene();
    env.hold_scenes(true);
    env.clear_moves();
    let moved = PixelRect::new(200, 150, 600, 400);
    env.set_window_frame(key, moved);
    env.queue_position(key, moved, PositionReadOrigin::Periodic);
    env.ui_pass();
    assert!(env.recorded_moves().is_empty());
    let reports = env.take_reports();
    assert!(
        matches!(&reports[..], [HubEvent::FloatMoved { key: reported, frame }] if *reported == key && *frame == moved)
    );
    for report in reports {
        env.deliver_report(report);
    }
    env.pump_scenes();
    env.send_message(HubMessage::Scene(unchanged.clone()));
    env.ui_pass();
    assert!(
        env.recorded_moves().is_empty(),
        "unchanged held target must not correct the OS move"
    );
    let response = env.take_held_scenes().pop().unwrap();
    env.send_message(HubMessage::Scene(response));
    env.ui_pass();
    assert_eq!(env.recorded_moves(), [(key, 200, 150, 600, 400)]);
    env.clear_moves();
    env.send_message(HubMessage::Scene(unchanged));
    env.ui_pass();
    assert_eq!(
        env.recorded_moves().len(),
        1,
        "a genuinely changed target remains eligible"
    );
}

#[test]
fn deliver_semantic_reports_validates_current_hub_mode_identity_and_monitor() {
    use crate::platform::macos::dome::events::{FullscreenKind, HubEvent, WindowPlacement};

    let mut env = TestEnv::new();
    env.setup();
    let key = env.spawn_window(100, "App", "Window");
    env.discover_windows(&[key]);
    env.run_actions("toggle float");
    env.settle(10);
    let id = env.window_id(key).unwrap();
    let original = env.window_frame(key);
    env.hold_scenes(true);
    env.with_hub(|hub| hub.minimize_window(id));
    env.deliver_report(HubEvent::FloatMoved {
        key,
        frame: PixelRect::new(200, 150, 600, 400),
    });
    env.deliver_report(HubEvent::FullscreenChanged {
        key,
        fullscreen: Some(FullscreenKind::Borderless),
        frame: Some(PixelRect::new(0, 0, 1920, 1080)),
    });
    env.pump_scenes();
    assert!(matches!(
        env.scene().windows[&key].placement,
        WindowPlacement::Minimized
    ));
    assert_eq!(env.window_frame(key), original);
    env.deliver_report(HubEvent::WindowRestored(key));
    env.deliver_report(HubEvent::FloatMoved {
        key,
        frame: PixelRect::new(200, 150, 600, 400),
    });
    env.pump_scenes();
    let response = env.scene();
    let WindowPlacement::Float { placement, .. } = response.windows[&key].placement else {
        panic!("restored float must remain floating")
    };
    assert_eq!(placement.content_box, PixelRect::new(200, 150, 600, 400));

    let mut resized = default_monitor();
    resized.work_area = PixelRect::new(0, 0, 2560, 1440);
    resized.bounds = resized.work_area.to_dimension();
    resized.full_height = 1440.0;
    env.change_monitors(vec![resized]);
    env.deliver_report(HubEvent::FullscreenChanged {
        key,
        fullscreen: Some(FullscreenKind::Borderless),
        frame: Some(PixelRect::new(0, 0, 1920, 1080)),
    });
    env.pump_scenes();
    assert!(matches!(
        env.scene().windows[&key].placement,
        WindowPlacement::Float { .. }
    ));
    env.remove_windows(&[key]);
    for report in [
        HubEvent::WindowGone(key),
        HubEvent::TitleChanged {
            key,
            title: "late title".into(),
        },
        HubEvent::SizeConstraint {
            key,
            limits: crate::core::LimitObservation::default(),
        },
        HubEvent::FocusReported(key),
        HubEvent::WindowRestored(key),
        HubEvent::WindowMinimized(key),
        HubEvent::FloatMoved {
            key,
            frame: PixelRect::new(10, 20, 300, 400),
        },
        HubEvent::FullscreenChanged {
            key,
            fullscreen: Some(FullscreenKind::Native),
            frame: None,
        },
    ] {
        env.deliver_report(report);
    }
    env.pump_scenes();
    assert!(
        !env.scene().windows.contains_key(&key),
        "late reports cannot resurrect a removed identity"
    );
}

#[test]
fn deliver_queued_float_report_filters_retired_identity() {
    use crate::platform::macos::dome::events::{HubEvent, WindowPlacement};
    use crate::platform::macos::window_server::PositionReadOrigin;

    for retired in [false, true] {
        let mut env = TestEnv::new();
        env.setup();
        let key = env.spawn_window(100, "App", "Window");
        env.discover_windows(&[key]);
        env.run_actions("toggle float");
        env.settle(10);
        let frame = PixelRect::new(200, 150, 600, 400);
        env.set_window_frame(key, frame);
        env.queue_position(key, frame, PositionReadOrigin::Periodic);
        env.ui_pass();
        let reports = env.take_reports();
        assert!(
            matches!(&reports[..], [HubEvent::FloatMoved { key: reported, frame: moved }] if *reported == key && *moved == frame)
        );
        if retired {
            env.remove_windows(&[key]);
            env.deliver_report(HubEvent::WindowGone(key));
        }
        for report in reports {
            env.deliver_report(report);
        }
        env.pump_scenes();
        if retired {
            assert!(env.scene().windows.is_empty());
        } else {
            let WindowPlacement::Float { placement, .. } = env.scene().windows[&key].placement
            else {
                panic!("live report must update the float placement")
            };
            assert_eq!(placement.content_box, frame);
        }
    }
}

#[test]
fn a_fractional_work_area_keeps_the_fullscreen_window_inside_it() {
    let mut macos = TestEnv::new();
    macos.setup();

    let mut monitor = default_monitor();
    monitor.work_area = PixelRect::from_dimension_inward(FRACTIONAL_WORK_AREA);
    macos.change_monitors(vec![monitor]);

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.run_actions("toggle fullscreen");
    macos.settle(10);
    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);

    // Round trip through another workspace, which returns via the `Offscreen` arm.
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);
}

#[test]
fn fullscreen_window_restored_from_offscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle fullscreen");
    macos.settle(10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg2));

    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_hidden_on_workspace_switch() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));
    assert!(!macos.is_offscreen(cg1));

    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn minimized_window_reappears_non_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(10);

    assert!(!macos.is_minimized(cg1));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    macos.exit_native_fullscreen(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn offscreen_window_becomes_borderless_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg1));

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    assert!(macos.is_minimized(cg1));
}

#[test]
fn new_window_already_borderless_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window_at(100, "Safari", "Google", 0, 0, 1920, 1080);
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn float_focus_unfocus_cycle() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);
    assert!(!macos.is_offscreen(cg2));

    macos.mirror_clicked(cg1);
    macos.settle(10);
    assert!(macos.is_offscreen(cg2));

    macos.mirror_clicked(cg2);
    macos.settle(10);
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn hide_noop_for_native_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);
    let frame_before = macos.window_frame(cg1);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), frame_before);
}

#[test]
fn hide_noop_for_minimized() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    // Second workspace switch -- minimize should NOT be called again
    macos.run_actions("focus workspace 2");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn offscreen_window_rehidden_on_external_move() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg1));

    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(10);

    assert!(macos.is_offscreen(cg1));
}

#[test]
fn borderless_fullscreen_full_lifecycle() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg2));

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert!(!macos.is_minimized(cg1));
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn minimized_borderless_reappears_still_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn borderless_fullscreen_exit_to_tiling() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_enter() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    // Hub treats fullscreen as taking the full monitor -- sibling is hidden.
    // In real macOS, space_changed would restore siblings on the original Space.
    assert!(macos.is_offscreen(cg2));
}

#[test]
fn native_fullscreen_exit() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    macos.exit_native_fullscreen(cg1, 200, 200, 800, 600);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn toggle_fullscreen_hides_siblings() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle fullscreen");
    macos.settle(10);

    assert_eq!(macos.window_frame(cg2), (0, 0, 1920, 1080));
    assert!(macos.is_offscreen(cg1));
}

#[test]
fn native_fullscreen_blocks_toggle_float() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.mirror_clicked(cg1);
    macos.settle(10);
    macos.enter_native_fullscreen(cg1);
    macos.settle(10);
    let frame_before = macos.window_frame(cg1);

    // toggle_float should be blocked by ProtectFullscreen restriction
    macos.run_actions("toggle float");
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), frame_before);

    macos.exit_native_fullscreen(cg1, 200, 200, 800, 600);
    macos.settle(10);

    let cg2_before = macos.window_frame(cg2);
    macos.run_actions("toggle float");
    macos.settle(10);
    assert!(!macos.is_offscreen(cg2));
    // cg1 is now floating, cg2 expands to fill the tiling area
    assert_ne!(macos.window_frame(cg2), cg2_before);
}

#[test]
fn borderless_fullscreen_blocks_toggle_float() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // toggle_float should be blocked by ProtectFullscreen restriction
    macos.run_actions("toggle float");
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    macos.simulate_external_move(cg1, 100, 100, 800, 600);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);
    assert_ne!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn borderless_fullscreen_allows_move_workspace() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));

    // move_workspace should be allowed despite ProtectFullscreen restriction
    macos.run_actions("move workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless_on_unfocused_workspace() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(!macos.is_offscreen(cg1));

    macos.exit_native_fullscreen(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    assert!(macos.is_minimized(cg1));
}

#[test]
fn native_fullscreen_exit_to_borderless_unfocused_then_switch_back() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    macos.run_actions("focus workspace 1");
    macos.settle(10);

    macos.exit_native_fullscreen(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.run_actions("focus workspace 0");
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn user_minimize_via_reconcile() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.user_minimize(cg1);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg2));
    let (_, _, w, _) = macos.window_frame(cg2);
    assert!(
        w > 1900,
        "cg2 should take (nearly) full screen, got width {w}"
    );
}

#[test]
fn user_minimized_window_receives_move_event() {
    // Simulates kAXWindowDeminiaturizedNotification routed through the move path.
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.simulate_external_move(cg1, 200, 200, 800, 600);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg2), (4, 4, 952, 1072));
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_unminimize_via_focus() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.user_minimize(cg1);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));

    macos.focus_window(cg1);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimized_deminiaturize_then_focus() {
    // Full flow: user minimizes → deminiaturize notification fires (move event) →
    // focus event arrives. The subsequent focus event is a no-op (window already
    // placed).
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.simulate_external_move(cg1, 300, 100, 800, 600);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));

    macos.focus_window(cg1);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (964, 4, 952, 1072));
}

#[test]
fn user_minimize_single_window_then_unminimize() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);
    assert!(!macos.is_offscreen(cg1));

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.focus_window(cg1);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn user_minimize_noop_on_unknown_window() {
    // If reconcile reports a minimize for an unknown cg_id, it should not panic.
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    let unknown_cg = 999;
    macos.report_minimized(&[unknown_cg]);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
}

#[test]
fn window_turned_borderless_fullscreen_after_user_minimize() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), (0, 0, 1920, 1080));
}

#[test]
fn tiling_state_preserved_through_user_minimize_round_trip() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    let (_, _, w_before, h_before) = macos.window_frame(cg1);

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.focus_window(cg1);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
    let (_, _, w_after, h_after) = macos.window_frame(cg1);
    assert_eq!((w_after, h_after), (w_before, h_before));
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn borderless_fullscreen_state_preserved_through_dome_minimize_round_trip() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);

    let placed = macos.window_frame(cg1);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_minimized(cg1));

    macos.run_actions("focus workspace 0");
    macos.settle(10);

    assert_eq!(macos.window_frame(cg1), placed);
    assert!(!macos.is_minimized(cg1));
}

#[test]
fn float_state_preserved_through_user_minimize_round_trip() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);

    let placed = macos.window_frame(cg2);

    macos.user_minimize(cg2);
    macos.settle(10);

    macos.focus_window(cg2);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg2), placed);
    assert!(!macos.is_minimized(cg2));
}

#[test]
fn tiling_state_restored_by_the_unminimize_action() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    let placed = macos.window_frame(cg2);

    macos.user_minimize(cg2);
    macos.settle(10);

    macos.run_unminimize(cg2);
    macos.settle(10);

    assert_eq!(macos.window_frame(cg2), placed);
    assert!(!macos.is_minimized(cg2));
}

#[test]
fn native_fullscreen_state_preserved_through_user_minimize_round_trip() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg1);
    macos.settle(10);

    let placed = macos.window_frame(cg1);

    macos.user_minimize(cg1);
    macos.settle(10);

    macos.run_unminimize(cg1);
    macos.settle(10);

    // Geometry unchanged (NativeFullscreen windows are positioned by macOS)
    assert_eq!(macos.window_frame(cg1), placed);
    assert!(!macos.is_minimized(cg1));
}
