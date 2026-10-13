use super::*;
use crate::platform::windows::dome::HubEvent;
use crate::platform::windows::dome::events::{HubMessage, WindowCommand, WindowPlacement};
use crate::platform::windows::window_server::Observation;

#[test]
fn switch_workspace_retains_hidden_and_minimized_scene_members() {
    let mut env = TestEnv::new();
    let first = env.open();
    let second = env.open();
    env.minimize_window(second);
    env.run_actions("focus workspace 1");
    let scene = env.latest_scene();
    assert!(matches!(
        scene.windows[&first].placement,
        WindowPlacement::Hidden
    ));
    assert!(matches!(
        scene.windows[&second].placement,
        WindowPlacement::Minimized
    ));
    assert_eq!(scene.windows[&first].id, env.window_id(first).unwrap());
    assert_eq!(scene.windows.len(), 2);
    assert!(env.is_offscreen(first));
    assert!(env.is_minimized(second));
}

#[test]
fn drain_intake_without_scene_preserves_duplicate_commands() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    env.ui_wake();
    assert_eq!(env.close_submissions(), vec![window, window]);
}

#[test]
fn report_float_move_with_held_scene_waits_for_response() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("toggle float");
    env.settle(10);
    let old = env.latest_scene();
    env.clear_moves();
    env.hold_scenes(true);
    env.report_move(window, PixelRect::new(200, 150, 600, 400), 1, env.now());
    env.assert_settled("the report must not submit geometry before the response scene");
    env.send_ui(HubMessage::Scene(old));
    env.ui_wake();
    env.assert_settled("the held old target must remain a no-op");
    env.advance_time(std::time::Duration::from_secs(4));
    env.ui_wake();
    env.assert_settled("elapsed time must not restore the old float target");
    env.hold_scenes(false);
    env.release();
    assert_eq!(env.dim(window), dim(200, 150, 600, 400));
    assert!(
        env.moved(window),
        "the responding scene must submit the observed frame"
    );
    env.clear_moves();
    env.layout();
    env.assert_settled("equal responding scene must not write twice");
}

#[test]
fn coalesce_scenes_preserves_every_display_refresh_marker() {
    let mut env = TestEnv::new();
    let window = env.open();
    let before = env.limits_demand().len();
    let mut scene = env.latest_scene();
    let mut first = default_monitor();
    first.dots_per_inch = 120;
    let mut last = first.clone();
    last.full_rect = PixelRect::new(-10, -20, 3000, 2000);
    last.dots_per_inch = 144;
    env.send_ui(HubMessage::DisplayRefreshed(vec![first]));
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.send_ui(HubMessage::DisplayRefreshed(vec![last.clone()]));
    let newest_frame = PixelRect::new(100, 150, 900, 650);
    let WindowPlacement::Tiling { placement, .. } =
        &mut scene.windows.get_mut(&window).unwrap().placement
    else {
        panic!("the marker scenario requires a tiled window");
    };
    placement.content_box = newest_frame;
    scene.monitors[0].tiling_windows[0].content_box = newest_frame;
    env.send_ui(HubMessage::Scene(scene));
    let frames = env.frame_submissions().len();
    env.clear_trace();
    env.ui_wake();
    assert_eq!(
        &env.frame_submissions()[frames..],
        &[(window, newest_frame)]
    );
    let trace = env.trace();
    assert!(
        trace.iter().rposition(|step| *step == "displays").unwrap()
            < trace.iter().position(|step| *step == "frame").unwrap()
    );
    assert_eq!(env.displayed_monitors()[0].full_rect, last.full_rect);
    assert_eq!(env.displayed_monitors()[0].dots_per_inch, 144);
    assert_eq!(&env.limits_demand()[before..], &[window, window]);
    env.send_ui(HubMessage::DisplayRefreshed(Vec::new()));
    env.ui_wake();
    assert_eq!(env.displayed_monitors()[0].dots_per_inch, 144);
}

