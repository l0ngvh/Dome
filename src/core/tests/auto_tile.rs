use crate::core::tests::{setup_with_auto_tile, snapshot};
use insta::assert_snapshot;

#[test]
fn auto_tile_sets_horizontal_spawn_mode_when_width_greater_than_height() {
    let mut hub = setup_with_auto_tile();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
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
    |                       W0                       ||                       W1                       |*                       W2                       *
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
fn auto_tile_sets_vertical_spawn_mode_when_height_greater_than_width() {
    let mut hub = setup_with_auto_tile();
    // Going on a round trip to ensure that we can always create a horizontal container with 6
    // direct children, as the auto tile logic can get confused when width is approximately equal
    // to height, due to floating precision lost
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_direction();
    // Each window is 25x30, height > width, so spawn mode should be vertical
    hub.set_focus(w0);
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(6),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=0.00, w=25.00, h=30.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(1), x=0.00, y=0.00, w=25.00, h=15.00)
            Window(id=WindowId(6), parent=ContainerId(1), x=0.00, y=15.00, w=25.00, h=15.00)
          )
          Window(id=WindowId(1), parent=ContainerId(0), x=25.00, y=0.00, w=25.00, h=30.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=50.00, y=0.00, w=25.00, h=30.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=75.00, y=0.00, w=25.00, h=30.00)
          Window(id=WindowId(4), parent=ContainerId(0), x=100.00, y=0.00, w=25.00, h=30.00)
          Window(id=WindowId(5), parent=ContainerId(0), x=125.00, y=0.00, w=25.00, h=30.00)
        )
      )
    )

    +-----------------------++-----------------------++-----------------------++-----------------------++-----------------------++-----------------------+
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |           W0          ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    +-----------------------+|                       ||                       ||                       ||                       ||                       |
    *************************|           W1          ||           W2          ||          W3           ||          W4           ||          W5           |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *           W6          *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *                       *|                       ||                       ||                       ||                       ||                       |
    *************************+-----------------------++-----------------------++-----------------------++-----------------------++-----------------------+
    ");
}

#[test]
fn auto_tile_preserves_tab_spawn_mode() {
    let mut hub = setup_with_auto_tile();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                W1                  |               [W2]                 |
    |                                                                         |***************************************************************************
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                    W2                                   *
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
fn auto_tile_adjusts_after_toggle_direction() {
    let mut hub = setup_with_auto_tile();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_direction();
    hub.set_focus(w0);
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(0), parent=ContainerId(1), x=0.00, y=0.00, w=75.00, h=10.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=75.00, y=0.00, w=75.00, h=10.00)
          )
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00)
        )
      )
    )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn auto_tile_with_tab_spawn_mode() {
    let mut hub = setup_with_auto_tile();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=2,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                      W1                        |                     [W2]                        |
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
    *                                                                         W2                                                                         *
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

#[test]
fn auto_tile_preserves_tab_spawn_mode_on_nested_container_on_delete() {
    let mut hub = setup_with_auto_tile();
    hub.insert_tiling();
    hub.insert_tiling();
    let w2 = hub.insert_tiling();
    hub.focus_left();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                W1                  |               [W3]                 *
    |                                                                         |*-------------------------------------------------------------------------*
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                    W3                                   *
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
    hub.insert_tiling();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W3            |         [W2]           |
    |                                                                         |***************************************************************************
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                    W2                                   *
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
