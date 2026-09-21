use super::*;
use crate::core::{PartitionTreeConfig, WindowMatcher};

/// Count minimized windows tracked by the daemon by parsing the same JSON
/// blob external launchers consume via `Query::MinimizedWindows`.
fn minimized_json_len(dome: &Dome) -> usize {
    let json = dome.query_minimized_windows_json();
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(&json).expect("query_minimized_windows_json is well-formed");
    arr.len()
}

#[test]
fn window_destroyed_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.destroy_window(w1);

    assert!(!env.is_offscreen(w2));
    env.assert_horizontally_tiled(&[env.dim(w2)]);
}

#[test]
fn window_minimized_removes_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.minimize_window(w2);

    env.assert_horizontally_tiled(&[env.dim(w1)]);
    // w2 stays tracked as a minimized window (not deleted), reachable
    // via the external launcher query surface.
    assert_eq!(minimized_json_len(&env.dome), 1);
}

#[test]
fn user_minimize_then_restore() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.minimize_window(w2);
    assert_eq!(minimized_json_len(&env.dome), 1);

    env.unminimize_window(w2);
    assert_eq!(minimized_json_len(&env.dome), 0);
    env.assert_horizontally_tiled(&[env.dim(w1), env.dim(w2)]);
}

#[test]
fn move_size_suppresses_placement() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    let placed = env.dim(w1);

    env.dome.move_size_started(w1);

    // Add a second window -- triggers relayout, but w1 should be skipped
    let w2 = env.open();

    assert_eq!(env.dim(w1), placed);

    env.dome.clear_move_state(w1);
    env.layout();

    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
}

#[test]
fn monitors_changed_updates_layout() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    let before = env.dim(w1);

    let new_monitor = MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        gdi_device: "\\\\.\\DISPLAY1".to_string(),
        work_area: PixelRect::new(0, 0, 1280, 720),
        bounds: Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(1280.0),
            Length::new(720.0),
        ),
        is_primary: true,
        scale: 1.0,
    };
    env.dome.monitors_changed(vec![new_monitor]);
    env.layout();

    let after = env.dim(w1);
    assert!(
        after.width < before.width,
        "window should be narrower after monitor shrink"
    );
    assert!(
        after.height < before.height,
        "window should be shorter after monitor shrink"
    );
}

#[test]
fn parked_monitor_windows_hide_on_unplug() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w = env.open();
    env.run_actions("move monitor right");
    assert!(
        !env.is_offscreen(w),
        "window should be visible on the second monitor before unplug"
    );

    env.remove_monitor(second_monitor().handle);

    assert!(
        env.is_offscreen(w),
        "parked workspace's window rides the hide diff after unplug, no workspace switch"
    );
}

#[test]
fn parked_monitor_windows_unhide_on_visit() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w = env.open();
    env.run_actions("move monitor right");
    assert!(
        !env.is_offscreen(w),
        "window should be visible on the second monitor before unplug"
    );

    env.remove_monitor(second_monitor().handle);
    assert!(
        env.is_offscreen(w),
        "parked workspace's window rides the hide diff after unplug"
    );

    // Visiting the parked workspace by its name plus origin monitor points the
    // primary's active workspace at it, so the window surfaces on the primary
    // with no reattach to a monitor.
    env.run_actions("focus workspace 0 --monitor External");
    assert!(
        !env.is_offscreen(w),
        "visiting the parked workspace surfaces its window on the primary"
    );
}

#[test]
fn unmanageable_window_is_ignored() {
    let mut env = TestEnv::new();
    let w1 = env.window().manageable(false).open();

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn ignored_window_rule_prevents_insertion() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.ignore.push(WindowMatcher {
                process: Some("bloat.exe".to_string()),
                ..Default::default()
            })
        })
        .build();

    let w1 = env.window().process("bloat.exe").open();

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn ignored_window_rule_by_class_prevents_insertion() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.ignore.push(WindowMatcher {
                class: Some("Shell_TrayWnd".to_string()),
                ..Default::default()
            })
        })
        .build();

    let w1 = env.window().class("Shell_TrayWnd").open();

    assert_eq!(env.dim(w1), SPAWN_DIM);
}

#[test]
fn title_change_reaches_the_painted_tab_strip() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let _w2 = env.open();
    env.run_actions("toggle layout");

    env.dome
        .update_titles(vec![(w1, Some("Renamed".to_string()))]);

    assert!(
        tabbed_container(&env)
            .titles
            .contains(&"Renamed".to_string()),
        "a title change must reach the tab strip the newest scene paints"
    );
}