#[test]
fn report_focus_clears_intent_without_waiting() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.minimize_window(window);
    env.hold_reports(true);
    env.inject_observation(Observation::Foreground { window });
    let reports = env.take_reports();
    assert!(matches!(reports.as_slice(), [HubEvent::FocusReported(key)] if *key == window));
    env.hold_reports(false);
    env.inject_observation(Observation::Foreground { window });
    assert_eq!(env.query_minimized_windows_json(), "[]");
    assert!(env.latest_scene().focus_request.is_none());
    env.inject_observation(Observation::Foreground {
        window: HwndId::test(99999),
    });
    assert!(env.latest_scene().focus_request.is_none());
}

#[test]
fn admit_hidden_only_after_scene_and_revoke_ordinary_writes_on_omission() {
    use crate::platform::windows::dome::events::FocusTarget as SceneFocus;
    let mut env = TestEnv::new();
    env.hold_scenes(true);
    let window = env.open();
    assert!(env.frame_submissions().is_empty());
    assert!(env.park_submissions().is_empty());
    assert!(env.focus_submissions().is_empty());
    assert!(env.recovery_records().is_empty());
    let mut scene = env.latest_scene();
    scene.windows.get_mut(&window).unwrap().placement = WindowPlacement::Hidden;
    scene.focus_request = None;
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.ui_wake();
    assert_eq!(env.park_submissions(), vec![window]);
    assert!(env.frame_submissions().is_empty());
    scene.windows.clear();
    scene.focus_request = Some(SceneFocus::Window(window));
    env.send_ui(HubMessage::Scene(scene));
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    env.ui_wake();
    env.deliver_due();
    env.report_move(window, PixelRect::new(100, 100, 500, 400), 1, env.now());
    assert_eq!(env.park_submissions(), vec![window]);
    assert!(env.frame_submissions().is_empty());
    assert!(env.close_submissions().is_empty());
    assert!(env.focus_submissions().is_empty());
}

#[test]
fn admit_minimized_or_exclusive_window_without_geometry() {
    for exclusive in [false, true] {
        let mut env = TestEnv::new();
        env.hold_scenes(true);
        let window = env.open();
        if exclusive {
            env.inject_observation(Observation::ExclusiveFullscreen(Some(window)));
        }
        let mut scene = env.latest_scene();
        if !exclusive {
            scene.windows.get_mut(&window).unwrap().placement = WindowPlacement::Minimized;
        }
        scene.focus_request = None;
        env.send_ui(HubMessage::Scene(scene));
        env.ui_wake();
        env.report_move(window, PixelRect::new(100, 100, 500, 400), 1, env.now());
        assert!(env.frame_submissions().is_empty());
        assert!(env.park_submissions().is_empty());
        assert!(env.minimize_submissions().is_empty());
    }
}

#[test]
fn admit_ignored_discovery_on_reload_without_native_creation() {
    use crate::core::WindowMatcher;
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.ignore.push(WindowMatcher {
                process: Some("bloat.exe".into()),
                ..Default::default()
            });
        })
        .build();
    let window = env.window().process("bloat.exe").open();
    assert!(!env.is_managed(window));
    assert!(!env.latest_scene().windows.contains_key(&window));
    assert!(env.frame_submissions().is_empty());
    assert!(env.park_submissions().is_empty());
    env.change_config("return {}");
    assert!(env.is_managed(window));
    assert!(env.latest_scene().windows.contains_key(&window));
    assert!(
        env.frame_submissions()
            .iter()
            .any(|(key, _)| *key == window)
    );
}

