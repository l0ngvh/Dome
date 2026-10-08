use super::*;
use crate::core::Pixels;

#[test]
fn single_window_placed_in_view() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn a_fractional_work_area_keeps_the_window_inside_it() {
    let mut macos = MacOS::new();
    // Zero border so the sole tile fills the work area exactly, leaving no inset to
    // absorb a sub-point rounding error.
    let mut dome = macos
        .dome_builder()
        .tiling(|tiling| tiling.border_size = Pixels::ZERO)
        .build();

    let mut monitor = default_monitor();
    monitor.work_area = PixelRect::from_dimension_inward(FRACTIONAL_WORK_AREA);
    dome.monitors_changed(vec![monitor]);

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_inside_work_area(macos.window_frame(cg1), FRACTIONAL_WORK_AREA);
}

#[test]
fn degenerate_content_box_parks_window() {
    let mut macos = MacOS::new();
    // Each edge exceeds half of SCREEN_HEIGHT, so no content height remains.
    let mut dome = macos
        .dome_builder()
        .tiling(|tiling| tiling.border_size = Pixels::new(600))
        .build();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert!(macos.is_offscreen(cg1));

    macos.change_config(&mut dome, "return {}");
    macos.settle(&mut dome, 10);

    assert!(!macos.is_offscreen(cg1));
    assert_eq!(macos.window_frame(cg1), (4, 4, 1912, 1072));
}

#[test]
fn two_windows_split_horizontally() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (x1, _, w1, _) = macos.window_frame(cg1);
    let (x2, _, w2, _) = macos.window_frame(cg2);
    assert!(x1 < x2);
    assert!(w1 > 0 && w2 > 0);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
}

#[test]
fn drag_drop_tiles_on_screen_even_split() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);

    start_drag(&mut dome, 100);
    macos.window(cg1).position.set((500, 300));
    macos.window(cg1).size.set((400, 400));

    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg2)], &[], &[]);
    macos.settle(&mut dome, 10);

    end_drag(&mut dome, &macos, 100, cg1, 500, 300, 400, 400);
    macos.settle(&mut dome, 10);

    let (x1, _, w1, _) = macos.window_frame(cg1);
    let (x2, _, w2, _) = macos.window_frame(cg2);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!((x1, w1), (4, 952), "cg1 fills its on-screen even half");
    assert_eq!((x2, w2), (964, 952), "cg2 fills its on-screen even half");
}

#[test]
fn tile_past_work_area_is_trimmed() {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
        .tiling(|tiling| tiling.layout = crate::core::Strategy::Master)
        .build();
    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let (x2, y2, w2, _) = macos.window_frame(cg2);
    macos.simulate_external_move(&mut dome, cg2, x2, y2, w2, 1500);
    macos.settle(&mut dome, 10);

    assert_eq!(
        macos.window_frame(cg2),
        (964, 4, 952, 1076),
        "the reported 1500 height runs past the bottom of the work area, so cg2 is trimmed to it"
    );
}

#[test]
fn workspace_switch_hides_and_restores() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    let placed = macos.window_frame(cg1);

    send(&mut dome, "focus workspace 1");
    macos.settle(&mut dome, 10);
    assert!(macos.is_offscreen(cg1));
    assert!(macos.is_offscreen(cg2));

    send(&mut dome, "focus workspace 0");
    macos.settle(&mut dome, 10);
    assert!(!macos.is_offscreen(cg1));
    assert!(!macos.is_offscreen(cg2));
    assert_eq!(macos.window_frame(cg1), placed);
}

#[test]
fn float_window_moved_by_user() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    macos.simulate_external_move(&mut dome, cg2, 200, 150, 600, 400);
    macos.settle(&mut dome, 10);

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

    let moves_before = macos.moves.borrow().len();
    macos.settle(&mut dome, 10);
    let moves_after = macos.moves.borrow();
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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    let snap_before = macos
        .last_float_snapshot(cg2)
        .expect("float snapshot must exist once cg2 is floated and visible");

    // Clear the move log so we can assert on set_frame calls caused strictly
    // by the config change.
    macos.moves.borrow_mut().clear();

    // A border several points above the default, so the delta cannot be
    // mistaken for rounding noise.
    macos.change_config(&mut dome, "return { border_size = 12 }");

    // Check before settle because settle drains the move log.
    let reshape_moves: Vec<_> = macos
        .moves
        .borrow()
        .iter()
        .filter(|(id, _, _, _, _)| *id == cg2)
        .copied()
        .collect();
    assert!(
        !reshape_moves.is_empty(),
        "expected at least one set_frame for cg2 after border_size change, got none"
    );

    macos.settle(&mut dome, 10);

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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Safari", "Google");
    let cg2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, cg1), new_window(&macos, cg2)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    macos.moves.borrow_mut().clear();

    dome.flush_layout();
    macos.settle(&mut dome, 10);

    let moves: Vec<_> = macos
        .moves
        .borrow()
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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win1)], &[], &[]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win2)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 4, 2552, 1432));
}

fn reserved_area_config(body: &str) -> String {
    format!("return {{ reserved_area = function(monitor) {body} end }}")
}

#[test]
fn a_reserved_area_applies_from_startup() {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );
}

