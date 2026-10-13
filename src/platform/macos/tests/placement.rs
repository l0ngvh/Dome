use super::*;
use crate::core::Pixels;

#[test]
fn focus_tile_with_visible_float_parks_float_before_mirror_output() {
    let mut env = TestEnv::new();
    env.setup();
    let tile = env.spawn_window(100, "Tile", "Tile");
    let float = env.spawn_window(200, "Float", "Float");
    env.discover_windows(&[tile, float]);
    env.run_actions("toggle float");
    env.settle(10);
    env.set_corner_radius(float, Length::new(17.0));
    env.simulate_external_move(float, 200, 150, 600, 400);
    env.settle(10);
    let snapshot = env.last_float_snapshot(float).unwrap();
    assert_eq!(
        snapshot.outer_frame,
        PixelRect::new(196, 146, 608, 408).to_dimension()
    );
    assert_eq!(
        snapshot.content_dim,
        PixelRect::new(200, 150, 600, 400).to_dimension()
    );
    assert_eq!(snapshot.corner_radius, Length::new(17.0));
    assert_eq!(snapshot.cocoa_frame.origin.x, 196.0);
    assert_eq!(snapshot.cocoa_frame.origin.y, 526.0);
    assert_eq!(snapshot.cocoa_frame.size.width, 608.0);
    assert_eq!(snapshot.cocoa_frame.size.height, 408.0);
    env.clear_effects();
    env.focus_window(tile);
    assert!(env.is_offscreen(float));
    let effects = env.effects();
    let park = effects
        .iter()
        .position(|effect| effect == &format!("park {float}"))
        .unwrap();
    let mirror = effects
        .iter()
        .position(|effect| effect == "floats")
        .unwrap();
    assert!(park < mirror);
    assert_eq!(
        env.last_float_snapshot(float).unwrap().outer_frame,
        snapshot.outer_frame
    );
    env.remove_windows(&[float]);
    assert!(env.last_float_snapshot(float).is_none());
}

#[test]
fn single_window_placed_in_view() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn a_fractional_work_area_keeps_the_window_inside_it() {
    let mut macos = TestEnv::new();
    // Zero border so the sole tile fills the work area exactly, leaving no inset to
    // absorb a sub-point rounding error.
    macos
        .builder()
        .tiling(|tiling| tiling.border_size = Pixels::ZERO)
        .build();

    let mut monitor = default_monitor();
    monitor.work_area = PixelRect::from_dimension_inward(FRACTIONAL_WORK_AREA);
    macos.change_monitors(vec![monitor]);

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);
}

#[test]
fn degenerate_content_box_parks_window() {
    let mut macos = TestEnv::new();
    // Each edge exceeds half of SCREEN_HEIGHT, so no content height remains.
    macos
        .builder()
        .tiling(|tiling| tiling.border_size = Pixels::new(600))
        .build();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    assert!(macos.is_offscreen(cg1));

    macos.change_config("return {}");
    macos.settle(10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn two_windows_split_horizontally() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    let (x1, _, w1, _) = macos.window_frame(cg1);
    let (x2, _, w2, _) = macos.window_frame(cg2);
    assert!(x1 < x2);
    assert!(w1 > 0 && w2 > 0);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn tile_past_work_area_is_trimmed() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[cg1]);
    macos.settle(10);

    use crate::platform::macos::dome::events::{HubMessage, WindowPlacement};

    let mut scene = macos.scene();
    let WindowPlacement::Tiling { placement, .. } =
        &mut scene.windows.get_mut(&cg1).unwrap().placement
    else {
        panic!("expected a tile");
    };
    placement.content_box = PixelRect::new(-92, 4, 1912, 1072);
    placement.visible_content_box = PixelRect::new(0, 4, 1820, 1072);
    macos.send_message(HubMessage::Scene(scene));
    macos.ui_pass();
    assert_eq!(macos.window_frame(cg1), (0, 4, 1820, 1072));
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    let placed = macos.window_frame(cg1);

    macos.run_actions("focus workspace 1");
    macos.settle(10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    macos.run_actions("focus workspace 0");
    macos.settle(10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg1), placed);
}