#[test]
fn retain_all_modes_and_monitor_order_in_full_scene() {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .build();
    let tiled = env.open();
    let minimized = env.open();
    env.minimize_window(minimized);
    let floating = env.open();
    env.run_actions("toggle float");
    env.run_actions("focus monitor right");
    let fullscreen = env
        .window()
        .spawned_at(second_monitor().work_area.to_dimension())
        .open();
    let scene = env.latest_scene();
    assert_eq!(scene.windows.len(), 4);
    assert_eq!(scene.monitors.len(), 2);
    assert!(matches!(
        scene.windows[&tiled].placement,
        WindowPlacement::Tiling { .. }
    ));
    assert!(matches!(
        scene.windows[&floating].placement,
        WindowPlacement::Float { .. }
    ));
    assert!(matches!(
        scene.windows[&minimized].placement,
        WindowPlacement::Minimized
    ));
    assert!(matches!(
        scene.windows[&fullscreen].placement,
        WindowPlacement::Fullscreen { .. }
    ));
    let first = scene.monitors.iter().find(|m| m.native == 1).unwrap();
    assert_eq!(
        first
            .tiling_windows
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![env.window_id(tiled).unwrap()]
    );
    assert_eq!(
        first.float_windows.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![env.window_id(floating).unwrap()]
    );
    let before = env.frame_submissions().len();
    env.layout();
    assert_eq!(env.frame_submissions().len(), before);
    env.enter_exclusive_fullscreen(fullscreen);
    let before = env.frame_submissions().len();
    env.run_actions("focus workspace 1");
    env.layout();
    assert!(
        !env.frame_submissions()[before..]
            .iter()
            .any(|(key, _)| *key == fullscreen)
    );
    assert!(!env.park_submissions().contains(&fullscreen));
}

#[test]
fn drain_coalesced_focus_edges_commands_and_appearance() {
    use crate::platform::windows::dome::events::FocusTarget as SceneFocus;
    let mut env = TestEnv::new();
    let window = env.open();
    env.settle(10);
    env.advance_time(std::time::Duration::from_millis(100));
    let mut scene = env.latest_scene();
    scene.focus_request = None;
    env.send_ui(HubMessage::Scene(scene.clone()));
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    let appearance = crate::config::Appearance {
        theme: crate::theme::Flavor::Latte,
        font: crate::font::FontConfig {
            size: 17.0,
            family: Some("Test font".into()),
        },
    };
    env.send_ui(HubMessage::AppearanceChanged(appearance.clone()));
    env.send_ui(HubMessage::Command(WindowCommand::Unminimize(window)));
    let newest_frame = PixelRect::new(100, 150, 700, 500);
    let WindowPlacement::Tiling { placement, .. } =
        &mut scene.windows.get_mut(&window).unwrap().placement
    else {
        panic!("the test starts with a tiled window");
    };
    placement.content_box = newest_frame;
    scene.monitors[0].tiling_windows[0].content_box = newest_frame;
    scene.focus_request = Some(SceneFocus::Window(window));
    env.send_ui(HubMessage::Scene(scene));
    env.send_ui(HubMessage::Command(WindowCommand::Close(window)));
    let frames = env.frame_submissions().len();
    env.clear_trace();
    env.deliver_ready_wakes();
    assert_eq!(
        &env.frame_submissions()[frames..],
        &[(window, newest_frame)]
    );
    assert_eq!(env.close_submissions(), vec![window, window]);
    let commands: Vec<_> = env
        .trace()
        .into_iter()
        .filter(|step| matches!(*step, "close" | "restore"))
        .collect();
    assert_eq!(commands, vec!["close", "restore", "close"]);
    let painted = env
        .window_appearance()
        .expect("appearance survives scene coalescing");
    assert_eq!(painted.theme, appearance.theme);
    assert_eq!(painted.font, appearance.font);
    assert_eq!(env.focus_submissions(), vec![window]);
    env.deliver_due();
    assert_eq!(env.focus_submissions(), vec![window, window]);
}