#[test]
fn delete_currently_displayed_window() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.destroy_window(w1);

    assert!(!env.is_offscreen(w2));
    env.assert_horizontally_tiled(&[env.dim(w2)]);

    // Second apply_layout proves displayed state was cleaned up
    env.layout();
    assert!(!env.is_offscreen(w2));
}

#[test]
fn destroy_last_window_focuses_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    env.destroy_window(w1);
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn destroy_one_of_two_windows_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.destroy_window(w2);
    assert_eq!(env.focus_target(), FocusTarget::Window(w1));
}

#[test]
fn workspace_switch_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open();

    env.run_actions("focus workspace 1");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn workspace_switch_back_does_not_focus_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");
    assert_eq!(env.focus_target(), FocusTarget::Window(w1));
}

#[test]
fn focus_parent_focuses_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();

    env.run_actions("focus parent");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn focus_child_after_parent_does_not_focus_overlay() {
    let mut env = TestEnv::builder()
        .tiling(|tiling| {
            tiling.partition_tree = PartitionTreeConfig {
                automatic_tiling: false,
                tab_bar_height: Pixels::new(24),
            }
        })
        .build();

    let _w1 = env.open();
    let _w2 = env.open();
    env.run_actions("toggle spawn");
    let _w3 = env.open();

    env.run_actions("focus parent");
    env.run_actions("focus left");
    assert!(
        matches!(env.focus_target(), FocusTarget::Window(_)),
        "after focus left from container, a window must be the focus target, got {:?}",
        env.focus_target()
    );
}

#[test]
fn monitor_switch_empty_to_empty_focuses_overlay() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    env.run_actions("focus workspace 1");

    env.run_actions("focus monitor right");
    assert_eq!(env.focus_target(), FocusTarget::Overlay);
}

#[test]
fn programmatic_echo_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    // Simulate OS echoing LOCATIONCHANGE for windows we just placed.
    // Both enter MoveKind::Programmatic.
    assert!(env.dome.location_changed(w1));
    assert!(env.dome.location_changed(w2));

    env.layout();

    // An echo round-trip must not blink the borders off.
    assert_eq!(env.painted_windows(0).len(), 2);
}

#[test]
fn user_drag_keeps_tiling_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let _w2 = env.open();

    let placed_w1 = env.dim(w1);

    env.dome.move_size_started(w1);
    env.layout();

    assert_eq!(env.dim(w1), placed_w1);
    // w2's border must survive the drag.
    assert_eq!(env.painted_windows(0).len(), 2);
}

#[test]
fn empty_monitor_clears_tiling_overlay() {
    let mut env = TestEnv::new();
    // No windows added. The primary monitor's tiling overlay exists from Dome::new.
    env.layout();

    assert!(env.painted_windows(0).is_empty());
    assert!(env.painted_containers(0).is_empty());
}

#[test]
fn dpi_reconcile_with_unchanged_scale_does_not_move_windows() {
    let mut env = TestEnv::new();
    let w = env.open();
    let before = env.dim(w);

    env.dome.handle_dpi_change();
    env.layout();

    let after = env.dim(w);
    assert_eq!(after.x, before.x);
    assert_eq!(after.y, before.y);
    assert_eq!(after.width, before.width);
    assert_eq!(after.height, before.height);
}

#[test]
fn dpi_change_then_apply_layout_places_at_new_scale() {
    let mut env = TestEnv::new();
    let w = env.open();

    let before = env.dim(w);
    assert!(before.width > Length::new(0.0));

    env.set_monitor_scale(default_monitor().handle, 1.5);
    env.dome.handle_dpi_change();
    env.layout();

    let after = env.dim(w);
    // Frames are physical pixels: a DPI change scales the border but not the work area.
    let border = env.border();
    let expected_x = before.x * 1.5;
    let expected_y = before.y * 1.5;
    let expected_w = before.width - border;
    let expected_h = before.height - border;

    assert_eq!(after.x, expected_x);
    assert_eq!(after.y, expected_y);
    assert_eq!(after.width, expected_w);
    assert_eq!(after.height, expected_h);
}

