use super::*;

#[test]
fn is_moving_suppresses_placement() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[cg1]);
    macos.settle(&mut dome, 10);

    let full_frame = macos.window_frame(cg1);

    start_drag(&mut dome, 100);
    macos.set_window_frame(cg1, (500, 300, 400, 400));

    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.add_windows(&macos, &[cg2]);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg1), (500, 300, 400, 400));

    end_drag(&mut dome, &macos, 100, cg1, 500, 300, 400, 400);
    macos.settle(&mut dome, 10);
    let new_frame = macos.window_frame(cg1);
    assert_ne!(new_frame, (500, 300, 400, 400));
    assert_ne!(new_frame, full_frame);
}

#[test]
fn monitor_change_rehides_offscreen_windows() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[cg1]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    let offscreen_before = macos.window_frame(cg1);
    assert!(macos.is_offscreen(cg1));

    dome.monitors_changed(vec![default_monitor(), second_monitor()]);
    macos.settle(&mut dome, 10);

    assert!(macos.is_offscreen(cg1));
    let offscreen_after = macos.window_frame(cg1);
    assert_ne!(
        offscreen_before, offscreen_after,
        "offscreen position should change with new monitor"
    );
}

#[test]
fn remove_borderless_fullscreen_window_restores_siblings() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.add_windows(&macos, &[cg1, cg2]);
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg1, 0, 0, 1920, 1080);
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));

    dome.remove_windows(&[cg1]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
}

#[test]
fn app_terminated_removes_windows() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Tab 1");
    let cg2 = macos.spawn_window(100, "Safari", "Tab 2");
    let cg3 = macos.spawn_window(101, "Terminal", "zsh");
    dome.add_windows(&macos, &[cg1, cg2, cg3]);
    macos.settle(&mut dome, 10);

    dome.app_terminated(100);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg3));
    assert_eq!(macos.window_frame(cg3), (4, 4, 1912, 1072));
}

#[test]
fn window_removed_fills_screen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.add_windows(&macos, &[cg1, cg2]);
    macos.settle(&mut dome, 10);

    dome.remove_windows(&[cg1]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
}

#[test]
fn render_frame_focused_none_after_last_window_removed() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let mut dome = macos.setup_dome();
    dome.add_windows(&macos, &[cg1]);
    dome.remove_windows(&[cg1]);

    let state = macos.last_scene_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_container_after_focus_parent() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let cg2 = macos.spawn_window(1, "App2", "Win2");
    let mut dome = macos.setup_dome();
    dome.add_windows(&macos, &[cg1, cg2]);
    send(&mut dome, "focus parent");

    let state = macos.last_scene_state();
    assert!(
        state.focused_window.is_none(),
        "the highlighted container holds focus, so no window does"
    );
}

#[test]
fn render_frame_focused_none_on_empty_workspace() {
    let mut macos = MacOS::new();
    let cg1 = macos.spawn_window(1, "App1", "Win1");
    let mut dome = macos.setup_dome();
    dome.add_windows(&macos, &[cg1]);
    send(&mut dome, "focus workspace 1");

    let state = macos.last_scene_state();
    assert_eq!(state.focused_window, None);
}

#[test]
fn render_frame_focused_monitor_changes_on_focus_monitor() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    let before = macos.last_scene_state();
    send(&mut dome, "focus monitor right");
    let after = macos.last_scene_state();

    assert_ne!(before.focused_monitor_id, after.focused_monitor_id);
}

#[test]
fn reconcile_keeps_and_refreshes_when_cg_id_present() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[cg1]);
    macos.settle(&mut dome, 10);

    let focused_before = macos.last_scene_state().focused_window;
    let old_handle = macos.refresh_window(&mut dome, cg1);
    let old_writes = old_handle.placement_count();
    let replacement_writes = macos.placement_count(cg1);

    assert_eq!(macos.last_scene_state().focused_window, focused_before);
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert_eq!(old_handle.placement_count(), old_writes);
    assert!(macos.placement_count(cg1) > replacement_writes);
    assert!(macos.is_offscreen(cg1));
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.last_scene_state().focused_window, focused_before);
    assert_eq!(old_handle.placement_count(), old_writes);
}

#[test]
fn reconcile_removes_when_cg_id_absent_and_no_fullscreen() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[cg1]);
    macos.settle(&mut dome, 10);

    // Window is genuinely gone. is_valid is false, it is absent from
    // app.windows(), and there is no fullscreen window on this PID.
    macos.set_valid(cg1, false);
    dome.remove_windows(&[cg1]);

    assert!(macos.last_scene_state().tiling_corner_radii.is_empty());
    let writes = macos.placement_count(cg1);
    send(&mut dome, "focus workspace 1");
    send(&mut dome, "focus workspace 0");
    assert_eq!(macos.placement_count(cg1), writes);
    assert!(macos.last_scene_state().focused_window.is_none());
}

