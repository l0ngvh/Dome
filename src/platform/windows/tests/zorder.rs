use super::*;

/// Asserts that every hwnd in `tiling` appears above `overlay_id` in the
/// normal z-order band. Windows below the overlay (e.g. hidden via
/// `move_offscreen` -> `HWND_BOTTOM`) are acceptable and mirror real
/// Win32 behavior.
fn assert_tiling_above_overlay(env: &TestEnv, tiling: &[HwndId], overlay_id: HwndId) {
    let stack = env.tiling_z_order();
    let overlay_pos = stack
        .iter()
        .position(|&id| id == overlay_id)
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

fn tiling_overlay_showing(env: &TestEnv, hwnd: HwndId) -> HwndId {
    let id = env
        .dome
        .window_id_for(hwnd)
        .expect("window must be registered");
    let mut matching = (0..env.painted_monitors())
        .filter(|&i| env.painted_windows(i).iter().any(|w| w.id == id))
        .filter_map(|i| env.tiling_overlay_id_for(env.painted_monitor_id(i)));
    let overlay = matching.next().expect("no overlay displays that window");
    assert!(matching.next().is_none(), "two overlays display one window");
    overlay
}

fn normal_index(env: &TestEnv, hwnd: HwndId) -> usize {
    env.tiling_z_order()
        .iter()
        .position(|&id| id == hwnd)
        .unwrap_or_else(|| panic!("{hwnd:?} not found in normal band"))
}

fn two_monitors_focused_left() -> (TestEnv, HwndId, HwndId, HwndId) {
    let mut env = TestEnv::builder()
        .monitors(vec![default_monitor(), second_monitor()])
        .build();
    let w1 = env.open();
    env.run_actions("focus monitor right");
    let w2 = env.open();
    let w3 = env.open();
    env.run_actions("focus monitor left");
    (env, w1, w2, w3)
}

#[test]
fn single_window_above_overlay() {
    let mut env = TestEnv::new();
    let w1 = env.open();

    let overlay = env.tiling_overlay_id();
    assert_eq!(env.tiling_z_order(), vec![w1, overlay]);
}

#[test]
fn all_tiling_above_overlay() {
    let mut env = TestEnv::new();
    let windows = env.open_many(5);

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &windows, overlay);
}

#[test]
fn focus_change_preserves_overlay_behind() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    let overlay = env.tiling_overlay_id();

    env.focus_window(w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), overlay);

    env.focus_window(w2);
    assert_eq!(*env.tiling_z_order().last().unwrap(), overlay);

    env.focus_window(w1);
    assert_eq!(*env.tiling_z_order().last().unwrap(), overlay);
}

#[test]
fn add_window_to_existing() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    let w3 = env.open();

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2, w3], overlay);
}

#[test]
fn workspace_switch_restores_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.run_actions("focus workspace 1");
    env.run_actions("focus workspace 0");

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn float_window_above_tiling_and_overlay() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();
    let w3 = env.open();

    // w3 is focused. Float it.
    env.run_actions("toggle float");

    // w3 should be in the topmost band (first in full z_order)
    let full = env.z_order();
    assert_eq!(full[0], w3);

    // Normal band should have w1, w2, and overlay, with overlay last
    let overlay = env.tiling_overlay_id();
    let normal = env.tiling_z_order();
    assert!(!normal.contains(&w3));
    assert_eq!(*normal.last().unwrap(), overlay);
}

#[test]
fn unfloat_window_rejoins_tiling_chain() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.run_actions("toggle float");
    env.run_actions("toggle float");

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn stable_positions_still_update_zorder() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);

    // Second apply_layout: positions unchanged, z-order should remain correct
    env.layout();

    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn move_window_to_other_workspace() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    let w3 = env.open();

    // w3 is focused. Move it to workspace 1.
    env.run_actions("move workspace 1");

    assert!(env.is_offscreen(w3));

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn stable_windows_skip_set_position() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let _w2 = env.open();

    env.clear_moves();
    env.layout();

    env.assert_settled("stable tiling windows should not trigger set_position");
}

#[test]
fn monitor_switch_issues_set_position() {
    let mut env = TestEnv::new();
    env.add_monitor(second_monitor());
    let w1 = env.open();

    env.clear_moves();
    env.run_actions("move monitor right");

    assert!(
        env.moved(w1),
        "cross-monitor move should trigger set_position"
    );
}

/// New tiling overlay parks at the bottom of the normal band on creation,
/// so the next CreateWindowExW for a managed window naturally sits above it.
#[test]
fn tiling_overlay_seeded_at_bottom() {
    let env = TestEnv::new();
    let overlay = env.tiling_overlay_id();
    assert_eq!(env.tiling_z_order(), vec![overlay]);
}

#[test]
fn unfloat_drops_window_from_topmost_band() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

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
    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn steady_state_apply_layout_keeps_the_same_zorder() {
    let mut env = TestEnv::new();
    env.open();
    env.open();
    let stack_before = env.z_order();
    env.layout();
    let stack_after = env.z_order();
    assert_eq!(
        stack_after, stack_before,
        "second apply_layout must land on the same stack"
    );
}

