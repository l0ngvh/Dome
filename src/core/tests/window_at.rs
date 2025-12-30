use crate::core::allocator::NodeId;
use crate::core::hub::Hub;
use crate::core::node::Dimension;

const BORDER: f32 = 5.0;

fn setup() -> Hub {
    Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        BORDER,
        3.0,
    )
}

#[test]
fn window_at_single_window() {
    let mut hub = setup();
    hub.insert_tiling();

    // Inside window
    assert_eq!(hub.window_at(50.0, 50.0).unwrap().get(), 0);

    // Outside bounds
    assert!(hub.window_at(-1.0, 50.0).is_none());
    assert!(hub.window_at(200.0, 50.0).is_none());
}

#[test]
fn window_at_horizontal_layout() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    // First window
    assert_eq!(hub.window_at(25.0, 50.0).unwrap().get(), 0);

    // Second window
    assert_eq!(hub.window_at(75.0, 50.0).unwrap().get(), 1);
}

#[test]
fn window_at_nested_containers() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();

    // First window (left half)
    assert_eq!(hub.window_at(25.0, 50.0).unwrap().get(), 0);

    // Second window (top right)
    assert_eq!(hub.window_at(75.0, 30.0).unwrap().get(), 1);

    // Third window (bottom right)
    assert_eq!(hub.window_at(75.0, 70.0).unwrap().get(), 2);
}

#[test]
fn window_at_empty_workspace() {
    let hub = setup();
    assert!(hub.window_at(50.0, 50.0).is_none());
}