#[test]
fn produce_unchanged_display_refresh_and_retain_facts_on_error_or_empty() {
    let mut monitor = default_monitor();
    monitor.full_rect = PixelRect::new(-20, -30, 2000, 1200);
    monitor.dots_per_inch = 144;
    let mut env = TestEnv::builder().monitors(vec![monitor.clone()]).build();
    let window = env.open();
    let before = env.display_refreshes();
    let limits = env.limits_demand().len();
    env.change_monitors(vec![monitor.clone()]);
    assert_eq!(env.display_refreshes(), before + 1);
    assert_eq!(&env.limits_demand()[limits..], &[window]);
    assert_eq!(env.displayed_monitors()[0].full_rect, monitor.full_rect);
    assert_eq!(env.displayed_monitors()[0].work_area, monitor.work_area);
    assert_eq!(env.displayed_monitors()[0].dots_per_inch, 144);
    env.fail_display_refresh();
    env.change_monitors(Vec::new());
    assert_eq!(env.display_refreshes(), before + 1);
    assert_eq!(env.displayed_monitors()[0].full_rect, monitor.full_rect);
    assert_eq!(&env.limits_demand()[limits..], &[window]);
    env.enter_exclusive_fullscreen(window);
    let limits = env.limits_demand().len();
    env.change_monitors(vec![monitor]);
    assert_eq!(env.display_refreshes(), before + 2);
    assert_eq!(env.limits_demand().len(), limits);
}

#[test]
fn report_delayed_float_after_premise_change_does_not_mutate() {
    for invalidation in ["destroy", "minimize", "retile", "workspace", "monitor"] {
        let mut env = TestEnv::builder()
            .monitors(vec![default_monitor(), second_monitor()])
            .build();
        let window = env.open();
        env.run_actions("toggle float");
        env.settle(10);
        env.hold_reports(true);
        env.report_move(window, PixelRect::new(2000, 200, 600, 400), 2, env.now());
        let reports = env.take_reports();
        assert!(matches!(reports.as_slice(), [HubEvent::FloatMoved { .. }]));
        env.hold_reports(false);
        match invalidation {
            "destroy" => {
                env.destroy_window(window);
                env.deliver_report(HubEvent::WindowGone(window));
                env.deliver_report(HubEvent::TitleChanged {
                    key: window,
                    title: "late title".into(),
                });
                env.deliver_report(HubEvent::WindowMinimized(window));
                env.deliver_report(HubEvent::WindowRestored(window));
                env.deliver_report(HubEvent::SizeConstraint {
                    key: window,
                    limits: crate::core::LimitObservation::default(),
                });
                env.deliver_report(HubEvent::FocusReported(window));
            }
            "minimize" => env.minimize_window(window),
            "retile" => env.run_actions("toggle float"),
            "workspace" => env.run_actions("focus workspace 1"),
            "monitor" => env.remove_monitor(2),
            _ => unreachable!(),
        }
        let before = env.frame_submissions().len();
        for report in reports {
            env.deliver_report(report);
        }
        assert_eq!(env.frame_submissions().len(), before, "{invalidation}");
        if invalidation == "destroy" {
            assert!(!env.is_managed(window));
        }
    }
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("toggle float");
    env.hold_reports(true);
    let frame = PixelRect::new(250, 200, 550, 350);
    env.report_move(window, frame, 1, env.now());
    let reports = env.take_reports();
    env.hold_reports(false);
    let before = env.frame_submissions().len();
    for report in reports {
        env.deliver_report(report);
    }
    assert_eq!(&env.frame_submissions()[before..], &[(window, frame)]);
}

#[test]
fn report_native_fullscreen_exit_when_dome_assigned_keeps_fullscreen() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("toggle fullscreen");
    let frames = env.frame_submissions().len();
    env.deliver_report(HubEvent::FullscreenChanged {
        key: window,
        fullscreen: None,
    });
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Fullscreen { .. }
    ));
    assert_eq!(env.frame_submissions().len(), frames);
    env.run_actions("toggle fullscreen");
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Tiling { .. }
    ));
}