/// Parked-offscreen windows must sit strictly below the tiling overlay so that
/// Win32's close-time focus walk lands on a Dome-owned window rather than
/// reactivating an inactive workspace.
#[test]
fn tiling_overlay_stays_above_parked_windows() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();

    env.run_actions("focus workspace 1");
    assert!(env.is_offscreen(w1));
    assert!(env.is_offscreen(w2));

    let overlay = env.tiling_overlay_id();
    let stack = env.z_order();
    let overlay_idx = stack
        .iter()
        .position(|&h| h == overlay)
        .expect("tiling overlay must be in z-stack");
    let w1_idx = stack.iter().position(|&h| h == w1).unwrap();
    let w2_idx = stack.iter().position(|&h| h == w2).unwrap();
    assert!(
        overlay_idx < w1_idx,
        "overlay at {overlay_idx} must sit above parked w1 at {w1_idx}"
    );
    assert!(
        overlay_idx < w2_idx,
        "overlay at {overlay_idx} must sit above parked w2 at {w2_idx}"
    );
}

#[test]
fn float_overlay_sits_directly_below_float_window() {
    let mut env = TestEnv::new();
    let _w1 = env.open();
    let w2 = env.open();

    env.run_actions("toggle float");

    let overlay_id = env
        .float_overlay_id(
            env.dome
                .window_id_for(w2)
                .expect("window must be registered"),
        )
        .expect("float overlay must exist after toggle float");

    let stack = env.z_order();
    let w2_idx = stack.iter().position(|&h| h == w2).unwrap();
    let overlay_idx = stack
        .iter()
        .position(|&h| h == overlay_id)
        .expect("float overlay must be in z-stack after toggle float");
    assert_eq!(
        overlay_idx,
        w2_idx + 1,
        "float overlay (idx {overlay_idx}) must sit directly below float window w2 (idx {w2_idx})"
    );
}

#[test]
fn overlay_recovers_when_promoted_to_top() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    let overlay = env.tiling_overlay_id();

    env.raise_without_notifying_dome(overlay);
    env.layout();

    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
}

#[test]
fn unfocused_monitor_is_left_alone() {
    let (mut env, _w1, w2, w3) = two_monitors_focused_left();
    let right_overlay = tiling_overlay_showing(&env, w2);

    env.raise_without_notifying_dome(right_overlay);
    env.layout();

    let overlay_pos = normal_index(&env, right_overlay);
    assert!(
        overlay_pos < normal_index(&env, w2),
        "unfocused monitor must keep its overlay above {w2:?}"
    );
    assert!(
        overlay_pos < normal_index(&env, w3),
        "unfocused monitor must keep its overlay above {w3:?}"
    );
}

#[test]
fn focusing_a_monitor_restores_its_overlay_order() {
    let (mut env, _w1, w2, w3) = two_monitors_focused_left();
    let right_overlay = tiling_overlay_showing(&env, w2);
    env.raise_without_notifying_dome(right_overlay);

    env.run_actions("focus monitor right");

    assert_tiling_above_overlay(&env, &[w2, w3], right_overlay);
}

#[test]
fn container_highlight_leaves_the_stack_alone() {
    let mut env = TestEnv::new();
    env.open();
    env.open();
    env.run_actions("focus parent");

    let before = env.z_order();
    env.layout();

    assert_eq!(
        env.z_order(),
        before,
        "a container highlight must order nothing"
    );
}

#[test]
fn float_focus_leaves_the_stack_alone() {
    let mut env = TestEnv::new();
    env.open();
    env.open();
    env.open();
    env.run_actions("toggle float");

    let before = env.z_order();
    env.layout();

    assert_eq!(env.z_order(), before, "a focused float must order nothing");
}

#[test]
fn workspace_switch_keeps_overlay_between_visible_and_parked() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    env.run_actions("focus workspace 1");
    let w3 = env.open();
    env.run_actions("focus workspace 0");

    let overlay = env.tiling_overlay_id();
    assert_tiling_above_overlay(&env, &[w1, w2], overlay);
    assert!(
        normal_index(&env, overlay) < normal_index(&env, w3),
        "overlay must stay above parked {w3:?}"
    );
}

#[test]
fn unfloat_keeps_overlay_out_of_topmost_band() {
    let mut env = TestEnv::new();
    let w1 = env.open();
    let w2 = env.open();
    let overlay = env.tiling_overlay_id();

    env.run_actions("toggle float");
    env.focus_window(w1);
    env.run_actions("toggle float");
    assert!(
        env.is_topmost(w1) && env.is_topmost(w2),
        "both windows must float before the unfloat under test"
    );

    env.run_actions("toggle float");

    assert!(
        !env.is_topmost(overlay),
        "the unfloated reference {w1:?} must leave the topmost band before the overlay \\
         is inserted below it, or the overlay lands above the still-topmost {w2:?} \\
         and Win32 promotes it into that band"
    );
}
