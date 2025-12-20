use crate::core::hub::Hub;
use crate::core::node::{Child, Dimension};
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
fn delete_window_removes_from_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    let w2 = hub.insert_window();
    hub.insert_window();

    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=6.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.00, y=0.00, w=6.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn delete_window_removes_parent_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    let w2 = hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");

    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
      )
    )
    ");
}

#[test]
fn delete_all_windows() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    let w2 = hub.insert_window();
    let w3 = hub.insert_window();

    hub.delete_window(w1);
    hub.delete_window(w2);
    hub.delete_window(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");
}

#[test]
fn delete_all_windows_cleanup_unfocused_workspace() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    let w2 = hub.insert_window();

    hub.focus_workspace(1);
    hub.delete_window(w1);
    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(1), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(1), name=1)
    )
    ");
}

#[test]
fn switch_workspace_attaches_windows_correctly() {
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

    hub.focus_workspace(1);

    hub.insert_window();
    hub.insert_window();

    hub.focus_workspace(0);

    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00)
          Window(id=WindowId(4), parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
        )
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=0.00, w=6.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(1), x=6.00, y=0.00, w=6.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_same_workspace() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    let initial_workspace = hub.current_workspace();
    hub.focus_workspace(0);

    assert_eq!(hub.current_workspace(), initial_workspace);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
      )
    )
    ");
}

#[test]
fn toggle_new_window_direction_creates_new_container() {
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
    hub.toggle_new_window_direction();
    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn delete_window_after_orientation_change() {
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
    hub.toggle_new_window_direction();
    let w3 = hub.insert_window();
    hub.delete_window(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn toggle_new_window_direction_in_single_window_workspace_creates_vertical_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn toggle_new_window_direction_in_vertical_container() {
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
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.33, y=0.00, w=6.67, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=3.33, y=0.00, w=6.67, h=5.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=3.33, y=5.00, w=6.67, h=5.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=3.33, y=5.00, w=3.33, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=6.67, y=5.00, w=3.33, h=5.00)
            )
          )
        )
      )
    )
    ");
}

#[test]
fn focus_parent_twice_nested_containers() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create nested containers
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.focus_parent();
    hub.focus_parent();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=5.00, w=5.00, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn focus_parent_twice_single_container() {
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

    hub.focus_parent();
    hub.focus_parent();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn insert_window_after_focusing_parent() {
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

    hub.focus_parent();

    hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
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
fn insert_window_after_focused_container_in_parent() {
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

    // Focus the middle container and toggle direction
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

#[test]
fn clean_up_parent_container_when_only_child_is_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    hub.insert_window();

    // Create new child container
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.focus_parent();
    hub.toggle_new_window_direction();

    // Should be inserted in the root container
    let w4 = hub.insert_window();
    hub.delete_window(w1);
    hub.delete_window(w4);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn delete_focused_window_change_focus_to_previous_window() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    let w2 = hub.insert_window();
    hub.insert_window();
    hub.focus_left();

    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn delete_focused_window_change_focus_to_next_window() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    hub.insert_window();
    hub.focus_left();

    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
      )
    )
    ");
}

#[test]
fn delete_focused_window_focus_last_window_of_preceding_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    let w3 = hub.insert_window();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");

    hub.delete_window(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn delete_focused_window_focus_first_window_of_following_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_left();
    hub.focus_left();

    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn delete_window_when_parent_focused_gives_focus_to_last_child() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    let w1 = hub.insert_window();
    hub.insert_window();
    hub.focus_parent();

    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00)
      )
    )
    ");
}

// TODO: test unfocus then insert new window

#[test]
fn container_replaced_by_child_keeps_position_in_parent() {
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
    let w1 = hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.delete_window(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=4.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=4.00, y=0.00, w=4.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=8.00, y=0.00, w=4.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_left_right_in_horizontal_container() {
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

    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
        )
      )
    )
    ");

    hub.focus_right();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_up_down_in_vertical_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.insert_window();

    hub.focus_up();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");

    hub.focus_down();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");
}

#[test]
fn focus_right_selects_first_child_of_next_container() {
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
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus w0
    hub.focus_left();

    // focus_right should select w2 (first child of first nested container)
    hub.focus_right();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.33, y=0.00, w=6.67, h=10.00, direction=Vertical,
            Container(id=ContainerId(2), parent=ContainerId(1), x=3.33, y=0.00, w=6.67, h=5.00, direction=Horizontal,
              Window(id=WindowId(1), parent=ContainerId(2), x=3.33, y=0.00, w=3.33, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=6.67, y=0.00, w=3.33, h=5.00)
            )
            Window(id=WindowId(2), parent=ContainerId(1), x=3.33, y=5.00, w=6.67, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn focus_left_selects_last_child_of_previous_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, w1] [w2]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // focus_left from w2 should select w1 (last child of previous container)
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_left_from_nested_container_goes_to_grandparent_previous() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, [w1, [w2, w3]]]
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.focus_left();
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.33, y=0.00, w=6.67, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=3.33, y=0.00, w=6.67, h=5.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=3.33, y=5.00, w=6.67, h=5.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=3.33, y=5.00, w=3.33, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=6.67, y=5.00, w=3.33, h=5.00)
            )
          )
        )
      )
    )
    ");
}

