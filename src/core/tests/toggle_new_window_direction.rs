use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

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
