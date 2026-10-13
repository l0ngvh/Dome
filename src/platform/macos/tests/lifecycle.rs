use crate::core::ReportedMonitor;

use super::*;

#[test]
fn admit_discovery_in_first_hidden_scene_parks_window_and_reload_without_ignore_rule_admits_window()
{
    use crate::core::WindowMatcher;
    use crate::platform::macos::dome::events::{HubMessage, WindowPlacement};

    let mut env = TestEnv::new();
    env.builder()
        .tiling(|tiling| {
            tiling.ignore = vec![WindowMatcher {
                app: Some("Ignored".into()),
                ..Default::default()
            }];
        })
        .build();
    let hidden = env.spawn_window(100, "App", "Hidden");
    let ignored = env.spawn_window(200, "Ignored", "Ignored");
    env.hold_scenes(true);
    env.queue_discoveries(&[hidden, ignored]);
    env.ui_pass();
    assert!(env.recorded_moves().is_empty());
    env.pump_reports();
    env.pump_scenes();
    env.run_actions("focus workspace 1");
    assert!(!env.scene().windows.contains_key(&ignored));
    assert!(matches!(
        env.scene().windows[&hidden].placement,
        WindowPlacement::Hidden
    ));
    assert!(env.recorded_moves().is_empty());
    let scene = env.take_held_scenes().pop().unwrap();
    env.send_message(HubMessage::Scene(scene));
    env.ui_pass();
    assert_eq!(env.window_frame(hidden), (1919, 1079, 800, 600));
    assert_eq!(env.window_frame(ignored), (0, 0, 800, 600));
    env.hold_scenes(false);
    env.change_config("return { ignore = {} }");
    assert!(env.scene().windows.contains_key(&ignored));
    assert_ne!(env.window_frame(ignored), (0, 0, 800, 600));
    assert_eq!(env.scene().monitors.len(), 1);
    env.change_monitors(vec![default_monitor(), second_monitor()]);
    let scene = env.scene();
    assert_eq!(scene.monitors.len(), 2);
    let empty_monitor = scene
        .monitors
        .iter()
        .find(|monitor| monitor.native == 2)
        .unwrap()
        .id;
    assert!(!scene.windows.values().any(|window| matches!(window.placement, WindowPlacement::Tiling { monitor, .. } | WindowPlacement::Float { monitor, .. } | WindowPlacement::Fullscreen { monitor } if monitor == empty_monitor)));
}

#[test]
fn core_scale_is_one_despite_the_backing_factor() {
    assert_eq!(ReportedMonitor::from(&default_monitor()).scale, 1.0);
}

#[test]
fn discover_window_during_drag_updates_dragged_window_placement() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    let full_frame = macos.window_frame(cg1);

    macos.start_drag(100);
    macos.set_window_frame(cg1, PixelRect::new(500, 300, 400, 400));

    macos.advance(std::time::Duration::from_millis(1));
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg2]);
    let new_frame = macos.window_frame(cg1);
    assert_ne!(new_frame, (500, 300, 400, 400));
    assert_ne!(new_frame, full_frame);
    assert!(macos.recorded_moves().iter().any(|entry| entry.0 == cg1));
    macos.settle(10);
    macos.end_drag(100, cg1, 500, 300, 400, 400);
    macos.settle(10);
    assert_eq!(macos.window_frame(cg1), new_frame);
}

#[test]
fn monitor_change_rehides_offscreen_windows() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    let offscreen_before = macos.window_frame(cg1);
    assert!(macos.is_offscreen(cg1));

    macos.change_monitors(vec![default_monitor(), second_monitor()]);
    macos.settle(10);

    assert!(macos.is_offscreen(cg1));
    let offscreen_after = macos.window_frame(cg1);
    assert_ne!(
        offscreen_before, offscreen_after,
        "offscreen position should change with new monitor"
    );
}

#[test]
fn remove_borderless_fullscreen_window_restores_siblings() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.simulate_external_move(cg1, 0, 0, 1920, 1080);
    macos.settle(10);
    assert!(macos.is_offscreen(cg2));

    macos.remove_windows(&[cg1]);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
}

#[test]
fn app_terminated_removes_windows() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Tab 1");
    let cg2 = macos.spawn_window(100, "Safari", "Tab 2");
    let cg3 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2, cg3]);
    macos.settle(10);

    macos.app_terminated(100);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg3));
    assert_eq!(macos.window_frame(cg3), (4, 4, 1912, 1072));
}

#[test]
fn window_removed_fills_screen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.remove_windows(&[cg1]);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
}

