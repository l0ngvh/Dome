use super::fixture::{PARTITION_CONFIG, TestDome, monitor};
use dome_core::PixelRect;

#[test]
fn click_float_mirror_restores_focus_and_parks_previous_float() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    dome.run("toggle float");
    let first_frame = dome.frame(first);
    let second = dome.open("Second");
    dome.run("toggle float");
    let second_frame = dome.frame(second);
    assert!(
        dome.overlays()
            .requested
            .as_ref()
            .unwrap()
            .mirrors
            .contains_key(&first)
    );

    dome.mirror_click(first);
    assert_eq!(dome.focused(), Some(first));
    assert_eq!(dome.frame(first), first_frame);
    assert_eq!(
        dome.frame(second),
        PixelRect::new(
            99,
            19,
            second_frame.width().value(),
            second_frame.height().value()
        )
    );
    {
        let overlays = dome.overlays();
        let mirrors = &overlays.requested.as_ref().unwrap().mirrors;
        assert!(!mirrors.contains_key(&first));
        assert!(mirrors.contains_key(&second));
    }

    dome.mirror_click(second);
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(second), second_frame);
    assert_eq!(
        dome.frame(first),
        PixelRect::new(
            99,
            19,
            first_frame.width().value(),
            first_frame.height().value()
        )
    );
    let overlays = dome.overlays();
    let mirrors = &overlays.requested.as_ref().unwrap().mirrors;
    assert!(mirrors.contains_key(&first));
    assert!(!mirrors.contains_key(&second));
}

#[test]
fn focus_workspace_restores_its_window_and_hides_previous_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace first");
    let first = dome.open("First");
    dome.run("focus workspace second");
    let second = dome.open("Second");
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(first), PixelRect::new(99, 19, 98, 18));

    dome.run("focus workspace first");
    assert_eq!(dome.focused(), Some(first));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(second), PixelRect::new(99, 19, 98, 18));

    dome.run("focus workspace second");
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(first), PixelRect::new(99, 19, 98, 18));
    assert_eq!(dome.frame(second), PixelRect::new(1, 1, 98, 18));
}

#[test]
fn focus_empty_workspace_and_monitor_targets_their_tiling_overlay() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("First");
    dome.run("focus workspace another-empty");
    assert_eq!(dome.frame(window), PixelRect::new(99, 19, 98, 18));
    {
        let state = dome.overlays();
        let requested = state.requested.as_ref().unwrap();
        let monitor = *requested.tiling.keys().next().unwrap();
        assert_eq!(requested.focus_tiling, Some(monitor));
    }
    dome.monitors_changed(vec![monitor(1, 0.0), monitor(2, 100.0)]);
    dome.run("focus monitor right");
    let state = dome.overlays();
    let requested = state.requested.as_ref().unwrap();
    let right = requested
        .tiling
        .iter()
        .find(|(_, placement)| {
            PixelRect::from_dimension(placement.monitor) == PixelRect::new(100, 0, 100, 20)
        })
        .map(|(id, _)| *id)
        .unwrap();
    assert_eq!(requested.focus_tiling, Some(right));
}

#[test]
fn focus_when_minimized_waits_for_movement() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open("Second");
    dome.update_window(second, |window| {
        window.minimized = true;
        window.rect = Some(PixelRect::new(5, 5, 40, 10));
    });
    assert!(dome.minimized(second));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));

    dome.focus(second);
    assert!(!dome.minimized(second));
    assert_eq!(dome.frame(second), PixelRect::new(5, 5, 40, 10));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));

    dome.moved(second, PixelRect::new(5, 5, 40, 10));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(second), PixelRect::new(51, 1, 48, 18));
}