#[test]
fn handle_dpi_change_on_secondary_monitor_updates_secondary_only() {
    let mut second = second_monitor();
    second.scale = 1.0;
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second])
        .build();

    let w_a = env.open();
    let before_a = env.dim(w_a);

    env.run_actions("focus monitor right");
    let w_b = env.open();
    let before_b = env.dim(w_b);

    env.set_monitor_scale(second_monitor().handle, 2.0);
    env.dome.handle_dpi_change();
    env.layout();

    let after_a = env.dim(w_a);
    assert_eq!(after_a.x, before_a.x);
    assert_eq!(after_a.y, before_a.y);
    assert_eq!(after_a.width, before_a.width);
    assert_eq!(after_a.height, before_a.height);

    // Frames are physical pixels: a DPI change scales the border but not the work area.
    let after_b = env.dim(w_b);
    let border = env.border();
    let expected_x = before_b.x + border;
    let expected_y = before_b.y + border;
    let expected_w = before_b.width - border * 2.0;
    let expected_h = before_b.height - border * 2.0;
    assert!(
        (after_b.x - expected_x).abs() < Length::new(2.0),
        "x: expected ~{expected_x}, got {}",
        after_b.x
    );
    assert!(
        (after_b.y - expected_y).abs() < Length::new(2.0),
        "y: expected ~{expected_y}, got {}",
        after_b.y
    );
    assert!(
        (after_b.width - expected_w).abs() < Length::new(2.0),
        "w: expected ~{expected_w}, got {}",
        after_b.width
    );
    assert!(
        (after_b.height - expected_h).abs() < Length::new(2.0),
        "h: expected ~{expected_h}, got {}",
        after_b.height
    );
}

#[test]
fn tab_bar_lifecycle_per_container() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();

    env.run_actions("toggle layout");

    {
        let tabbed: Vec<_> = env
            .painted_containers(0)
            .into_iter()
            .filter(|c| c.is_tabbed)
            .collect();
        assert_eq!(tabbed.len(), 1);
        assert_eq!(tabbed[0].titles.len(), 2);
        assert!(tabbed[0].active_tab_index < 2);
    }

    env.run_actions("toggle layout");
    assert!(env.painted_containers(0).iter().all(|c| !c.is_tabbed));
}

fn tabbed_container(env: &TestEnv) -> ContainerPlacement {
    env.painted_containers(0)
        .into_iter()
        .find(|c| c.is_tabbed)
        .expect("a tabbed container is painted")
}

#[test]
fn tab_click_focuses_tab_index() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();

    env.run_actions("toggle layout");

    let container = tabbed_container(&env);
    assert_eq!(container.active_tab_index, 1);

    env.dome.tab_clicked(container.id, 0);

    assert_eq!(tabbed_container(&env).active_tab_index, 0);
}

#[test]
fn primary_monitor_answers_to_its_display_name() {
    let env = TestEnv::new();

    let monitors = env.dome.query_monitors_json();
    assert!(monitors.contains("\"unique_name\":\"Test\""), "{monitors}");

    let workspaces = env.dome.query_workspaces_json();
    assert!(workspaces.contains("\"monitor\":\"Test\""), "{workspaces}");
    assert!(!workspaces.contains("primary"), "{workspaces}");
}

#[test]
fn primary_change_to_a_new_display_carries_the_workspaces() {
    let mut env = TestEnv::new();

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    env.change_monitors(vec![demoted, promoted]);

    let workspaces = env.dome.query_workspaces_json();
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}

#[test]
fn primary_change_to_a_tracked_display_parks_the_displaced_workspaces() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    env.change_monitors(vec![demoted, promoted]);

    let workspaces = env.dome.query_workspaces_json();
    assert!(workspaces.contains("\"state\":\"parked\""), "{workspaces}");
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}

#[test]
fn border_size_changed_resize_managed_windows() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    let w3 = env.open();
    env.run_actions("toggle float");

    let prev_d1 = env.dim(w1);
    let prev_d2 = env.dim(w2);
    let prev_d3 = env.dim(w3);
    env.change_config(|config| {
        config.tiling.border_size = config.tiling.border_size + Pixels::new(2)
    });

    let d1 = env.dim(w1);
    let d2 = env.dim(w2);
    let d3 = env.dim(w3);
    assert_eq!(d1.width, prev_d1.width - Length::new(4.0));
    assert_eq!(d1.height, prev_d1.height - Length::new(4.0));
    assert_eq!(d2.width, prev_d2.width - Length::new(4.0));
    assert_eq!(d2.height, prev_d2.height - Length::new(4.0));
    assert_eq!(d3.width, prev_d3.width - Length::new(4.0));
    assert_eq!(d3.height, prev_d3.height - Length::new(4.0));
}

#[test]
fn config_reload_dispatches_apply_theme_on_flavor_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();
    env.run_actions("toggle float");
    let _w3 = env.open();

    let configured = baseline_config().appearance.theme;
    assert!(
        env.window_appearance().is_none(),
        "no appearance message before the reload"
    );

    assert_ne!(crate::theme::Flavor::Latte, configured);
    env.change_config(|config| config.appearance.theme = crate::theme::Flavor::Latte);

    assert_eq!(
        env.window_appearance()
            .expect("the reload dispatched an appearance")
            .theme,
        crate::theme::Flavor::Latte
    );
}