#[test]
fn render_frame_focused_none_after_last_window_removed() {
    let mut macos = TestEnv::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    macos.setup();
    macos.discover_windows(&[cg1]);
    macos.remove_windows(&[cg1]);

    let state = macos.last_scene_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_container_after_focus_parent() {
    let mut macos = TestEnv::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let cg2 = macos.spawn_window(1, "App2", "Win2");
    macos.setup();
    macos.discover_windows(&[cg1, cg2]);
    macos.run_actions("focus parent");

    // After focus_parent, focused_tiling_window() returns None (container highlighted),
    // so the platform receives focused_window: None and focuses the overlay.
    let state = macos.last_scene_state();
    assert!(state.focused_window.is_none());
}

#[test]
fn render_frame_focused_none_on_empty_workspace() {
    let mut macos = TestEnv::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    macos.setup();
    macos.discover_windows(&[cg1]);
    macos.run_actions("focus workspace 1");

    let state = macos.last_scene_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_monitor_changes_on_focus_monitor() {
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), second_monitor()]);

    let before = macos.last_scene_state();
    macos.run_actions("focus monitor right");
    let after = macos.last_scene_state();

    assert_ne!(before.focused_monitor_id, after.focused_monitor_id);
}

#[test]
fn accepted_refresh_preserves_domain_identity() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.refresh_window_and_assert_identity(cg1);
}

#[test]
fn reconcile_removes_when_cg_id_absent_and_no_fullscreen() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    macos.remove_windows(&[cg1]);

    assert!(
        !macos.is_managed(cg1),
        "window must be torn down when cg_id absent and no fullscreen guard"
    );
}

#[test]
fn reconcile_fullscreen_guard_is_per_pid() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(200, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.enter_native_fullscreen(cg2);

    macos.remove_windows(&[cg1]);

    assert!(
        !macos.is_managed(cg1),
        "PID-200's fullscreen must not protect PID-100's window from removal"
    );
    assert!(
        macos.is_managed(cg2),
        "PID-200's fullscreen window must remain tracked"
    );
}

#[test]
fn parked_monitor_windows_hide_on_unplug() {
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), second_monitor()]);

    macos.run_actions("focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);
    assert_eq!(
        macos.window_frame(win),
        (1924, 4, 2552, 1432),
        "window should be tiled on the second monitor before unplug"
    );

    macos.change_monitors(vec![default_monitor()]);
    macos.settle(10);

    // Dome parks a hidden window one pixel inside the furthest monitor's bottom-right
    // corner, which is (1919, 1079) once only the primary remains. Asserting the exact
    // spot separates a parked window from one left behind at the departed monitor's
    // coordinates, where it would still read as offscreen.
    let (x, y, _, _) = macos.window_frame(win);
    assert_eq!(
        (x, y),
        (1919, 1079),
        "parked workspace's window rides the hide diff after unplug, no workspace switch"
    );
}

#[test]
fn parked_monitor_windows_unhide_on_visit() {
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), second_monitor()]);

    macos.run_actions("focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);

    macos.change_monitors(vec![default_monitor()]);
    macos.settle(10);
    let (x, y, _, _) = macos.window_frame(win);
    assert_eq!((x, y), (1919, 1079), "window parks offscreen on unplug");

    // Visiting by name plus origin points the primary's active workspace at the parked
    // workspace, so the window surfaces without reattaching to a monitor.
    macos.run_actions("focus workspace 0 --monitor External");
    macos.settle(10);

    assert_eq!(
        macos.window_frame(win),
        (4, 4, 1912, 1072),
        "visiting the parked workspace tiles its window on the primary"
    );
}

#[test]
fn primary_monitor_answers_to_its_display_name() {
    let mut macos = TestEnv::new();
    macos.setup();

    let monitors = macos.query_monitors_json();
    assert!(monitors.contains("\"unique_name\":\"Test\""), "{monitors}");

    let workspaces = macos.query_workspaces_json();
    assert!(workspaces.contains("\"monitor\":\"Test\""), "{workspaces}");
    assert!(!workspaces.contains("primary"), "{workspaces}");
}

#[test]
fn primary_change_to_a_new_display_carries_the_workspaces() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg]);
    macos.settle(10);

    let mut incoming = default_monitor();
    incoming.display_id = 3;
    incoming.name = "Studio".to_string();
    macos.change_monitors(vec![incoming]);
    macos.settle(10);

    let workspaces = macos.query_workspaces_json();
    assert!(
        workspaces.contains("\"monitor\":\"Studio\""),
        "{workspaces}"
    );
    assert!(!workspaces.contains("parked"), "{workspaces}");
}

