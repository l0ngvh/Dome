use super::*;

#[test]
fn toggle_fullscreen_hides_siblings() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w2 is focused (last added). Toggle fullscreen.
    env.run_actions("toggle fullscreen");

    assert!(env.is_offscreen(w1));
    assert!(env.is_bottom(w1));
    let d2 = env.dim(w2);
    assert_eq!(d2.x, Length::ZERO);
    assert_eq!(d2.y, Length::ZERO);
    assert_eq!(d2.width, SCREEN_WIDTH);
    assert_eq!(d2.height, SCREEN_HEIGHT);
}

#[test]
fn toggle_fullscreen_on_and_off() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let before1 = env.dim(w1);
    let before2 = env.dim(w2);

    env.run_actions("toggle fullscreen");
    env.run_actions("toggle fullscreen");

    // Both should be back to tiled positions
    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
    assert!(!env.is_topmost(w1));
    assert!(!env.is_topmost(w2));
    assert_eq!(env.dim(w1), before1);
    assert_eq!(env.dim(w2), before2);
}

#[test]
fn toggle_float() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w2 is focused. Toggle float.
    env.run_actions("toggle float");

    // w1 should fill the screen (w2 is floating, not part of tiling)
    assert!(!env.is_offscreen(w1));
    assert!(!env.is_offscreen(w2));
    assert!(!env.is_topmost(w1));
    assert!(env.is_topmost(w2));
    let d1 = env.dim(w1);
    let border = env.config.border_size;
    assert!(
        (d1.width - (SCREEN_WIDTH - Length::new(2.0 * border))).abs() < Length::new(1.0),
        "w1 should fill screen width, got {}",
        d1.width
    );
}

#[test]
fn fullscreen_restored_after_workspace_switch() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle fullscreen");
    let fs_dim = env.dim(w2);

    // Switch away
    env.run_actions("focus workspace 1");
    assert!(env.is_offscreen(w2));

    // Switch back -- fullscreen window should be restored
    env.run_actions("focus workspace 0");
    assert_eq!(env.dim(w2), fs_dim);
}

#[test]
fn window_created_as_fullscreen_borderless() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    // Second window arrives already fullscreen
    let _w2 = env.open(2, "Game", "game.exe", fullscreen_dim());

    // w1 should be hidden (fullscreen window takes over)
    assert!(env.is_offscreen(w1));
}

#[test]
fn move_window_to_other_workspace() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Move w2 (focused) to workspace 1
    env.run_actions("move workspace 1");

    // w2 should be offscreen, w1 should fill the screen
    assert!(env.is_offscreen(w2));
    assert!(env.is_bottom(w2));
    assert_h_tiled(
        &[env.dim(w1)],
        default_monitor().dimension,
        env.config.border_size,
    );
}

#[test]
fn fullscreen_borderless_minimizes_on_workspace_switch() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());

    env.run_actions("focus workspace 1");

    assert!(env.is_minimized(w1));
}

#[test]
fn fullscreen_exclusive_not_repositioned() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", SPAWN_DIM);

    env.enter_exclusive_fullscreen(w1);
    let after_exclusive = env.dim(w1);

    // Switching workspace should not reposition (hide_window is noop for exclusive)
    env.run_actions("focus workspace 1");
    assert_eq!(env.dim(w1), after_exclusive);
}

#[test]
fn borderless_fullscreen_restored_on_workspace_switch_back() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());
    assert!(!env.is_minimized(w1));
    assert_eq!(env.dim(w1), fullscreen_dim());

    env.run_actions("focus workspace 1");
    assert!(env.is_minimized(w1));

    env.run_actions("focus workspace 0");
    assert!(!env.is_minimized(w1));
    assert_eq!(env.dim(w1), fullscreen_dim());
}

#[test]
fn dome_minimized_window_survives_minimize_event() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());

    env.run_actions("focus workspace 1");
    assert!(env.is_minimized(w1));

    env.minimize_window(w1);
    assert!(env.is_minimized(w1));
}