#[test]
fn float_window_moved_by_user() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);

    macos.simulate_external_move(cg2, 200, 150, 600, 400);
    macos.settle(10);

    // Float should stay at the user-chosen position, not be corrected
    assert_eq!(macos.window_frame(cg2), (200, 150, 600, 400));

    let border = macos.border();
    let snap = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot should be present for focused float");
    assert_eq!(
        snap.outer_frame,
        Dimension::new(
            Length::new(200.0 - border),
            Length::new(150.0 - border),
            Length::new(600.0 + 2.0 * border),
            Length::new(400.0 + 2.0 * border),
        )
    );

    let moves_before = macos.recorded_moves().len();
    macos.settle(10);
    let moves_after = macos.recorded_moves();
    let new_moves: Vec<_> = moves_after[moves_before..]
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .collect();
    assert!(
        new_moves.is_empty(),
        "idempotence: expected no set_frame for cg2 after settle, got {new_moves:?}"
    );
}

#[test]
fn float_window_reshaped_on_border_size_change() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);

    let snap_before = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist once cg2 is floated and visible");

    // Clear the move log so we can assert on set_frame calls caused strictly
    // by the config change.
    macos.clear_moves();

    // A border several points above the default, so the delta cannot be
    // mistaken for rounding noise.
    macos.change_config("return { border_size = 12 }");

    // Check before settle because settle drains the move log.
    let reshape_moves: Vec<_> = macos
        .recorded_moves()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        !reshape_moves.is_empty(),
        "expected at least one set_frame for cg2 after border_size change, got none"
    );

    macos.settle(10);

    // Outer-frame values are exact integers by construction (default float
    // placement rounds to whole pixels).
    let expected_x = snap_before.outer_frame.x.value() as i32 + 12;
    let expected_y = snap_before.outer_frame.y.value() as i32 + 12;
    let expected_w = snap_before.outer_frame.width.value() as i32 - 24;
    let expected_h = snap_before.outer_frame.height.value() as i32 - 24;
    assert_eq!(
        macos.window_frame(cg2),
        (expected_x, expected_y, expected_w, expected_h)
    );

    let snap_after = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist after re-flush");
    assert_eq!(
        snap_after.outer_frame, snap_before.outer_frame,
        "border-size change must not alter the hub-stored outer dim"
    );
    assert_eq!(
        snap_after.content_dim,
        Dimension::new(
            Length::new(snap_before.outer_frame.x.value() + 12.0),
            Length::new(snap_before.outer_frame.y.value() + 12.0),
            Length::new(snap_before.outer_frame.width.value() - 24.0),
            Length::new(snap_before.outer_frame.height.value() - 24.0),
        )
    );
}

#[test]
fn float_place_with_same_target_is_noop() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[cg1, cg2]);
    macos.settle(10);

    macos.run_actions("toggle float");
    macos.settle(10);

    macos.clear_moves();

    macos.layout();
    macos.settle(10);

    let moves: Vec<_> = macos
        .recorded_moves()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        moves.is_empty(),
        "expected zero set_frame calls for cg2 on same-target re-place, got {moves:?}"
    );
}

#[test]
fn multi_monitor_per_display() {
    let mut macos = TestEnv::new();
    macos.setup();

    macos.change_monitors(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win1]);
    macos.settle(10);

    macos.run_actions("focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[win2]);
    macos.settle(10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 4, 2552, 1432));
}

fn reserved_area_config(body: &str) -> String {
    format!("return {{ reserved_area = function(monitor) {body} end }}")
}

#[test]
fn a_reserved_area_applies_from_startup() {
    let mut macos = TestEnv::new();
    macos
        .builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();

    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);

    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );
}

#[test]
fn a_fullscreen_window_starts_below_the_reserved_area() {
    let mut macos = TestEnv::new();
    macos
        .builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();

    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);
    macos.run_actions("toggle fullscreen");
    macos.settle(10);

    assert_eq!(macos.window_frame(win), (0, 30, 1920, 1050));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );
}

#[test]
fn a_reserved_area_applies_to_an_added_monitor() {
    let mut macos = TestEnv::new();
    macos
        .builder()
        .config_source(&reserved_area_config(
            "if monitor.name == 'External' then return { top = 30 } end",
        ))
        .build();

    macos.change_monitors(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win1]);
    macos.settle(10);

    macos.run_actions("focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[win2]);
    macos.settle(10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 34, 2552, 1402));
}

#[test]
fn a_reserved_area_matches_the_numbered_name() {
    let mut macos = TestEnv::new();
    macos
        .builder()
        .config_source(&reserved_area_config(
            "if monitor.name == 'DELL #2' then return { top = 30 } end",
        ))
        .build();

    let left = MonitorInfo {
        name: "DELL".to_string(),
        ..default_monitor()
    };
    let right = MonitorInfo {
        name: "DELL".to_string(),
        ..second_monitor()
    };
    macos.change_monitors(vec![left, right]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win1]);
    macos.settle(10);

    macos.run_actions("focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[win2]);
    macos.settle(10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 34, 2552, 1402));
}

