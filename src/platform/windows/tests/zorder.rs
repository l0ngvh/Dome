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
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());

    assert_eq!(
        env.tiling_z_order(),
        vec![HwndId::test(1), HwndId::test(9999)]
    );
}

#[test]
fn focused_window_on_top() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2), HwndId::test(3)]);
}

#[test]
fn all_tiling_above_overlay() {
    let mut env = TestEnv::new();
    for i in 1..=5 {
        let w = env.spawn_window(i, "App", "app.exe");
        env.add_window(w);
    }

    let stack = env.tiling_z_order();
    assert_eq!(stack.len(), 6); // 5 windows + overlay
    assert_eq!(*stack.last().unwrap(), env.overlay_id());
}

#[test]
fn focus_change_preserves_overlay_behind() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.focus_window(&w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());

    env.focus_window(&w2);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());

    env.focus_window(&w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), env.overlay_id());
}

#[test]
fn destroy_window_rebuilds_chain() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());

    env.destroy_window(&w2);

    assert_eq!(
        env.tiling_z_order(),
        vec![HwndId::test(3), HwndId::test(1), HwndId::test(9999),]
    );
}

#[test]
fn destroy_focused_rebuilds_chain() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());

    env.destroy_window(&w3);

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn add_window_to_existing() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w3.clone());

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2), HwndId::test(3)]);
}

#[test]
fn workspace_switch_restores_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn empty_workspace_overlay_focus() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());

    env.reset_sink_focus();
    env.run_actions("focus workspace 1");

    assert_eq!(env.sink_focus_count(), 1);
}

#[test]
fn float_window_above_tiling_and_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());

    // w3 is focused. Float it.
    env.run_actions("toggle float");

    // w3 should be in the topmost band (first in full z_order)
    let full = env.z_order();
    assert_eq!(full[0], HwndId::test(3));

    // Normal band should have w1, w2, and overlay, with overlay last
    let normal = env.tiling_z_order();
    assert!(!normal.contains(&HwndId::test(3)));
    assert_eq!(*normal.last().unwrap(), env.overlay_id());
}

#[test]
fn unfloat_window_rejoins_tiling_chain() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("toggle float");
    env.run_actions("toggle float");

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn stable_positions_still_update_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);

    // Second apply_layout: positions unchanged, z-order should remain correct
    env.dome.apply_layout();

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn move_window_to_other_workspace() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    let w3 = env.spawn_window(3, "App3", "app3.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    env.add_window(w3.clone());

    // w3 is focused. Move it to workspace 1.
    env.run_actions("move workspace 1");

    assert!(w3.is_offscreen());

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn overlay_behind_after_empty_workspace_roundtrip() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");

    assert_tiling_above_overlay(&env, &[HwndId::test(1), HwndId::test(2)]);
}

#[test]
fn stable_windows_skip_set_position() {
    let mut env = TestEnv::new();
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    let w2 = env.spawn_window(2, "App2", "app2.exe");
    env.add_window(w1.clone());
    env.add_window(w2.clone());

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
    env.add_screen(second_screen());
    let w1 = env.spawn_window(1, "App1", "app1.exe");
    env.add_window(w1.clone());

    env.moves.lock().unwrap().clear();
    env.run_actions("move monitor right");

    assert!(
        env.moves
            .lock()
            .unwrap()
            .iter()
            .any(|(id, ..)| *id == w1.hwnd_id),
        "cross-monitor move should trigger set_position"
    );
}
