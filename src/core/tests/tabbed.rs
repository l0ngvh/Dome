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
