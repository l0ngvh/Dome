use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn switch_workspace_attaches_windows_correctly() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());

    hub.focus_workspace(1);

    hub.insert_tiling("W2".into());
    hub.insert_tiling("W3".into());

    hub.focus_workspace(0);

    hub.insert_tiling("W4".into());

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=51.00, y=1.00, w=48.00, h=28.00)
          Window(id=WindowId(4), parent=ContainerId(0), x=101.00, y=1.00, w=48.00, h=28.00)
        )
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=1.00, w=73.00, h=28.00)
          Window(id=WindowId(3), parent=ContainerId(1), x=76.00, y=1.00, w=73.00, h=28.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W0                       ||                       W1                       |*                       W4                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn focus_same_workspace() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    let initial_workspace = hub.current_workspace();
    hub.focus_workspace(0);

    assert_eq!(hub.current_workspace(), initial_workspace);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
      )
    )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}