#[test]
fn exclusive_fullscreen_survives_minimize_event() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", SPAWN_DIM);

    env.enter_exclusive_fullscreen(w1);
    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));

    env.minimize_window(w1);
    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));
}

#[test]
fn float_restored_from_offscreen_is_topmost() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Float w2, then switch away
    env.run_actions("toggle float");
    assert!(env.is_topmost(w2));

    env.run_actions("focus workspace 1");
    assert!(env.is_offscreen(w2));
    assert!(env.is_bottom(w2));

    // Switch back -- float should be topmost again
    env.run_actions("focus workspace 0");
    assert!(!env.is_offscreen(w2));
    assert!(env.is_topmost(w2));
}

#[test]
fn float_to_tiling_loses_topmost() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    env.run_actions("toggle float");
    assert!(env.is_topmost(w2));

    env.run_actions("toggle float");
    assert!(!env.is_topmost(w2));
    assert!(!env.is_topmost(w1));
}

#[test]
fn float_focus_change_retops() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Float both
    env.run_actions("toggle float");
    env.focus_window(w1);
    env.run_actions("toggle float");

    // w1 is focused and float
    assert!(env.is_topmost(w1));

    // Focus w2
    env.focus_window(w2);
    assert!(env.is_topmost(w2));
}

#[test]
fn tiling_windows_not_topmost() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    assert!(!env.is_topmost(w1));
    assert!(!env.is_topmost(w2));
    assert!(!env.is_topmost(w3));
}

#[test]
fn float_survives_sibling_add() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);

    env.run_actions("toggle float");
    assert!(env.is_topmost(w1));

    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w1 should still be float and topmost
    assert!(env.is_topmost(w1));
    assert!(!env.is_topmost(w2));
}

#[test]
fn exclusive_fullscreen_blocks_all_commands() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", SPAWN_DIM);

    env.enter_exclusive_fullscreen(w1);
    let dim_before = env.dim(w1);

    // BlockAll restriction should prevent toggle_float
    env.run_actions("toggle float");
    assert_eq!(env.dim(w1), dim_before);

    // BlockAll restriction should prevent toggle_fullscreen
    env.run_actions("toggle fullscreen");
    assert_eq!(env.dim(w1), dim_before);

    // BlockAll restriction should prevent focus/move/workspace commands
    env.run_actions("focus left");
    assert_eq!(env.dim(w1), dim_before);

    env.run_actions("focus workspace 1");
    assert_eq!(env.dim(w1), dim_before);

    env.run_actions("focus monitor right");
    assert_eq!(env.dim(w1), dim_before);

    env.run_actions("move left");
    assert_eq!(env.dim(w1), dim_before);

    env.run_actions("move workspace 1");
    assert_eq!(env.dim(w1), dim_before);

    env.run_actions("move monitor right");
    assert_eq!(env.dim(w1), dim_before);
}

#[test]
fn borderless_fullscreen_blocks_toggle_float_but_allows_workspace_move() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());
    let dim_before = env.dim(w1);
    assert_eq!(dim_before, fullscreen_dim());

    // ProtectFullscreen restriction should block toggle_float
    env.run_actions("toggle float");
    assert_eq!(env.dim(w1), dim_before);

    // ProtectFullscreen allows workspace move -- window should be minimized
    env.run_actions("move workspace 1");
    assert!(env.is_minimized(w1));
}

#[test]
fn borderless_fullscreen_exit_unblocks_commands() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());
    let dim_before = env.dim(w1);
    assert_eq!(dim_before, fullscreen_dim());

    // Blocked while borderless fullscreen
    env.run_actions("toggle float");
    assert_eq!(env.dim(w1), dim_before);

    // Exit borderless FS: window reports non-fullscreen dimensions
    env.dome.window_moved(w1, dim(100, 100, 800, 600), 1);
    env.dome.apply_layout();

    // toggle_float should now work
    env.run_actions("toggle float");
    assert!(env.is_topmost(w1));
}

#[test]
fn float_overlay_updates_on_focus_away() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.focus_window(w1);
    env.run_actions("toggle float");
    env.focus_window(w2);

    let before = env.float_overlay_state();
    env.focus_window(w1);
    assert_ne!(
        env.float_overlay_state(),
        before,
        "float overlay state must change when focus moves to a different float",
    );
}

