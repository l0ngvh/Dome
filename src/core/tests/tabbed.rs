use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn toggle_tabbed_mode() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=2,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
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
fn focus_prev_next_tab() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_prev_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W0                        |                     [W1]                       |                      W2                         |
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
    *                                                                         W1                                                                         *
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
    hub.focus_next_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=2,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
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
fn focus_next_tab_wrapped() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=0,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn focus_prev_tab_wraps() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.focus_prev_tab();
    hub.focus_prev_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=2,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
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
fn toggle_tabbed_off() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_container_layout();

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
fn tabbed_container_takes_one_slot() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=76.00, y=3.00, w=73.00, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=76.00, y=3.00, w=73.00, h=26.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=76.00, y=3.00, w=73.00, h=26.00)
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [W3]           |
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
}

#[test]
fn vertical_to_tabbed() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_parent();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=0,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
        )
      )
    )

    ******************************************************************************************************************************************************
    *                [W0]                 |                W1                  |                W2                  |                W3                  *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn container_in_tabbed_container() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_spawn_direction();
    hub.insert_tiling();

    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();

    hub.focus_next_tab();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=51.00, y=3.00, w=98.00, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=51.00, y=3.00, w=98.00, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=2.00, w=100.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=51.00, y=3.00, w=48.00, h=26.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=101.00, y=3.00, w=48.00, h=26.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++--------------------------------------------------------------------------------------------------+
    |                                                ||              W1                |             W2                |             [C2]                |
    |                                                |+------------------------------------------------+**************************************************
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
    |                       W0                       ||                                                |*                                                *
    |                                                ||                       W3                       |*                       W4                       *
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

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=76.00, y=3.00, w=73.00, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=76.00, y=3.00, w=73.00, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, tabbed=true, active_tab=1,
              Window(id=WindowId(3), parent=ContainerId(2), x=76.00, y=5.00, w=73.00, h=24.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=76.00, y=5.00, w=73.00, h=24.00)
            )
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [C2]           |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                W3                  |               [W4]                 |
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
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
fn change_tab_shows_container_focus() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=37.50, y=0.00, w=112.50, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=37.50, y=2.00, w=112.50, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=38.50, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=76.00, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=113.50, y=3.00, w=35.50, h=26.00)
            )
          )
        )
      )
    )

    +------------------------------------++--------------------------------------------------------------------------------------------------------------+
    |                                    ||                W1                  |               W2                  |               [C2]                  |
    |                                    |+-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                 W0                 ||                                   |*                                    *|                                   |
    |                                    ||                W3                 |*                 W4                 *|                W5                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");

    hub.focus_prev_tab();
    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=37.50, y=0.00, w=112.50, h=30.00, tabbed=true, active_tab=0,
            Window(id=WindowId(1), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=37.50, y=2.00, w=112.50, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=38.50, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=76.00, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=113.50, y=3.00, w=35.50, h=26.00)
            )
          )
        )
      )
    )

    +------------------------------------++--------------------------------------------------------------------------------------------------------------+
    |                                    ||               [W1]                 |               W2                  |                C2                   |
    |                                    |****************************************************************************************************************
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                 W0                 |*                                                                                                              *
    |                                    |*                                                      W1                                                      *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    |                                    |*                                                                                                              *
    +------------------------------------+****************************************************************************************************************
    ");

    hub.focus_next_tab();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=37.50, y=0.00, w=112.50, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=38.50, y=3.00, w=110.50, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=37.50, y=2.00, w=112.50, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=38.50, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=76.00, y=3.00, w=35.50, h=26.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=113.50, y=3.00, w=35.50, h=26.00)
            )
          )
        )
      )
    )

    +------------------------------------++--------------------------------------------------------------------------------------------------------------+
    |                                    ||                W1                  |               W2                  |               [C2]                  |
    |                                    |+-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                 W0                 ||                                   |*                                    *|                                   |
    |                                    ||                W3                 |*                 W4                 *|                W5                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");
}

