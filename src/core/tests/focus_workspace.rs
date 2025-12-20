use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

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