#[test]
fn primary_change_to_a_tracked_display_parks_the_displaced_workspaces() {
    let mut macos = TestEnv::new();
    macos.setup();
    macos.change_monitors(vec![default_monitor(), second_monitor()]);
    macos.run_actions("focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    macos.change_monitors(vec![demoted, promoted]);
    macos.settle(10);

    let workspaces = macos.query_workspaces_json();
    assert!(workspaces.contains("\"state\":\"parked\""), "{workspaces}");
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}

#[test]
fn discover_members_waits_for_admission_and_scene_omission_revokes_writes() {
    use crate::platform::macos::dome::events::{HubMessage, WindowCommand, WindowPlacement};

    let mut env = TestEnv::new();
    env.setup();
    let hidden = env.spawn_window(100, "App", "Hidden");
    let minimized = env.spawn_window(200, "App", "Minimized");
    let fullscreen = env.spawn_window(300, "App", "Fullscreen");
    env.queue_discoveries(&[hidden, minimized]);
    env.ui_pass();
    assert!(
        env.recorded_moves().is_empty(),
        "discovery is not admission"
    );
    env.pump_reports();
    env.pump_scenes();
    env.ui_pass();
    env.user_minimize(minimized);
    env.run_actions("focus workspace 1");
    env.discover_native_fullscreen(fullscreen);
    let scene = env.scene();
    assert_eq!(scene.windows.len(), 3);
    assert!(matches!(
        scene.windows[&hidden].placement,
        WindowPlacement::Hidden
    ));
    assert!(matches!(
        scene.windows[&minimized].placement,
        WindowPlacement::Minimized
    ));
    assert!(matches!(
        scene.windows[&fullscreen].placement,
        WindowPlacement::Fullscreen { .. }
    ));
    assert!(env.is_offscreen(hidden));

    let mut omission = scene;
    omission.windows.remove(&hidden);
    env.send_message(HubMessage::Scene(omission));
    env.ui_pass();
    env.clear_effects();
    env.send_message(HubMessage::Command(WindowCommand::Close(hidden)));
    env.send_message(HubMessage::Command(WindowCommand::Unminimize(hidden)));
    env.ui_pass();
    assert!(
        !env.effects()
            .iter()
            .any(|effect| effect == &format!("close {hidden}")
                || effect == &format!("unminimize {hidden}"))
    );
}

#[test]
fn drain_intake_keeps_newest_geometry_all_commands_and_focus_edges() {
    use crate::platform::macos::dome::events::{
        FocusTarget, HubMessage, WindowCommand, WindowPlacement,
    };

    let mut env = TestEnv::new();
    env.setup();
    let key = env.spawn_window(100, "App", "Window");
    env.discover_windows(&[key]);
    env.settle(10);
    env.clear_moves();
    env.clear_effects();
    let mut scene = env.scene();
    scene.focus_request = Some(FocusTarget::Window(key));
    env.send_message(HubMessage::Scene(scene.clone()));
    scene.focus_request = None;
    scene.windows.get_mut(&key).unwrap().placement = WindowPlacement::Hidden;
    env.send_message(HubMessage::Scene(scene.clone()));
    env.send_message(HubMessage::Command(WindowCommand::Close(key)));
    env.send_message(HubMessage::Command(WindowCommand::Close(key)));
    let appearance = crate::config::Appearance {
        theme: crate::theme::Flavor::Mocha,
        font: crate::font::FontConfig {
            size: 14.0,
            family: None,
        },
    };
    env.send_message(HubMessage::AppearanceChanged(appearance));
    env.send_message(HubMessage::Command(WindowCommand::Unminimize(key)));
    scene.focus_request = Some(FocusTarget::Window(key));
    env.send_message(HubMessage::Scene(scene));
    env.ui_pass();
    assert_eq!(env.recorded_moves().len(), 1);
    assert!(env.is_offscreen(key));
    let effects = env.effects();
    let commands: Vec<_> = effects
        .iter()
        .filter(|effect| effect.starts_with("close ") || effect.starts_with("unminimize "))
        .cloned()
        .collect();
    assert_eq!(
        commands,
        [
            format!("close {key}"),
            format!("close {key}"),
            format!("unminimize {key}")
        ]
    );
    assert!(effects.iter().any(|effect| effect == "appearance"));
    assert!(
        effects
            .iter()
            .position(|effect| effect == &format!("park {key}"))
            .unwrap()
            < effects
                .iter()
                .position(|effect| effect == &format!("close {key}"))
                .unwrap()
    );
    env.advance(std::time::Duration::from_millis(100));
    env.release_ready_focus();
    assert!(env.effects().contains(&format!("focus {key}")));
    env.clear_effects();
    env.send_message(HubMessage::Command(WindowCommand::Close(key)));
    env.ui_pass();
    assert_eq!(env.effects(), [format!("close {key}")]);
}
