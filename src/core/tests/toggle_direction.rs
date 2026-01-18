use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn toggle_direction_on_focused_container() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_parent();
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
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
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
fn toggle_direction_on_window() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
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
fn toggle_direction_on_window_nested() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=15.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=15.00, w=75.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=15.00, w=75.00, h=15.00)
          )
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
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn toggle_direction_inside_tabbed_only_affects_tabbed_subtree() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(7),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=12.50, h=28.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=112.50, y=2.00, w=12.50, h=28.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=125.00, y=2.00, w=12.50, h=28.00)
              Window(id=WindowId(7), parent=ContainerId(2), x=137.50, y=2.00, w=12.50, h=28.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||      W2        |     W3        |    [C2]       |
    |                                                ||                                                |+----------++-----------++----------+*************
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                       W0                       ||                       W1                       ||          ||           ||          |*           *
    |                                                ||                                                ||    W4    ||     W5    ||    W6    |*     W7    *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    |                                                ||                                                ||          ||           ||          |*           *
    +------------------------------------------------++------------------------------------------------++----------++-----------++----------+*************
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(7),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00, direction=Vertical,
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=50.00, h=7.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=100.00, y=9.00, w=50.00, h=7.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=100.00, y=16.00, w=50.00, h=7.00)
              Window(id=WindowId(7), parent=ContainerId(2), x=100.00, y=23.00, w=50.00, h=7.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||      W2        |     W3        |    [C2]       |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W4                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W5                       |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W1                       |+------------------------------------------------+
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W6                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                       W7                       *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn toggle_direction_skips_nested_tabbed_container() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.set_focus(w1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=125.00, y=2.00, w=25.00, h=28.00)
            )
          )
        )
      )
    )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|      W2        |     W3        |    [C2]       |
    |                                                |*                                                *+-----------------------++-----------------------+
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                       W0                       |*                       W1                       *|                       ||                       |
    |                                                |*                                                *|          W4           ||          W5           |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    |                                                |*                                                *|                       ||                       |
    +------------------------------------------------+**************************************************+-----------------------++-----------------------+
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, tabbed=true, active_tab=2,
            Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=22.00, w=150.00, h=8.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=22.00, w=150.00, h=8.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=22.00, w=150.00, h=8.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=0.00, y=22.00, w=75.00, h=8.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=75.00, y=22.00, w=75.00, h=8.00)
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
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W2                        |                      W3                        |                     [C2]                        |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W4                                   ||                                    W5                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn toggle_direction_inside_tabbed_skips_nested_tabbed() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w1);
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    let w6 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w6);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(6),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=3,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(5), parent=ContainerId(2), x=50.00, y=2.00, w=16.67, h=28.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=66.67, y=2.00, w=16.67, h=28.00)
              Container(id=ContainerId(3), parent=ContainerId(2), x=83.33, y=2.00, w=16.67, h=28.00, tabbed=true, active_tab=2,
                Window(id=WindowId(7), parent=ContainerId(3), x=83.33, y=4.00, w=16.67, h=26.00)
                Window(id=WindowId(8), parent=ContainerId(3), x=83.33, y=4.00, w=16.67, h=26.00)
                Container(id=ContainerId(4), parent=ContainerId(3), x=83.33, y=4.00, w=16.67, h=26.00, direction=Horizontal,
                  Window(id=WindowId(9), parent=ContainerId(4), x=83.33, y=4.00, w=5.56, h=26.00)
                  Window(id=WindowId(10), parent=ContainerId(4), x=88.89, y=4.00, w=5.56, h=26.00)
                  Window(id=WindowId(11), parent=ContainerId(4), x=94.44, y=4.00, w=5.56, h=26.00)
                )
              )
            )
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||    W1      |   W3      |   W4      |  [C2]     ||                                                |
    |                                                |+---------------+****************+---------------+|                                                |
    |                                                ||               |*              *| W7  |W8  [C4] ||                                                |
    |                                                ||               |*              *+----++---++----+|                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                       W0                       ||               |*              *|    ||   ||    ||                       W2                       |
    |                                                ||      W5       |*      W6      *|    ||   ||    ||                                                |
    |                                                ||               |*              *| W9 || W1|| W11||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    |                                                ||               |*              *|    ||   ||    ||                                                |
    +------------------------------------------------++---------------+****************+----++---++----++------------------------------------------------+
    ");

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(6),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=3,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=2.00, w=50.00, h=28.00, direction=Vertical,
              Window(id=WindowId(5), parent=ContainerId(2), x=50.00, y=2.00, w=50.00, h=9.33)
              Window(id=WindowId(6), parent=ContainerId(2), x=50.00, y=11.33, w=50.00, h=9.33)
              Container(id=ContainerId(3), parent=ContainerId(2), x=50.00, y=20.67, w=50.00, h=9.33, tabbed=true, active_tab=2,
                Window(id=WindowId(7), parent=ContainerId(3), x=50.00, y=22.67, w=50.00, h=7.33)
                Window(id=WindowId(8), parent=ContainerId(3), x=50.00, y=22.67, w=50.00, h=7.33)
                Container(id=ContainerId(4), parent=ContainerId(3), x=50.00, y=22.67, w=50.00, h=7.33, direction=Horizontal,
                  Window(id=WindowId(9), parent=ContainerId(4), x=50.00, y=22.67, w=16.67, h=7.33)
                  Window(id=WindowId(10), parent=ContainerId(4), x=66.67, y=22.67, w=16.67, h=7.33)
                  Window(id=WindowId(11), parent=ContainerId(4), x=83.33, y=22.67, w=16.67, h=7.33)
                )
              )
            )
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||    W1      |   W3      |   W4      |  [C2]     ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W5                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |**************************************************|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W0                       |*                                                *|                       W2                       |
    |                                                |*                       W6                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |**************************************************|                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||      W7        |     W8        |    [C4]       ||                                                |
    |                                                |+---------------++--------------++---------------+|                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||      W9       ||      W10     ||       W11     ||                                                |
    |                                                ||               ||              ||               ||                                                |
    |                                                ||               ||              ||               ||                                                |
    +------------------------------------------------++---------------++--------------++---------------++------------------------------------------------+
    ");
}

#[test]
fn toggle_direction_on_empty_workspace() {
    let mut hub = setup();

    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");
}

#[test]
fn toggle_direction_with_float_focused() {
    use crate::core::node::Dimension;
    let mut hub = setup();

    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.toggle_direction();

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

#[test]
fn toggle_direction_on_single_window() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
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