#[test]
fn report_delayed_fullscreen_after_destroy_or_minimize_does_not_mutate() {
    for destroyed in [false, true] {
        let mut env = TestEnv::new();
        let window = env.open();
        env.hold_reports(true);
        env.report_move(
            window,
            PixelRect::from_dimension(fullscreen_dim()),
            1,
            env.now(),
        );
        let reports = env.take_reports();
        assert!(matches!(
            reports.as_slice(),
            [HubEvent::FullscreenChanged {
                fullscreen: Some(_),
                ..
            }]
        ));
        env.hold_reports(false);
        if destroyed {
            env.destroy_window(window);
        } else {
            env.minimize_window(window);
        }
        let frames = env.frame_submissions().len();
        for report in reports {
            env.deliver_report(report);
        }
        assert_eq!(env.frame_submissions().len(), frames);
        if destroyed {
            assert!(!env.latest_scene().windows.contains_key(&window));
        } else {
            assert!(matches!(
                env.latest_scene().windows[&window].placement,
                WindowPlacement::Minimized
            ));
        }
    }
}

#[test]
fn deliver_fullscreen_entry_after_retile_uses_current_attached_workspace() {
    let mut env = TestEnv::new();
    let window = env.open();
    env.run_actions("toggle float");
    env.hold_reports(true);
    env.report_move(
        window,
        PixelRect::from_dimension(fullscreen_dim()),
        1,
        env.now(),
    );
    let reports = env.take_reports();
    assert!(matches!(
        reports.as_slice(),
        [HubEvent::FullscreenChanged {
            fullscreen: Some(_),
            ..
        }]
    ));
    env.hold_reports(false);
    env.run_actions("toggle float");
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Tiling { .. }
    ));
    for report in reports {
        env.deliver_report(report);
    }
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Fullscreen { .. }
    ));
    env.deliver_report(HubEvent::FullscreenChanged {
        key: window,
        fullscreen: None,
    });
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Tiling { .. }
    ));
    let frames = env.frame_submissions().len();
    env.deliver_report(HubEvent::FullscreenChanged {
        key: window,
        fullscreen: None,
    });
    assert!(matches!(
        env.latest_scene().windows[&window].placement,
        WindowPlacement::Tiling { .. }
    ));
    assert_eq!(env.frame_submissions().len(), frames);
}

#[test]
fn deliver_fullscreen_after_monitor_removal_preserves_parked_workspace_until_replug() {
    for entering in [false, true] {
        let mut env = TestEnv::builder()
            .monitors(vec![default_monitor(), second_monitor()])
            .build();
        env.run_actions("focus monitor right");
        let ordinary = PixelRect::new(2100, 200, 600, 400);
        let fullscreen = second_monitor().work_area;
        let window = env
            .window()
            .spawned_at(if entering { ordinary } else { fullscreen }.to_dimension())
            .open();
        assert_eq!(
            matches!(
                env.latest_scene().windows[&window].placement,
                WindowPlacement::Fullscreen { .. }
            ),
            !entering
        );
        env.hold_reports(true);
        env.report_move(
            window,
            if entering { fullscreen } else { ordinary },
            2,
            env.now(),
        );
        let reports = env.take_reports();
        assert!(
            matches!(reports.as_slice(), [HubEvent::FullscreenChanged { fullscreen, .. }] if fullscreen.is_some() == entering)
        );
        env.hold_reports(false);
        env.remove_monitor(2);
        assert!(matches!(
            env.latest_scene().windows[&window].placement,
            WindowPlacement::Hidden
        ));
        let frames = env.frame_submissions().len();
        for report in reports {
            env.deliver_report(report);
        }
        assert!(matches!(
            env.latest_scene().windows[&window].placement,
            WindowPlacement::Hidden
        ));
        assert_eq!(env.frame_submissions().len(), frames);
        env.add_monitor(second_monitor());
        let scene = env.latest_scene();
        let placement = &scene.windows[&window].placement;
        let monitor = match placement {
            WindowPlacement::Fullscreen { monitor } if entering => *monitor,
            WindowPlacement::Tiling { monitor, .. } if !entering => *monitor,
            _ => panic!("unexpected replug placement"),
        };
        assert_eq!(
            scene
                .monitors
                .iter()
                .find(|candidate| candidate.id == monitor)
                .unwrap()
                .native,
            2
        );
    }
}
