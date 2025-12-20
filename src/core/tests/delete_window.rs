use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

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