#[test]
fn float_overlay_updates_on_focus_to_tiling() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let before = env.float_overlay_state();
    env.focus_window(w1);
    assert_ne!(
        env.float_overlay_state(),
        before,
        "float overlay state must change when focus moves from a float to a tiling window",
    );
}

#[test]
fn float_overlay_updates_on_focus_from_tiling() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.focus_window(w1);

    let before = env.float_overlay_state();
    env.focus_window(w2);
    assert_ne!(
        env.float_overlay_state(),
        before,
        "float overlay state must change when focus moves from a tiling window to a float",
    );
}

#[test]
fn float_settled_skips_update_without_focus_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let before = env.float_overlay_state();
    env.dome.apply_layout();
    assert_eq!(env.float_overlay_state(), before);
}

#[test]
fn float_refocus_same_window_is_noop() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let before = env.float_overlay_state();
    env.focus_window(w1);
    assert_eq!(env.float_overlay_state(), before);
}

#[test]
fn float_focus_away_does_not_change_topmost() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    env.focus_window(w1);
    env.run_actions("toggle float");

    env.focus_window(w2);
    assert!(env.is_topmost(w1));
    assert!(env.is_topmost(w2));
}

#[test]
fn float_overlay_updates_on_position_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let before = env.float_overlay_state();
    let mut new_config = env.config.clone();
    new_config.border_size = env.config.border_size + 2.0;
    env.dome.config_changed(new_config);
    env.dome.apply_layout();
    assert_ne!(
        env.float_overlay_state(),
        before,
        "float overlay must reflect the new border size after config_changed",
    );
}

#[test]
fn config_reload_dispatches_apply_theme_on_flavor_change() {
    let mut env = TestEnv::new(); // default Mocha
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    // Sanity: both overlays start at the default Mocha flavor.
    assert_eq!(env.tiling_overlay_flavor(), crate::theme::Flavor::Mocha);
    assert_eq!(env.float_overlay_flavor(), crate::theme::Flavor::Mocha);

    let mut new_config = env.config.clone();
    new_config.theme = crate::theme::Flavor::Latte;
    env.dome.config_changed(new_config);

    // After a flavor change, both overlays must end up holding Latte.
    assert_eq!(env.tiling_overlay_flavor(), crate::theme::Flavor::Latte);
    assert_eq!(env.float_overlay_flavor(), crate::theme::Flavor::Latte);
}

#[test]
fn config_reload_dispatches_apply_font_on_font_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let new_font = crate::font::FontConfig {
        text_size: 18.0,
        subtext_size: 12.0,
    };
    // Sanity: overlays start at the default font (different from `new_font`).
    assert_ne!(env.tiling_overlay_font(), new_font);
    assert_ne!(env.float_overlay_font(), new_font);

    let mut new_config = env.config.clone();
    new_config.font = new_font.clone();
    env.dome.config_changed(new_config);

    // After a font change, both overlays must hold the new font.
    assert_eq!(env.tiling_overlay_font(), new_font);
    assert_eq!(env.float_overlay_font(), new_font);
}

#[test]
fn tiling_state_preserved_through_user_minimize() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    let dim_before = env.dim(w1);
    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));

    // Simulate OS minimize (user clicks taskbar). The OS minimizes before
    // Dome receives the event.
    env.set_minimized(w1, true);
    env.minimize_window(w1);
    assert!(env.is_minimized(w1));

    // Simulate OS restore (user clicks taskbar again). The OS restores
    // before Dome receives the event.
    env.set_minimized(w1, false);
    env.restore_window(w1);

    // Geometry and mode are preserved: same tiling slot dimensions.
    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));
    assert_eq!(env.dim(w1), dim_before);
}

