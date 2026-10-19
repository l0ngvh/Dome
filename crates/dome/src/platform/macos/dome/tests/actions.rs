use super::fixture::{PARTITION_CONFIG, TestDome};
use dome_core::{FractionalLength, PixelRect};
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[test]
fn toggle_float_move_and_reload_keeps_outer_border() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open_with("Second", |window| {
        window.corner_radius = Some(FractionalLength::new(4.5));
    });
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(second), PixelRect::new(51, 1, 48, 18));

    dome.run("toggle float");
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(second), PixelRect::new(51, 1, 48, 18));
    assert_eq!(
        dome.overlays().floats[&second].cocoa_frame,
        NSRect::new(NSPoint::new(50.0, 0.0), NSSize::new(50.0, 20.0))
    );

    assert_eq!(
        dome.overlays().floats[&second].corner_radius,
        FractionalLength::new(4.5)
    );
    dome.moved(second, PixelRect::new(30, 4, 40, 10));
    assert_eq!(dome.frame(second), PixelRect::new(30, 4, 40, 10));
    assert_eq!(
        dome.overlays().floats[&second].cocoa_frame,
        NSRect::new(NSPoint::new(29.0, 5.0), NSSize::new(42.0, 12.0))
    );

    dome.reload_config(&PARTITION_CONFIG.replace("border_size = 1", "border_size = 2"));
    assert_eq!(dome.frame(second), PixelRect::new(31, 5, 38, 8));
    assert_eq!(
        dome.overlays().floats[&second].cocoa_frame,
        NSRect::new(NSPoint::new(29.0, 5.0), NSSize::new(42.0, 12.0))
    );

    dome.run("toggle float");
    assert!(dome.overlays().floats.is_empty());
    assert_eq!(dome.frame(first), PixelRect::new(2, 2, 46, 16));
    assert_eq!(dome.frame(second), PixelRect::new(52, 2, 46, 16));
}

#[test]
fn toggle_fullscreen_hides_tabs_and_floats() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    dome.open("Second");
    let third = dome.open("Third");
    dome.focus(third);
    dome.run("toggle float");
    dome.focus(first);
    dome.run("toggle layout");
    assert!(!dome.overlays().requested.as_ref().unwrap().tabs.is_empty());
    assert!(dome.overlays().floats.contains_key(&third));
    dome.run("toggle fullscreen");
    assert_eq!(dome.frame(first), PixelRect::new(0, 0, 100, 20));
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        assert!(requested.tabs.is_empty());
        assert!(requested.floats.is_empty());
        assert!(requested.mirrors.is_empty());
        assert!(state.floats.is_empty());
        assert!(!requested.tiling.is_empty());
        assert!(
            requested
                .tiling
                .values()
                .all(|tiling| tiling.windows.is_empty() && tiling.containers.is_empty())
        );
    }
    dome.run("toggle fullscreen");
    assert!(dome.overlays().floats.contains_key(&third));
}

#[test]
fn discover_waits_for_accessibility_and_rejects_own_process() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let external = dome.open_with("External", |window| window.size_settable = false);
    let own = dome.open_with("Dome", |window| window.pid = std::process::id() as i32);
    assert_eq!(dome.frame(external), PixelRect::new(5, 5, 40, 10));
    assert_eq!(dome.frame(own), PixelRect::new(5, 5, 40, 10));
    dome.update_window(external, |window| window.size_settable = true);
    assert_eq!(dome.frame(external), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(own), PixelRect::new(5, 5, 40, 10));
}