#[test]
fn focus_down_from_nested_container_goes_to_grandparent_next() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [[[w0, w1], w2], w3]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_down();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=6.67, direction=Horizontal,
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=6.67, direction=Vertical,
              Window(id=WindowId(0), parent=ContainerId(2), x=0.00, y=0.00, w=5.00, h=3.33)
              Window(id=WindowId(3), parent=ContainerId(2), x=0.00, y=3.33, w=5.00, h=3.33)
            )
            Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=6.67)
          )
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");
}

#[test]
fn focus_right_from_last_child_goes_to_next_sibling_in_parent() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, w1] [w2]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus w1 (last in nested container)
    hub.focus_left();

    // focus_right from w1 should go to w2 (next sibling in parent)
    hub.focus_right();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_down_into_horizontal_nested_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.insert_window();

    // Focus window 0 (top)
    hub.focus_up();
    hub.focus_up();

    hub.focus_down();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=5.00, w=3.33, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=3.33, y=5.00, w=3.33, h=5.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=6.67, y=5.00, w=3.33, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn focus_left_at_boundary_does_nothing() {
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
    hub.focus_left();
    hub.focus_left(); // Already at leftmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_right_at_boundary_does_nothing() {
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
    hub.focus_right(); // Already at rightmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_up_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.focus_up(); // Already at topmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn focus_down_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_down(); // Already at bottommost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn window_with_border() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 2.0);
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=2.00, y=2.00, w=6.00, h=6.00)
      )
    )");
}

#[test]
fn border_with_nested_containers() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 12.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 1.0);
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=12.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=12.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=1.00, h=8.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=4.00, y=1.00, w=1.00, h=8.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=7.00, y=1.00, w=1.00, h=8.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=9.00, y=0.00, w=3.00, h=10.00, direction=Vertical,
            Window(id=WindowId(3), parent=ContainerId(1), x=10.00, y=1.00, w=1.00, h=3.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=10.00, y=6.00, w=1.00, h=3.00)
          )
        )
      )
    )
    ");
}

fn snapshot(hub: &Hub) -> String {
    let mut s = format!(
        "Hub(focused={}, screen=(x={:.2} y={:.2} w={:.2} h={:.2}),\n",
        hub.current_workspace(),
        hub.screen().x,
        hub.screen().y,
        hub.screen().width,
        hub.screen().height
    );
    for (workspace_id, workspace) in hub.all_workspaces() {
        let focused = if let Some(current) = workspace.focused {
            format!(", focused={}", current)
        } else {
            String::new()
        };
        if workspace.root().is_none() {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{})\n",
                workspace_id, workspace.name, focused
            ));
        } else {
            s.push_str(&format!(
                "  Workspace(id={}, name={}{},\n",
                workspace_id, workspace.name, focused
            ));
            fmt_child_str(hub, &mut s, workspace.root().unwrap(), 2);
            s.push_str("  )\n");
        }
    }
    s.push_str(")\n");
    s
}

fn fmt_child_str(hub: &Hub, s: &mut String, child: Child, indent: usize) {
    let prefix = "  ".repeat(indent);
    match child {
        Child::Window(id) => {
            let w = hub.get_window(id);
            let dim = w.dimension();
            s.push_str(&format!(
                "{}Window(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2})\n",
                prefix, id, w.parent, dim.x, dim.y, dim.width, dim.height
            ));
        }
        Child::Container(id) => {
            let c = hub.get_container(id);
            s.push_str(&format!(
                    "{}Container(id={}, parent={}, x={:.2}, y={:.2}, w={:.2}, h={:.2}, direction={:?},\n",
                    prefix,
                    id,
                    c.parent,
                    c.dimension.x,
                    c.dimension.y,
                    c.dimension.width,
                    c.dimension.height,
                    c.direction,
                ));
            for &child in c.children() {
                fmt_child_str(hub, s, child, indent + 1);
            }
            s.push_str(&format!("{})\n", prefix));
        }
    }
}

fn setup_logger() {
    use tracing_subscriber::fmt::format::FmtSpan;
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}