#[test]
fn float_state_preserved_through_user_minimize() {
    // A float window user-minimized via the OS should return to its
    // original float position and retain topmost z-order when restored.
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // w2 is focused (last added). Toggle to float.
    env.run_actions("toggle float");
    let dim_before = env.dim(w2);
    assert!(env.is_topmost(w2));
    assert!(!env.is_minimized(w2));

    // Simulate OS minimize.
    env.set_minimized(w2, true);
    env.minimize_window(w2);
    assert!(env.is_minimized(w2));

    // Simulate OS restore.
    env.set_minimized(w2, false);
    env.restore_window(w2);

    // Float position and topmost are preserved.
    assert!(!env.is_minimized(w2));
    assert!(!env.is_offscreen(w2));
    assert_eq!(env.dim(w2), dim_before);
    assert!(env.is_topmost(w2));
}

#[test]
fn fullscreen_borderless_state_preserved_through_user_minimize() {
    // A borderless fullscreen window user-minimized via the OS should
    // return to its fullscreen geometry when restored. This is distinct
    // from the dome-minimize path tested by
    // borderless_fullscreen_restored_on_workspace_switch_back.
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());

    let dim_before = env.dim(w1);
    assert_eq!(dim_before, fullscreen_dim());
    assert!(!env.is_minimized(w1));

    // Simulate OS minimize (user clicks taskbar).
    env.set_minimized(w1, true);
    env.minimize_window(w1);
    assert!(env.is_minimized(w1));

    // Simulate OS restore.
    env.set_minimized(w1, false);
    env.restore_window(w1);

    // Fullscreen geometry is preserved.
    assert!(!env.is_minimized(w1));
    assert_eq!(env.dim(w1), fullscreen_dim());
}

#[test]
fn minimized_borderless_fullscreen_dont_get_affect_by_stale_move_event() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", fullscreen_dim());

    env.run_actions("focus workspace 1");
    assert!(env.is_minimized(w1));

    // Replay the OS echo. The fix asserts: minimize survives this.
    env.flush_moves();
    assert!(env.is_minimized(w1));

    // Switching back must still restore at fullscreen_dim.
    env.run_actions("focus workspace 0");
    assert!(!env.is_minimized(w1));
    assert_eq!(env.dim(w1), fullscreen_dim());
}

#[test]
fn fullscreen_exclusive_state_preserved_through_user_minimize() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "Game", "game.exe", SPAWN_DIM);

    env.enter_exclusive_fullscreen(w1);
    let dim_after_exclusive = env.dim(w1);
    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));

    // Simulate OS minimize (user clicks taskbar).
    env.set_minimized(w1, true);
    env.minimize_window(w1);
    assert!(env.is_minimized(w1));

    // Simulate OS restore.
    env.set_minimized(w1, false);
    env.restore_window(w1);

    assert!(!env.is_minimized(w1));
    assert!(!env.is_offscreen(w1));
    assert_eq!(env.dim(w1), dim_after_exclusive);

    // Behavioral witness that we are back in ExclusiveFullscreen (not just
    // at fullscreen-sized geometry by coincidence): the BlockAll restriction
    // is intact, so DisplayModeChange actions are refused. Only
    // ExclusiveFullscreen carries BlockAll; BorderlessFullscreen and
    // Positioned variants would let one of these toggles change the dim.
    env.run_actions("toggle float");
    assert_eq!(env.dim(w1), dim_after_exclusive);
    env.run_actions("toggle fullscreen");
    assert_eq!(env.dim(w1), dim_after_exclusive);
}

#[test]
fn dome_issued_fullscreen_placement_does_not_flip_to_borderless_fullscreen() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Toggle fullscreen on w2 -> show_fullscreen_window places at monitor.dim.
    env.run_actions("toggle fullscreen");
    assert_eq!(env.dim(w2), fullscreen_dim());

    // Simulate the async LOCATIONCHANGE echo: production's worker observes
    // the placed rect covering the monitor's work area.
    env.dome.window_moved(w2, fullscreen_dim(), 1);
    env.dome.apply_layout();

    env.run_actions("toggle fullscreen");
    let d = env.dim(w2);
    assert!(
        d.width < SCREEN_WIDTH,
        "expected re-tiled width < monitor width"
    );
    assert!(!env.is_offscreen(w1), "sibling should be re-tiled");
}
