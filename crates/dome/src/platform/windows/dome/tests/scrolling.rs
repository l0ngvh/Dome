use super::fixture::{TestDome, monitor};
use crate::platform::windows::window_server::WindowOrder;
use dome_core::PixelRect;

const SCROLLING_CONFIG: &str = r#"
return {
    layout = "scrolling",
    border_size = 0,
    minimum_width = 1,
    minimum_height = 1,
    scrolling = { column_width = "40%" },
    reserved_area = function(monitor) return nil end,
}
"#;

#[test]
fn focus_scrolling_window_restores_native_window_and_removal_omits_thumbnail() {
    let mut dome = TestDome::new(SCROLLING_CONFIG);
    let windows = ["First", "Second", "Third", "Fourth"].map(|title| dome.open(title));
    let second = windows[1];
    let fourth = windows[3];
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        let thumbnail = &requested.thumbnails[&second];
        assert_eq!(thumbnail.frame, PixelRect::new(0, 0, 20, 20));
        assert_eq!(thumbnail.source_rect, PixelRect::new(20, 0, 20, 20));
        assert!(
            requested
                .tiling
                .values()
                .all(|tiling| tiling.order == WindowOrder::After(fourth))
        );
    }
    assert_eq!(dome.frame(second), PixelRect::new(-32000, -32000, 40, 20));
    assert_eq!(dome.frame(fourth), PixelRect::new(60, 0, 40, 20));
    for id in &windows[1..3] {
        assert_eq!(dome.order(*id), Some(WindowOrder::After(fourth)));
    }
    assert_eq!(
        dome.frame(windows[0]),
        PixelRect::new(-32000, -32000, 40, 20)
    );
    assert!(!dome.in_taskbar(windows[0]));
    dome.focus(second);
    assert_eq!(dome.frame(second), PixelRect::new(0, 0, 40, 20));
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .thumbnails
            .contains_key(&second)
    );
    dome.focus(fourth);
    assert!(
        dome.overlays()
            .requested
            .as_ref()
            .unwrap()
            .thumbnails
            .contains_key(&second)
    );
    dome.remove(second);
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .thumbnails
            .contains_key(&second)
    );
}

#[test]
fn click_thumbnail_after_monitor_move_restores_source_at_new_origin() {
    let mut dome = TestDome::new(SCROLLING_CONFIG);
    let windows = ["First", "Second", "Third", "Fourth"].map(|title| dome.open(title));
    let source = windows[1];
    let mut moved_monitor = monitor(1);
    moved_monitor.work_area = PixelRect::new(100, -20, 100, 20);
    dome.display_changed(vec![moved_monitor], None);
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        let thumbnail = &requested.thumbnails[&source];
        assert_eq!(thumbnail.frame, PixelRect::new(100, -20, 20, 20));
        assert_eq!(thumbnail.source_rect, PixelRect::new(20, 0, 20, 20));
    }
    assert_eq!(dome.frame(source), PixelRect::new(-32000, -32000, 40, 20));
    assert_eq!(dome.focused(), Some(windows[3]));

    dome.dome.thumbnail_clicked(source);

    assert_eq!(dome.focused(), Some(source));
    assert_eq!(dome.frame(source), PixelRect::new(100, -20, 40, 20));
    assert!(dome.in_taskbar(source));
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .thumbnails
            .contains_key(&source)
    );
}