#[test]
fn a_reload_reapplies_the_reserved_area() {
    let mut macos = TestEnv::new();
    macos.setup();

    let win = macos.spawn_window(100, "Safari", "Google");
    macos.discover_windows(&[win]);
    macos.settle(10);
    assert_eq!(macos.window_frame(win), (4, 4, 1912, 1072));

    macos.change_config(&reserved_area_config("return { top = 30 }"));
    macos.settle(10);
    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );

    macos.change_config(&reserved_area_config(
        "return { top = 0, bottom = 0, left = 0, right = 0 }",
    ));
    macos.settle(10);
    assert_eq!(macos.window_frame(win), (4, 4, 1912, 1072));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 0, 1920, 1080).to_dimension()
    );
}

fn temp_layout_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("dome_{name}_{nanos}.lua"))
}

/// Removes the file when dropped, so a failed assertion does not leave it in the temp
/// directory.
struct TempLayoutFile(std::path::PathBuf);

impl TempLayoutFile {
    fn new(name: &str, contents: &str) -> Self {
        let path = temp_layout_path(name);
        std::fs::write(&path, contents).unwrap();
        Self(path)
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempLayoutFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

const TERMINAL_THEN_SAFARI: &str = r#"
return {
  ["Test"] = {
    ["0"] = {
      layout = "partition_tree",
      tree = {
        split = "horizontal",
        children = { { app = "Terminal" }, { app = "Safari" } },
      },
    },
  },
}
"#;

#[test]
fn apply_layout_file_places_windows_from_the_file() {
    let mut macos = TestEnv::new();
    macos.setup();
    let safari = macos.spawn_window(100, "Safari", "Google");
    let terminal = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[safari, terminal]);
    macos.settle(10);
    assert!(macos.window_frame(safari).0 < macos.window_frame(terminal).0);
    let layout = TempLayoutFile::new("apply_layout_places_windows", TERMINAL_THEN_SAFARI);

    macos.apply_layout_file(layout.path());
    macos.settle(10);

    assert!(macos.window_frame(terminal).0 < macos.window_frame(safari).0);
}

#[test]
fn apply_layout_file_that_fails_to_load_keeps_the_arrangement() {
    let mut macos = TestEnv::new();
    macos.setup();
    let safari = macos.spawn_window(100, "Safari", "Google");
    let terminal = macos.spawn_window(101, "Terminal", "zsh");
    macos.discover_windows(&[safari, terminal]);
    macos.settle(10);
    let before = (macos.window_frame(safari), macos.window_frame(terminal));
    let missing = temp_layout_path("apply_layout_missing");

    macos.apply_layout_file(missing.to_str().unwrap());
    macos.settle(10);

    assert_eq!(
        (macos.window_frame(safari), macos.window_frame(terminal)),
        before
    );
}

#[test]
fn tiling_border_takes_the_latest_corner_radius() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Finder", "Home");
    macos.set_corner_radius(cg1, Length::new(26.0));
    macos.discover_windows(&[cg1]);
    macos.settle(10);
    let id = macos.window_id(cg1).unwrap();
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(26.0)
    );

    // Hiding Finder's toolbar shrinks its corner radius without resizing the window.
    let (x, y, w, h) = macos.window_frame(cg1);
    macos.set_corner_radius(cg1, Length::new(16.0));
    macos.simulate_external_move(cg1, x, y, w, h);
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(16.0)
    );
}

#[test]
fn float_border_takes_the_window_corner_radius() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Chrome", "Google");
    macos.set_corner_radius(cg1, Length::new(20.0));
    macos.discover_windows(&[cg1]);
    macos.settle(10);
    macos.run_actions("toggle float");
    macos.settle(10);

    assert_eq!(
        macos.last_float_snapshot(cg1).unwrap().corner_radius,
        Length::new(20.0)
    );
}

#[test]
fn a_window_first_seen_in_native_fullscreen_takes_its_corner_radius_on_exit() {
    let mut macos = TestEnv::new();
    macos.setup();

    let cg1 = macos.spawn_window(100, "Finder", "Home");
    macos.set_native_fullscreen(cg1, true);
    macos.discover_native_fullscreen(cg1);
    macos.settle(10);

    macos.set_corner_radius(cg1, Length::new(26.0));
    macos.exit_native_fullscreen(cg1, 200, 200, 800, 600);
    macos.settle(10);

    let id = macos.window_id(cg1).unwrap();
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(26.0)
    );
}
