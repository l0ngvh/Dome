use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

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