#[test]
fn a_fullscreen_window_starts_below_the_reserved_area() {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
        .config_source(&reserved_area_config("return { top = 30 }"))
        .build();

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);
    send(&mut dome, "toggle fullscreen");
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win), (0, 30, 1920, 1050));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );
}

#[test]
fn a_reserved_area_applies_to_an_added_monitor() {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
        .config_source(&reserved_area_config(
            "if monitor.name == 'External' then return { top = 30 } end",
        ))
        .build();

    dome.monitors_changed(vec![default_monitor(), second_monitor()]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win1)], &[], &[]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win2)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 34, 2552, 1402));
}

#[test]
fn a_reserved_area_matches_the_numbered_name() {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
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
    dome.monitors_changed(vec![left, right]);

    let win1 = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win1)], &[], &[]);
    macos.settle(&mut dome, 10);

    send(&mut dome, "focus monitor right");
    let win2 = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win2)], &[], &[]);
    macos.settle(&mut dome, 10);

    assert_eq!(macos.window_frame(win1), (4, 4, 1912, 1072));
    assert_eq!(macos.window_frame(win2), (1924, 34, 2552, 1402));
}

#[test]
fn a_reload_reapplies_the_reserved_area() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let win = macos.spawn_window(100, "Safari", "Google");
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, win)], &[], &[]);
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 4, 1912, 1072));

    macos.change_config(&mut dome, &reserved_area_config("return { top = 30 }"));
    macos.settle(&mut dome, 10);
    assert_eq!(macos.window_frame(win), (4, 34, 1912, 1042));
    assert_eq!(
        macos.painted_work_area(0),
        PixelRect::new(0, 30, 1920, 1050).to_dimension()
    );

    macos.change_config(
        &mut dome,
        &reserved_area_config("return { top = 0, bottom = 0, left = 0, right = 0 }"),
    );
    macos.settle(&mut dome, 10);
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
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let safari = macos.spawn_window(100, "Safari", "Google");
    let terminal = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, safari), new_window(&macos, terminal)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);
    assert!(macos.window_frame(safari).0 < macos.window_frame(terminal).0);
    let layout = TempLayoutFile::new("apply_layout_places_windows", TERMINAL_THEN_SAFARI);

    dome.apply_layout_file(layout.path());
    macos.settle(&mut dome, 10);

    assert!(macos.window_frame(terminal).0 < macos.window_frame(safari).0);
}

#[test]
fn apply_layout_file_that_fails_to_load_keeps_the_arrangement() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();
    let safari = macos.spawn_window(100, "Safari", "Google");
    let terminal = macos.spawn_window(101, "Terminal", "zsh");
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![new_window(&macos, safari), new_window(&macos, terminal)],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);
    let before = (macos.window_frame(safari), macos.window_frame(terminal));
    let missing = temp_layout_path("apply_layout_missing");

    dome.apply_layout_file(missing.to_str().unwrap());
    macos.settle(&mut dome, 10);

    assert_eq!(
        (macos.window_frame(safari), macos.window_frame(terminal)),
        before
    );
}

#[test]
fn tiling_border_takes_the_latest_corner_radius() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Finder", "Home");
    macos.window(cg1).corner_radius.set(Length::new(26.0));
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    let id = dome.tracked_window(cg1).unwrap().window_id;
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(26.0)
    );

    // Hiding Finder's toolbar shrinks its corner radius without resizing the window.
    let (x, y, w, h) = macos.window_frame(cg1);
    macos.window(cg1).corner_radius.set(Length::new(16.0));
    macos.simulate_external_move(&mut dome, cg1, x, y, w, h);
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(16.0)
    );
}

#[test]
fn float_border_takes_the_window_corner_radius() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Chrome", "Google");
    macos.window(cg1).corner_radius.set(Length::new(20.0));
    dome.reconcile_windows(&[], &[], &[], vec![new_window(&macos, cg1)], &[], &[]);
    macos.settle(&mut dome, 10);
    send(&mut dome, "toggle float");
    macos.settle(&mut dome, 10);

    assert_eq!(
        macos.last_float_snapshot(cg1).unwrap().corner_radius,
        Length::new(20.0)
    );
}

#[test]
fn a_window_first_seen_in_native_fullscreen_takes_its_corner_radius_on_exit() {
    let mut macos = MacOS::new();
    let mut dome = macos.setup_dome();

    let cg1 = macos.spawn_window(100, "Finder", "Home");
    macos.window(cg1).set_native_fullscreen(true);
    let PendingAdd::Positioned { new, .. } = new_window(&macos, cg1) else {
        unreachable!("new_window builds a positioned add");
    };
    dome.reconcile_windows(
        &[],
        &[],
        &[],
        vec![PendingAdd::NativeFullscreen { new }],
        &[],
        &[],
    );
    macos.settle(&mut dome, 10);

    macos.window(cg1).corner_radius.set(Length::new(26.0));
    macos.exit_native_fullscreen(&mut dome, cg1, 200, 200, 800, 600);
    macos.settle(&mut dome, 10);

    let id = dome.tracked_window(cg1).unwrap().window_id;
    assert_eq!(
        macos.last_scene_state().tiling_corner_radii[&id],
        Length::new(26.0)
    );
}
