use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

#[test]
fn initial_window_cover_full_screen() {
    setup_logger();
    let screen = Dimension {
        x: 2.0,
        y: 1.0,
        width: 20.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=2.00 y=1.00 w=20.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=2.00, y=1.00, w=20.00, h=10.00)
      )
    )
    ");
}

#[test]
fn split_window_evenly() {
    setup_logger();
    let screen = Dimension {
        x: 2.0,
        y: 1.0,
        width: 20.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    for _ in 0..4 {
        hub.insert_window();
    }

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=2.00 y=1.00 w=20.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=2.00, y=1.00, w=20.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=2.00, y=1.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=7.00, y=1.00, w=5.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=12.00, y=1.00, w=5.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=17.00, y=1.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn new_container_preserves_wrapped_window_position() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    // Focus w1 (middle)
    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // New container wrapping w1 should be in the middle position
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=4.00, y=0.00, w=4.00, h=5.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=4.00, y=5.00, w=4.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn insert_window_after_focused_window() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    // Focus w1 (middle)
    hub.focus_left();
    hub.insert_window();

    // w3 should be inserted right after w1, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.00, y=0.00, w=3.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=6.00, y=0.00, w=3.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=9.00, y=0.00, w=3.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn insert_window_after_focused_container_with_different_new_window_direction() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0] [w1, w2] [w3]
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus the middle container
    hub.focus_left();
    hub.focus_parent();
    hub.insert_window();

    // w4 should be inserted right after the focused container, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.00, y=0.00, w=3.00, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=3.00, y=0.00, w=3.00, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=3.00, y=5.00, w=3.00, h=5.00)
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=6.00, y=0.00, w=3.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=9.00, y=0.00, w=3.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn insert_window_after_focused_container_with_same_new_window_direction() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0] [w1, w2] [w3]
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus the middle container and toggle back new window direction
    hub.focus_left();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // w4 should be inserted right after the focused container, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=4.00, y=0.00, w=4.00, h=3.33)
            Window(id=WindowId(2), parent=ContainerId(1), x=4.00, y=3.33, w=4.00, h=3.33)
            Window(id=WindowId(4), parent=ContainerId(1), x=4.00, y=6.67, w=4.00, h=3.33)
          )
          Window(id=WindowId(3), parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn insert_to_new_container_when_focused_container_window_insert_direction_differ_and_no_parent() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.insert_window();

    hub.focus_parent();
    hub.toggle_new_window_direction();

    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00, direction=Horizontal,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=5.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn insert_to_parent_when_focused_container_window_insert_direction_differ_but_has_parent() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Creating [w0, [w1, w2], w3]
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();

    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.focus_parent();
    hub.toggle_new_window_direction();

    // Should be inserted in the root container
    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=2.50, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=2.50, y=0.00, w=2.50, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=2.50, y=0.00, w=2.50, h=5.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=2.50, y=5.00, w=2.50, h=5.00)
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=5.00, y=0.00, w=2.50, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=7.50, y=0.00, w=2.50, h=10.00)
        )
      )
    )
    ");
}

// TODO: test unfocus then insert new window
