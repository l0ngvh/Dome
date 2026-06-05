use super::*;

/// Asserts that every hwnd in `tiling` appears above the overlay in the
/// normal z-order band. Windows below the overlay (e.g. hidden via
/// `move_offscreen` -> `HWND_BOTTOM`) are acceptable and mirror real
/// Win32 behavior.
fn assert_tiling_above_overlay(env: &TestEnv, tiling: &[HwndId]) {
    let stack = env.tiling_z_order();
    let overlay = env.overlay_id();
    let overlay_pos = stack
        .iter()
        .position(|&id| id == overlay)
        .expect("overlay must be in normal band");
    for &hwnd in tiling {
        let pos = stack
            .iter()
            .position(|&id| id == hwnd)
            .unwrap_or_else(|| panic!("{hwnd:?} not found in normal band"));
        assert!(
            pos < overlay_pos,
            "{hwnd:?} at index {pos} is not above overlay at index {overlay_pos}"
        );
    }
}

#[test]
fn single_window_above_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    assert_eq!(
        env.tiling_z_order(),
        vec![w1, FOCUS_SINK_ID, env.overlay_id()]
    );
}

#[test]
fn focused_window_on_top() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    assert_tiling_above_overlay(&env, &[w1, w2, w3]);
}

#[test]
fn all_tiling_above_overlay() {
    let mut env = TestEnv::new();
    for i in 1..=5 {
        env.open(i, "App", "app.exe", SPAWN_DIM);
    }

    let stack = env.tiling_z_order();
    assert_eq!(stack.len(), 7); // 5 windows + sink + overlay
    assert_eq!(*stack.last().unwrap(), env.overlay_id());
}

#[test]
fn focus_change_preserves_overlay_behind() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.focus_window(w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());

    env.focus_window(w2);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());

    env.focus_window(w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());
}

#[test]
fn destroy_window_rebuilds_chain() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    env.destroy_window(w2);

    assert_eq!(
        env.tiling_z_order(),
        vec![w3, w1, FOCUS_SINK_ID, env.overlay_id()]
    );
}

#[test]
fn destroy_focused_rebuilds_chain() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    env.destroy_window(w3);

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn add_window_to_existing() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    assert_tiling_above_overlay(&env, &[w1, w2, w3]);
}

#[test]
fn workspace_switch_restores_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn empty_workspace_overlay_focus() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");

    assert_eq!(env.focus_target(), FocusTarget::Sink);
}

#[test]
fn float_window_above_tiling_and_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    // w3 is focused. Float it.
    env.run_actions("toggle float");

    // w3 should be in the topmost band (first in full z_order)
    let full = env.z_order();
    assert_eq!(full[0], w3);

    // Normal band should have w1, w2, and overlay, with overlay last
    let normal = env.tiling_z_order();
    assert!(!normal.contains(&w3));
    assert_eq!(*normal.last().unwrap(), env.overlay_id());
}

#[test]
fn unfloat_window_rejoins_tiling_chain() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle float");
    env.run_actions("toggle float");

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn stable_positions_still_update_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    assert_tiling_above_overlay(&env, &[w1, w2]);

    // Second apply_layout: positions unchanged, z-order should remain correct
    env.dome.apply_layout();

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn move_window_to_other_workspace() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    // w3 is focused. Move it to workspace 1.
    env.run_actions("move workspace 1");

    assert!(env.is_offscreen(w3));

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn overlay_behind_after_empty_workspace_roundtrip() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");

    assert_tiling_above_overlay(&env, &[w1, w2]);
}

#[test]
fn stable_windows_skip_set_position() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.moves.lock().unwrap().clear();
    env.dome.apply_layout();

    assert!(
        env.moves.lock().unwrap().is_empty(),
        "stable tiling windows should not trigger set_position"
    );
}

#[test]
fn monitor_switch_issues_set_position() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.moves.lock().unwrap().clear();
    env.run_actions("move monitor right");

    assert!(
        env.moves.lock().unwrap().iter().any(|(id, ..)| *id == w1),
        "cross-monitor move should trigger set_position"
    );
}

