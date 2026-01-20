use super::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn move_left_from_vertical_container_to_horizontal_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=15.00, w=50.00, h=15.00)
          )
        )
      )
    )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W1                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W0                       |*                       W3                       *+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_right_from_vertical_container_to_horizontal_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=50.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=50.00, y=15.00, w=50.00, h=15.00)
          )
          Window(id=WindowId(3), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
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
    |                                                ||                       W1                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                |+------------------------------------------------+*                                                *
    |                       W0                       |+------------------------------------------------+*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                       W2                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_up_from_horizontal_container_to_vertical_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=20.00, w=75.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=20.00, w=75.00, h=10.00)
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_down_from_horizontal_container_to_vertical_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=10.00, w=75.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=10.00, w=75.00, h=10.00)
          )
          Window(id=WindowId(3), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_left_from_vertical_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(0), parent=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=10.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=10.00, w=75.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=75.00, y=20.00, w=75.00, h=10.00)
          )
        )
      )
    )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W3                                   *|                                    W1                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_left_from_vertical_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
    )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_right_from_vertical_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=15.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=15.00, w=75.00, h=15.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00)
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn move_right_from_vertical_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
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
fn move_up_from_horizontal_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=0.00, w=150.00, h=15.00)
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=15.00, w=75.00, h=15.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=15.00, w=75.00, h=15.00)
          )
        )
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
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_up_from_horizontal_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
        )
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
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
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
    ");
}

#[test]
fn move_down_from_horizontal_container_creates_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=150.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=15.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=15.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00)
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
    |                                    W0                                   ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_down_from_horizontal_container_replaces_new_root_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
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
fn move_right_at_edge_goes_to_horizontal_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=50.00, h=15.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=15.00, w=50.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=50.00, y=15.00, w=25.00, h=15.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=15.00, w=25.00, h=15.00)
            )
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
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
    |                                                ||                       W1                       |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                |+------------------------------------------------+*                                                *
    |                       W0                       |+-----------------------++-----------------------+*                       W4                       *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||           W2          ||          W3           |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    |                                                ||                       ||                       |*                                                *
    +------------------------------------------------++-----------------------++-----------------------+**************************************************
    ");
}

#[test]
fn move_left_at_edge_goes_to_horizontal_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_left();
    hub.focus_left();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=15.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=15.00, w=50.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=100.00, y=15.00, w=25.00, h=15.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=125.00, y=15.00, w=25.00, h=15.00)
            )
          )
        )
      )
    )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W1                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W0                       |*                       W2                       *+-----------------------++-----------------------+
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|          W3           ||          W4           |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    +------------------------------------------------+**************************************************+-----------------------++-----------------------+
    ");
}

#[test]
fn move_down_at_edge_goes_to_vertical_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=10.00, w=75.00, h=10.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=10.00, w=75.00, h=10.00, direction=Vertical,
              Window(id=WindowId(2), parent=ContainerId(2), x=75.00, y=10.00, w=75.00, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=15.00, w=75.00, h=5.00)
            )
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W4                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_up_at_edge_goes_to_vertical_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_up();
    hub.focus_up();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=20.00, w=75.00, h=10.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=20.00, w=75.00, h=10.00, direction=Vertical,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=20.00, w=75.00, h=5.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=75.00, y=25.00, w=75.00, h=5.00)
            )
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_does_nothing_for_single_window() {
    let mut hub = setup();
    hub.insert_tiling();
    let before = snapshot(&hub);
    hub.move_left();
    hub.move_right();
    hub.move_up();
    hub.move_down();
    assert_eq!(before, snapshot(&hub));
}

#[test]
fn swap_left_in_horizontal_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
    )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn swap_right_in_horizontal_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_left();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
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
    |                                    W1                                   |*                                    W0                                   *
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
fn swap_up_in_vertical_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
        )
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
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
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
    ");
}

#[test]
fn swap_down_in_vertical_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.focus_up();

    hub.move_down();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
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
    |                                                                         W1                                                                         |
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
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_from_tabbed_parent_goes_to_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_prev_tab();

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=0,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||         [W1]           |         W3            |*                                                *
    |                                                |+------------------------------------------------+*                                                *
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
    |                       W0                       ||                                                |*                       W2                       *
    |                                                ||                       W1                       |*                                                *
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
fn move_from_nested_container_skip_tabbed_grandparent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=75.00, y=2.00, w=37.50, h=28.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=112.50, y=2.00, w=37.50, h=28.00)
            )
            Window(id=WindowId(3), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |        [C2]           |          W3            |
    |                                                                         |+------------------------------------+*************************************
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                    W0                                   ||                                    |*                                   *
    |                                                                         ||                 W2                 |*                W4                 *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    |                                                                         ||                                    |*                                   *
    +-------------------------------------------------------------------------++------------------------------------+*************************************
    ");

    hub.move_right();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||      W1        |    [W2]       |     W3        |*                                                *
    |                                                |+------------------------------------------------+*                                                *
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
    |                       W0                       ||                                                |*                       W4                       *
    |                                                ||                       W2                       |*                                                *
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
fn move_container_up_toggles_direction_when_matching_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.focus_parent();

    hub.move_up();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Container(id=ContainerId(2), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(3), parent=ContainerId(2), x=0.00, y=10.00, w=75.00, h=10.00)
            Window(id=WindowId(4), parent=ContainerId(2), x=75.00, y=10.00, w=75.00, h=10.00)
          )
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=20.00, w=75.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=20.00, w=75.00, h=10.00)
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W3                                   ||                                    W4                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_container_left_toggles_direction_when_matching_parent() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.focus_parent();

    hub.move_left();
    insta::assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(2), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(3), parent=ContainerId(2), x=50.00, y=0.00, w=50.00, h=15.00)
            Window(id=WindowId(4), parent=ContainerId(2), x=50.00, y=15.00, w=50.00, h=15.00)
          )
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=15.00, w=50.00, h=15.00)
          )
        )
      )
    )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W3                       *|                       W1                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*------------------------------------------------*+------------------------------------------------+
    |                       W0                       |*------------------------------------------------*+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W4                       *|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_in_direction_on_empty_workspace() {
    let mut hub = setup();

    hub.move_left();
    hub.move_right();
    hub.move_up();
    hub.move_down();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");
}

#[test]
fn move_in_direction_with_float_focused() {
    use crate::core::node::Dimension;
    let mut hub = setup();

    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });

    hub.move_left();
    hub.move_right();
    hub.move_up();
    hub.move_down();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=FloatWindowId(0),
        Float(id=FloatWindowId(0), x=10.00, y=5.00, w=30.00, h=20.00)
      )
    )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}
