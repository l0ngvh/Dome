use super::fixture::{TestDome, monitor};
use crate::platform::macos::ui::window::WindowLevel;
use dome_core::PixelRect;
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[test]
fn focus_master_pane_scrolls_mirror_and_restores_native_window() {
    let mut dome = TestDome::new(
        r#"
return {
    layout = "master",
    border_size = 1,
    minimum_width = 1,
    minimum_height = 14,
    master = { master_count = 3 },
    reserved_area = function(monitor) return nil end,
}
"#,
    );
    let first = dome.open("First");
    let second = dome.open("Second");
    let third = dome.open("Third");
    dome.open("Secondary");
    dome.focus(third);
    assert_eq!(dome.frame(second), PixelRect::new(99, 19, 48, 12));
    {
        let state = dome.overlays();
        let mirror = &state.requested.as_ref().unwrap().mirrors[&second];
        assert_eq!(
            mirror.cocoa_frame,
            NSRect::new(NSPoint::new(1.0, 15.0), NSSize::new(48.0, 5.0))
        );
        assert_eq!(mirror.source, PixelRect::new(0, 7, 48, 5).to_dimension());
    }
    dome.focus(first);
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 12));
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .mirrors
            .contains_key(&first)
    );
}

#[test]
fn scroll_content_out_of_view_parks_native_window_without_empty_mirror() {
    let mut dome = TestDome::new(
        r#"
return {
    layout = "scrolling",
    border_size = 1,
    minimum_width = 1,
    minimum_height = 1,
    scrolling = { column_width = 50 },
    reserved_area = function(monitor) return nil end,
}
"#,
    );
    let mut monitor = monitor(1, 0.0);
    monitor.width = 101.0;
    dome.monitors_changed(vec![monitor]);
    dome.open("First");
    let second = dome.open("Second");
    dome.open("Third");
    dome.open("Fourth");
    assert_eq!(dome.frame(second), PixelRect::new(100, 19, 48, 18));
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        assert!(!requested.mirrors.contains_key(&second));
        assert!(
            !requested
                .tiling
                .values()
                .flat_map(|tiling| &tiling.windows)
                .any(|shown| shown.placement.id == second)
        );
    }
    dome.focus(second);
    assert_eq!(dome.frame(second), PixelRect::new(1, 1, 48, 18));
}

#[test]
fn focus_and_resize_scrolling_monitor_updates_native_frame_and_mirror_crop() {
    let mut dome = TestDome::new(
        r#"
return {
    layout = "scrolling",
    border_size = 0,
    minimum_width = 1,
    minimum_height = 1,
    scrolling = { column_width = "40%" },
    reserved_area = function(monitor) return nil end,
}
"#,
    );
    dome.open("First");
    let second = dome.open("Second");
    dome.open("Third");
    let fourth = dome.open("Fourth");
    {
        let state = dome.overlays();
        let mirror = &state.requested.as_ref().unwrap().mirrors[&second];
        assert_eq!(
            mirror.cocoa_frame,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(20.0, 20.0))
        );
        assert_eq!(mirror.source, PixelRect::new(20, 0, 20, 20).to_dimension());
        assert_eq!(mirror.scale, 1.0);
        assert!(matches!(mirror.level, WindowLevel::Bottom));
    }
    assert_eq!(dome.frame(second), PixelRect::new(99, 19, 40, 20));
    dome.focus(second);
    assert_eq!(dome.frame(second), PixelRect::new(0, 0, 40, 20));
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .mirrors
            .contains_key(&second)
    );
    dome.focus(fourth);
    assert!(
        dome.overlays()
            .requested
            .as_ref()
            .unwrap()
            .mirrors
            .contains_key(&second)
    );
    let mut monitor = monitor(1, 0.0);
    monitor.width = 120.0;
    monitor.height = 30.0;
    monitor.backing_scale = 2.0;
    dome.monitors_changed(vec![monitor]);
    {
        let state = dome.overlays();
        let mirror = &state.requested.as_ref().unwrap().mirrors[&second];
        assert_eq!(
            mirror.cocoa_frame,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(24.0, 30.0))
        );
        assert_eq!(mirror.source, PixelRect::new(24, 0, 24, 30).to_dimension());
        assert_eq!(mirror.scale, 2.0);
    }
    assert_eq!(dome.frame(second), PixelRect::new(119, 29, 48, 30));
    dome.remove(second);
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .mirrors
            .contains_key(&second)
    );
}
