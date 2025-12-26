use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn toggle_new_window_direction_creates_new_container() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.toggle_new_window_direction();
    hub.insert_tiling("W2".into());

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=76.00, y=1.00, w=73.00, h=13.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=76.00, y=16.00, w=73.00, h=13.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn delete_window_after_orientation_change() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.toggle_new_window_direction();
    let w2 = hub.insert_tiling("W2".into());
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=76.00, y=1.00, w=73.00, h=13.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=76.00, y=16.00, w=73.00, h=13.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
    hub.delete_window(w2);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=76.00, y=1.00, w=73.00, h=28.00)
        )
      )
    )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn toggle_new_window_direction_in_single_window_workspace_creates_vertical_container() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.toggle_new_window_direction();
    hub.insert_tiling("W1".into());

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=148.00, h=13.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=16.00, w=148.00, h=13.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_new_window_direction_in_vertical_container() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.toggle_new_window_direction();
    hub.insert_tiling("W2".into());
    hub.toggle_new_window_direction();
    hub.insert_tiling("W3".into());

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=51.00, y=1.00, w=98.00, h=13.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=15.00, w=100.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=51.00, y=16.00, w=48.00, h=13.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=101.00, y=16.00, w=48.00, h=13.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++--------------------------------------------------------------------------------------------------+
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                W1                                                |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                |+--------------------------------------------------------------------------------------------------+
    |                       W0                       |+------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                       W2                       |*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}