#[test]
fn set_focus_updates_active_tab() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    // Focus W0 should update active_tab to 0
    hub.set_focus(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=0,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      W1                        |                      W2                         |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_active_tab_updates_active_tab() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    let w2 = hub.insert_tiling();
    hub.toggle_container_layout();

    // W2 is active (index 2), delete it
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=3.00, w=148.00, h=26.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
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
    *                                                                         W1                                                                         *
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
fn toggle_tabbed_off_fixes_direction_conflict_with_parent_and_children() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    let w3 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.set_focus(w1);

    hub.toggle_direction();
    hub.set_focus(w3);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=148.00, h=8.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=11.00, w=148.00, h=8.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, tabbed=true, active_tab=1,
            Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=23.00, w=148.00, h=6.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=1.00, y=23.00, w=148.00, h=6.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=22.00, w=150.00, h=8.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=1.00, y=23.00, w=73.00, h=6.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=76.00, y=23.00, w=73.00, h=6.00)
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
    |                       W2                        |                     [W3]                       |                      C2                         |
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");

    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=148.00, h=5.50)
          Window(id=WindowId(1), parent=ContainerId(0), x=1.00, y=8.50, w=148.00, h=5.50)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=16.00, w=48.00, h=13.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=51.00, y=16.00, w=48.00, h=13.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=15.00, w=50.00, h=15.00, direction=Vertical,
              Window(id=WindowId(4), parent=ContainerId(2), x=101.00, y=16.00, w=48.00, h=5.50)
              Window(id=WindowId(5), parent=ContainerId(2), x=101.00, y=23.50, w=48.00, h=5.50)
            )
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W4                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W2                       |*                       W3                       *+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W5                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn toggle_tabbed_off_fixes_direction_conflict_with_children() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    let w3 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_direction();
    hub.set_focus(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=51.00, y=1.00, w=48.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(2), parent=ContainerId(1), x=101.00, y=3.00, w=48.00, h=26.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=101.00, y=3.00, w=48.00, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=2.00, w=50.00, h=28.00, direction=Vertical,
              Window(id=WindowId(4), parent=ContainerId(2), x=101.00, y=3.00, w=48.00, h=5.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=101.00, y=10.00, w=48.00, h=5.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=101.00, y=17.00, w=48.00, h=5.00)
              Window(id=WindowId(7), parent=ContainerId(2), x=101.00, y=24.00, w=48.00, h=5.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||      W2        |    [W3]       |     C2        |
    |                                                ||                                                |**************************************************
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
    |                       W0                       ||                       W1                       |*                                                *
    |                                                ||                                                |*                       W3                       *
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

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=23.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=26.00, y=1.00, w=23.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00, direction=Vertical,
            Window(id=WindowId(2), parent=ContainerId(1), x=51.00, y=1.00, w=98.00, h=8.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=51.00, y=11.00, w=98.00, h=8.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=20.00, w=100.00, h=10.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=51.00, y=21.00, w=23.00, h=8.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=76.00, y=21.00, w=23.00, h=8.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=101.00, y=21.00, w=23.00, h=8.00)
              Window(id=WindowId(7), parent=ContainerId(2), x=126.00, y=21.00, w=23.00, h=8.00)
            )
          )
        )
      )
    )

    +-----------------------++-----------------------++--------------------------------------------------------------------------------------------------+
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                W2                                                |
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                                                                  |
    |                       ||                       ||                                                                                                  |
    |                       ||                       |+--------------------------------------------------------------------------------------------------+
    |                       ||                       |****************************************************************************************************
    |                       ||                       |*                                                                                                  *
    |                       ||                       |*                                                                                                  *
    |                       ||                       |*                                                                                                  *
    |                       ||                       |*                                                                                                  *
    |           W0          ||           W1          |*                                                W3                                                *
    |                       ||                       |*                                                                                                  *
    |                       ||                       |*                                                                                                  *
    |                       ||                       |*                                                                                                  *
    |                       ||                       |****************************************************************************************************
    |                       ||                       |+-----------------------++-----------------------++-----------------------++-----------------------+
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||           W4          ||           W5          ||           W6          ||           W7          |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    |                       ||                       ||                       ||                       ||                       ||                       |
    +-----------------------++-----------------------++-----------------------++-----------------------++-----------------------++-----------------------+
    ");
}