/// New tiling overlay parks at the bottom of the normal band on creation,
/// so the next CreateWindowExW for a managed window naturally sits above it.
#[test]
fn tiling_overlay_seeded_at_bottom() {
    let env = TestEnv::new();
    assert_eq!(env.tiling_z_order(), vec![env.overlay_id()]);
}

/// Float->Tiling emits ZOrder::NotTopmost to clear WS_EX_TOPMOST, then
/// positions the window above the overlay reference. The tiling-above-overlay
/// invariant is restored by the per-window lift in show_tiling.
#[test]
fn unfloat_drops_window_from_topmost_band() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w2 is focused (last added). Float it so it enters the topmost band.
    env.run_actions("toggle float");
    assert!(env.is_topmost(w2), "floated window must be in topmost band");

    // Unfloat w2 back to tiling.
    env.run_actions("toggle float");

    // w2 must have left the topmost band.
    assert!(
        !env.is_topmost(w2),
        "unfloated window must leave the topmost band"
    );
    // Both tiling windows sit in the normal band above the overlay.
    assert_tiling_above_overlay(&env, &[w1, w2]);
}

/// After the initial layout pass seeds all windows above the overlay,
/// a second apply_layout with identical targets must not reorder the z-stack
/// at all (no per-pass overlay shuffle, no managed-window churn).
#[test]
fn steady_state_apply_layout_does_not_touch_overlay_zorder() {
    let mut env = TestEnv::new();
    env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let stack_before = env.z_order();
    env.dome.apply_layout();
    let stack_after = env.z_order();
    assert_eq!(
        stack_after, stack_before,
        "second apply_layout must not reorder the overlay"
    );
}

/// When window_above() returns None (overlay has been promoted to the top
/// with nothing above it), show_tiling's fallback path demotes the overlay
/// below the managed window via demote_below.
#[test]
fn lift_falls_back_when_overlay_at_top() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    // Corrupt overlay to topmost band so window_above() returns None.
    let overlay_id = env.overlay_id();
    env.z_stack.apply(overlay_id, ZOrder::Topmost);
    // Drive a workspace round-trip: switch away parks w1 offscreen, switch
    // back triggers show_tiling's lift on the Offscreen->Tiling transition.
    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");
    let stack = env.z_stack.normal_stack();
    assert_eq!(
        stack.last(),
        Some(&overlay_id),
        "overlay must end up at the bottom of the normal band via demote_below fallback"
    );
    assert!(
        stack.contains(&w1),
        "w1 must be in the normal band above the overlay"
    );
}

/// Parked-offscreen windows must sit strictly below the focus sink so that
/// Win32's close-time focus walk lands on a Dome-owned window rather than
/// reactivating an inactive workspace (see
/// docs/architecture.md, "Virtual workspaces").
#[test]
fn focus_sink_stays_above_parked_windows() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("focus workspace 1");
    assert!(env.is_offscreen(w1));
    assert!(env.is_offscreen(w2));

    let stack = env.z_order();
    let sink_idx = stack
        .iter()
        .position(|&h| h == FOCUS_SINK_ID)
        .expect("focus sink must be in z-stack");
    let w1_idx = stack.iter().position(|&h| h == w1).unwrap();
    let w2_idx = stack.iter().position(|&h| h == w2).unwrap();
    assert!(
        sink_idx < w1_idx,
        "sink at {sink_idx} must sit above parked w1 at {w1_idx}"
    );
    assert!(
        sink_idx < w2_idx,
        "sink at {sink_idx} must sit above parked w2 at {w2_idx}"
    );
}

/// Production positions the float overlay with `ZOrder::After(float_window)`
/// so the overlay sits directly below its float window in the combined
/// z-stack (see docs/architecture.md, "Displaying visual indicators":
/// "float overlays sit inside the topmost band themselves, just below their
/// float").
#[test]
fn float_overlay_sits_directly_below_float_window() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle float");

    let stack = env.z_order();
    let w2_idx = stack.iter().position(|&h| h == w2).unwrap();
    let overlay_idx = stack
        .iter()
        .position(|&h| h == FLOAT_OVERLAY_ID)
        .expect("float overlay must be in z-stack after toggle float");
    assert_eq!(
        overlay_idx,
        w2_idx + 1,
        "float overlay (idx {overlay_idx}) must sit directly below float window w2 (idx {w2_idx})"
    );
}
