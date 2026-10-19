use super::{
    PARTITION_CONFIG,
    fixture::{StackWindow, TestDome, monitor},
};
use crate::platform::windows::ui::{FloatBorderPlacement, FloatBorderState};
use crate::platform::windows::window_server::WindowNotification;
use dome_core::{FractionalLength, PixelRect, WindowId};

#[test]
fn toggle_float_wraps_application_frame_and_tracks_native_movement() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open("Second");
    dome.set_corner_radius(first, 3.0);
    dome.set_corner_radius(second, 4.0);
    dome.focus(second);
    {
        let state = dome.overlays();
        let windows = &state
            .requested
            .as_ref()
            .unwrap()
            .tiling
            .values()
            .next()
            .unwrap()
            .windows;
        assert_eq!(
            windows
                .iter()
                .find(|show| show.placement.id == first)
                .unwrap()
                .corner_radius,
            FractionalLength::new(3.0)
        );
        assert_eq!(
            windows
                .iter()
                .find(|show| show.placement.id == second)
                .unwrap()
                .corner_radius,
            FractionalLength::new(4.0)
        );
    }
    dome.set_corner_radius(first, 6.0);
    dome.run("toggle float");
    {
        let state = dome.overlays();
        let windows = &state
            .requested
            .as_ref()
            .unwrap()
            .tiling
            .values()
            .next()
            .unwrap()
            .windows;
        assert_eq!(windows[0].placement.id, first);
        assert_eq!(windows[0].corner_radius, FractionalLength::new(6.0));
    }

    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(second), PixelRect::new(51, 1, 48, 18));
    let border = resolved_float(&dome, second);
    assert_eq!(border.placement.border_box, PixelRect::new(50, 0, 50, 20));
    assert_eq!(border.corner_radius, FractionalLength::new(4.0));

    dome.set_corner_radius(second, 8.0);
    dome.moved(second, PixelRect::new(10, 4, 35, 12), 1);
    let border = resolved_float(&dome, second);
    assert_eq!(dome.frame(second), PixelRect::new(10, 4, 35, 12));
    assert_eq!(border.placement.border_box, PixelRect::new(9, 3, 37, 14));
    assert_eq!(border.corner_radius, FractionalLength::new(8.0));
}

fn resolved_float(dome: &TestDome, window: WindowId) -> FloatBorderPlacement {
    let FloatBorderState::Visible(placement) = dome.overlays().floats[&window] else {
        panic!("expected visible border");
    };
    placement
}

fn assert_preserved(dome: &TestDome, window: WindowId, previous: FloatBorderPlacement) {
    assert_eq!(resolved_float(dome, window), previous);
}

#[test]
fn start_moving_float_preserves_border_until_removal() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Floating");
    dome.run("toggle float");
    dome.moved(window, PixelRect::new(10, 4, 35, 12), 1);
    let previous = resolved_float(&dome, window);
    dome.deliver_notification(WindowNotification::MovementStarted(window));
    dome.focus(window);
    assert_preserved(&dome, window, previous);
    let mut appearance = dome.dome.appearance.clone();
    appearance.font.size += 2.0;
    dome.dome.apply_appearance(&appearance);
    dome.focus(window);
    assert_eq!(
        dome.overlays().requested.as_ref().unwrap().appearance.font,
        appearance.font
    );
    assert_preserved(&dome, window, previous);
    dome.remove(window);
    assert!(!dome.overlays().floats.contains_key(&window));

    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Tiled");
    dome.deliver_notification(WindowNotification::MovementStarted(window));
    dome.run("toggle float");
    assert!(!dome.overlays().floats.contains_key(&window));
}

#[test]
fn toggle_float_stacks_border_above_tiles_and_parks_below_visible_windows() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first_window = dome.open("First tile");
    let second = dome.open("Second tile");
    let window = dome.open("Floating");
    dome.run("toggle float");
    dome.focus(first_window);
    let overlay = {
        let state = dome.overlays();
        let placements = state.requested.as_ref().unwrap();
        assert_eq!(placements.tiling.len(), 1);
        assert!(placements.tabs.is_empty());
        assert!(placements.thumbnails.is_empty());
        StackWindow::TilingOverlay(*placements.tiling.keys().next().unwrap())
    };
    let first = StackWindow::Application(first_window);
    let second = StackWindow::Application(second);
    let floating = StackWindow::Application(window);
    let border = StackWindow::FloatBorder(window);
    assert_eq!(
        dome.server.stacking_order(),
        vec![floating, border, first, overlay, second]
    );

    dome.focus(window);
    dome.run("toggle float");
    dome.focus(first_window);
    let stack = dome.server.stacking_order();
    assert_eq!(&stack[..2], &[first, overlay]);
    assert_eq!(stack.len(), 4);
    assert!(stack[2..].contains(&floating));
    assert!(stack[2..].contains(&second));
    assert!(!dome.overlays().floats.contains_key(&window));
    assert!(
        !dome
            .overlays()
            .requested
            .as_ref()
            .unwrap()
            .floats
            .contains_key(&window)
    );

    dome.focus(window);
    dome.run("toggle float");
    dome.run("move workspace parked");
    dome.focus(first_window);
    assert_eq!(dome.frame(window).x().value(), -32000);
    assert!(!dome.in_taskbar(window));
    assert_eq!(
        dome.server.stacking_order(),
        vec![first, overlay, second, floating]
    );
}

#[test]
fn toggle_fullscreen_omits_overlays_and_switch_workspace_preserves_exclusive_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    dome.open("Second");
    dome.run("toggle layout");
    let third = dome.open("Third");
    dome.run("toggle float");
    assert!(dome.overlays().floats.contains_key(&third));
    assert!(!dome.overlays().requested.as_ref().unwrap().tabs.is_empty());
    dome.focus(first);
    dome.run("toggle fullscreen");
    assert_eq!(dome.frame(first), PixelRect::new(0, 0, 100, 20));
    assert!(!dome.in_taskbar(third));
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        assert!(requested.tabs.is_empty());
        assert!(requested.floats.is_empty());
        assert!(requested.thumbnails.is_empty());
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
    assert!(dome.in_taskbar(third));

    let mut dome = TestDome::new(PARTITION_CONFIG);
    let exclusive = dome.open("Exclusive");
    dome.change(exclusive, |window| {
        window.state.window_rect = PixelRect::new(0, 0, 100, 20);
    });
    dome.display_changed(vec![monitor(1)], Some(exclusive));
    dome.run("focus workspace other");
    assert_eq!(dome.frame(exclusive), PixelRect::new(0, 0, 100, 20));
    assert!(!dome.minimized(exclusive));
}