#[test]
fn toggle_tabbed_off_fixes_direction_conflict_with_parent() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w1);
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    let w4 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_direction();
    let w6 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w6);
    hub.toggle_direction();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(6),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=28.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=30.00, y=0.00, w=90.00, h=30.00, tabbed=true, active_tab=3,
            Window(id=WindowId(1), parent=ContainerId(1), x=31.00, y=3.00, w=88.00, h=26.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=31.00, y=3.00, w=88.00, h=26.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=31.00, y=3.00, w=88.00, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=30.00, y=2.00, w=90.00, h=28.00, direction=Vertical,
              Window(id=WindowId(5), parent=ContainerId(2), x=31.00, y=3.00, w=88.00, h=7.33)
              Window(id=WindowId(6), parent=ContainerId(2), x=31.00, y=12.33, w=88.00, h=7.33)
              Container(id=ContainerId(3), parent=ContainerId(2), x=30.00, y=20.67, w=90.00, h=9.33, tabbed=true, active_tab=2,
                Window(id=WindowId(7), parent=ContainerId(3), x=31.00, y=23.67, w=88.00, h=5.33)
                Window(id=WindowId(8), parent=ContainerId(3), x=31.00, y=23.67, w=88.00, h=5.33)
                Container(id=ContainerId(4), parent=ContainerId(3), x=30.00, y=22.67, w=90.00, h=7.33, direction=Horizontal,
                  Window(id=WindowId(9), parent=ContainerId(4), x=31.00, y=23.67, w=28.00, h=5.33)
                  Window(id=WindowId(10), parent=ContainerId(4), x=61.00, y=23.67, w=28.00, h=5.33)
                  Window(id=WindowId(11), parent=ContainerId(4), x=91.00, y=23.67, w=28.00, h=5.33)
                )
              )
            )
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=121.00, y=1.00, w=28.00, h=28.00)
        )
      )
    )

    +----------------------------++----------------------------------------------------------------------------------------++----------------------------+
    |                            ||         W1           |        W3           |        W4           |       [C2]          ||                            |
    |                            |+----------------------------------------------------------------------------------------+|                            |
    |                            ||                                                                                        ||                            |
    |                            ||                                                                                        ||                            |
    |                            ||                                                                                        ||                            |
    |                            ||                                                                                        ||                            |
    |                            ||                                           W5                                           ||                            |
    |                            ||                                                                                        ||                            |
    |                            ||                                                                                        ||                            |
    |                            |+----------------------------------------------------------------------------------------+|                            |
    |                            |******************************************************************************************|                            |
    |                            |*                                                                                        *|                            |
    |                            |*                                                                                        *|                            |
    |                            |*                                                                                        *|                            |
    |             W0             |*                                                                                        *|             W2             |
    |                            |*                                           W6                                           *|                            |
    |                            |*                                                                                        *|                            |
    |                            |*                                                                                        *|                            |
    |                            |*                                                                                        *|                            |
    |                            |******************************************************************************************|                            |
    |                            |+----------------------------------------------------------------------------------------+|                            |
    |                            ||             W7              |            W8              |           [C4]              ||                            |
    |                            |+----------------------------++----------------------------++----------------------------+|                            |
    |                            ||                            ||                            ||                            ||                            |
    |                            ||                            ||                            ||                            ||                            |
    |                            ||             W9             ||             W10            ||             W11            ||                            |
    |                            ||                            ||                            ||                            ||                            |
    |                            ||                            ||                            ||                            ||                            |
    +----------------------------++----------------------------++----------------------------++----------------------------++----------------------------+
    ");

    hub.set_focus(w4);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=19.43, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=21.43, y=0.00, w=107.14, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=22.43, y=1.00, w=105.14, h=5.50)
            Window(id=WindowId(3), parent=ContainerId(1), x=22.43, y=8.50, w=105.14, h=5.50)
            Window(id=WindowId(4), parent=ContainerId(1), x=22.43, y=16.00, w=105.14, h=5.50)
            Container(id=ContainerId(2), parent=ContainerId(1), x=21.43, y=22.50, w=107.14, h=7.50, direction=Horizontal,
              Window(id=WindowId(5), parent=ContainerId(2), x=22.43, y=23.50, w=19.43, h=5.50)
              Window(id=WindowId(6), parent=ContainerId(2), x=43.86, y=23.50, w=19.43, h=5.50)
              Container(id=ContainerId(3), parent=ContainerId(2), x=64.29, y=22.50, w=64.29, h=7.50, tabbed=true, active_tab=2,
                Window(id=WindowId(7), parent=ContainerId(3), x=65.29, y=25.50, w=62.29, h=3.50)
                Window(id=WindowId(8), parent=ContainerId(3), x=65.29, y=25.50, w=62.29, h=3.50)
                Container(id=ContainerId(4), parent=ContainerId(3), x=64.29, y=24.50, w=64.29, h=5.50, direction=Horizontal,
                  Window(id=WindowId(9), parent=ContainerId(4), x=65.29, y=25.50, w=19.43, h=3.50)
                  Window(id=WindowId(10), parent=ContainerId(4), x=86.71, y=25.50, w=19.43, h=3.50)
                  Window(id=WindowId(11), parent=ContainerId(4), x=108.14, y=25.50, w=19.43, h=3.50)
                )
              )
            )
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=129.57, y=1.00, w=19.43, h=28.00)
        )
      )
    )

    +-------------------++----------------------------------------------------------------------------------------------------------++-------------------+
    |                   ||                                                                                                          ||                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                    W1                                                    ||                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                                                                          ||                   |
    |                   |+----------------------------------------------------------------------------------------------------------+|                   |
    |                   |+----------------------------------------------------------------------------------------------------------+|                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                    W3                                                    ||                   |
    |                   ||                                                                                                          ||                   |
    |                   ||                                                                                                          ||                   |
    |                   |+----------------------------------------------------------------------------------------------------------+|                   |
    |         W0        |************************************************************************************************************|        W2         |
    |                   |*                                                                                                          *|                   |
    |                   |*                                                                                                          *|                   |
    |                   |*                                                                                                          *|                   |
    |                   |*                                                    W4                                                    *|                   |
    |                   |*                                                                                                          *|                   |
    |                   |*                                                                                                          *|                   |
    |                   |************************************************************************************************************|                   |
    |                   |+--------------------++-------------------++---------------------------------------------------------------+|                   |
    |                   ||                    ||                   ||         W7          |        W8          |       [C4]         ||                   |
    |                   ||                    ||                   |+--------------------++-------------------++--------------------+|                   |
    |                   ||         W5         ||         W6        ||                    ||                   ||                    ||                   |
    |                   ||                    ||                   ||         W9         ||        W10        ||         W11        ||                   |
    |                   ||                    ||                   ||                    ||                   ||                    ||                   |
    +-------------------++--------------------++-------------------++--------------------++-------------------++--------------------++-------------------+
    ")
}