#[test]
fn reconcile_removing_one_process_window_keeps_other_process_fullscreen_window() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let mut process_ids = (1..=3).filter(|&process_id| process_id as u32 != std::process::id());
    let process_id = process_ids.next().unwrap();
    let fullscreen_process_id = process_ids.next().unwrap();

    let cg1 = macos.spawn_window(process_id, "Safari", "Google");
    let cg2 = macos.spawn_window(fullscreen_process_id, "Terminal", "zsh");
    dome.add_windows(&macos, &[cg1, cg2]);
    macos.settle(&mut dome, 10);
    let removed_id = dome.window_id_for(cg1);
    let fullscreen_id = dome.window_id_for(cg2);
    let scene = macos.last_scene_state();
    assert!(scene.tiling_corner_radii.contains_key(&removed_id));
    assert!(scene.tiling_corner_radii.contains_key(&fullscreen_id));

    macos.enter_native_fullscreen(&mut dome, cg2);
    dome.remove_windows(&[cg1]);
    assert!(
        !macos
            .last_scene_state()
            .tiling_corner_radii
            .contains_key(&removed_id)
    );
    let removed_writes = macos.placement_count(cg1);

    macos.exit_native_fullscreen(&mut dome, cg2, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
    assert!(
        macos
            .last_scene_state()
            .tiling_corner_radii
            .contains_key(&fullscreen_id)
    );
    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg2));
    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(cg2), (4, 4, 1912, 1072));
    assert_eq!(macos.placement_count(cg1), removed_writes);
}

#[test]
fn parked_monitor_windows_hide_on_unplug() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    send(&mut dome, "focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[win]);
    macos.settle(&mut dome, 10);
    assert_eq!(
        macos.window_frame(win),
        (1924, 4, 2552, 1432),
        "window should be tiled on the second monitor before unplug"
    );

    dome.monitors_changed(vec![default_monitor()]);
    macos.settle(&mut dome, 10);

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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    send(&mut dome, "focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[win]);
    macos.settle(&mut dome, 10);

    dome.monitors_changed(vec![default_monitor()]);
    macos.settle(&mut dome, 10);
    let (x, y, _, _) = macos.window_frame(win);
    assert_eq!((x, y), (1919, 1079), "window parks offscreen on unplug");

    // Visiting by name plus origin points the primary's active workspace at the parked
    // workspace, so the window surfaces without reattaching to a monitor.
    send(&mut dome, "focus workspace 0 --monitor External");
    macos.settle(&mut dome, 10);

    assert_eq!(
        macos.window_frame(win),
        (4, 4, 1912, 1072),
        "visiting the parked workspace tiles its window on the primary"
    );
}

#[test]
fn primary_monitor_answers_to_its_display_name() {
    let mut macos = MacOS::new();
    let dome = macos.setup_dome();

    let monitors = dome.query_monitors_json();
    assert!(monitors.contains("\"unique_name\":\"Test\""), "{monitors}");

    let workspaces = dome.query_workspaces_json();
    assert!(workspaces.contains("\"monitor\":\"Test\""), "{workspaces}");
    assert!(!workspaces.contains("primary"), "{workspaces}");
}

#[test]
fn primary_change_to_a_new_display_carries_the_workspaces() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[cg]);
    macos.settle(&mut dome, 10);

    let mut incoming = default_monitor();
    incoming.display_id = 3;
    incoming.name = "Studio".to_string();
    dome.monitors_changed(vec![incoming]);
    macos.settle(&mut dome, 10);

    let workspaces = dome.query_workspaces_json();
    assert!(
        workspaces.contains("\"monitor\":\"Studio\""),
        "{workspaces}"
    );
    assert!(!workspaces.contains("parked"), "{workspaces}");
}

#[test]
fn primary_change_to_a_tracked_display_parks_the_displaced_workspaces() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    dome.monitors_changed(vec![default_monitor(), second_monitor()]);
    send(&mut dome, "focus monitor right");
    let win = macos.spawn_window(100, "Safari", "Google");
    dome.add_windows(&macos, &[win]);
    macos.settle(&mut dome, 10);

    let mut demoted = default_monitor();
    demoted.is_primary = false;
    let mut promoted = second_monitor();
    promoted.is_primary = true;
    dome.monitors_changed(vec![demoted, promoted]);
    macos.settle(&mut dome, 10);

    let workspaces = dome.query_workspaces_json();
    assert!(workspaces.contains("\"state\":\"parked\""), "{workspaces}");
    assert!(
        workspaces.contains("\"monitor\":\"External\""),
        "{workspaces}"
    );
}
