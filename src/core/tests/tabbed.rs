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
fn toggle_tabbed_mode_focus_currently_focused_node() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_left();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
fn focus_tab_change_workspace_focus_to_active_tab_window() {
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
}

#[test]
fn focus_tab_change_workspace_focus_to_active_tab_container_focused() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_up();
    hub.focus_left();
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=2.00, w=150.00, h=9.33)
            Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=11.33, w=150.00, h=9.33)
            Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=20.67, w=150.00, h=9.33)
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [C1]                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
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
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W3                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn focus_tab_change_workspace_focus_to_tabbed_container_active_tab_focused() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_up();
    hub.toggle_container_layout();
    hub.focus_left();
    hub.toggle_container_layout();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=4.00, w=150.00, h=26.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=4.00, w=150.00, h=26.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=4.00, w=150.00, h=26.00)
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [C1]                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W1                        |                     [W2]                       |                      W3                         |
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
fn tabbed_container_takes_one_slot() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
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
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_parent();
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=3,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
        )
      )
    )

    ******************************************************************************************************************************************************
    *                 W0                  |                W1                  |                W2                  |               [W3]                 *
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
    *                                                                         W3                                                                         *
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
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_spawn_mode();
    let w4 = hub.insert_tiling();

    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();
    hub.set_focus(w4);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=2.00, w=37.50, h=28.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=112.50, y=2.00, w=37.50, h=28.00)
            )
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [C2]           |
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
    |                                                                         ||                 W3                 |*                W4                 *
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

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, tabbed=true, active_tab=1,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=4.00, w=75.00, h=26.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=75.00, y=4.00, w=75.00, h=26.00)
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
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=125.00, y=2.00, w=25.00, h=28.00)
            )
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [C2]           |
    |                                                                         |+-----------------------+*************************+-----------------------+
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                    W0                                   ||                       |*                       *|                       |
    |                                                                         ||           W3          |*           W4          *|          W5           |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    +-------------------------------------------------------------------------++-----------------------+*************************+-----------------------+
    ");

    hub.focus_prev_tab();
    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=0,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=125.00, y=2.00, w=25.00, h=28.00)
            )
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||         [W1]           |         W2            |          C2            |
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
    |                                                                         |*                                    W1                                   *
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

    hub.focus_next_tab();
    hub.focus_next_tab();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, tabbed=true, active_tab=2,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Horizontal,
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=2.00, w=25.00, h=28.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=125.00, y=2.00, w=25.00, h=28.00)
            )
          )
        )
      )
    )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W2            |         [C2]           |
    |                                                                         |+-----------------------+*************************+-----------------------+
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                    W0                                   ||                       |*                       *|                       |
    |                                                                         ||           W3          |*           W4          *|          W5           |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    |                                                                         ||                       |*                       *|                       |
    +-------------------------------------------------------------------------++-----------------------+*************************+-----------------------+
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.set_focus(w1);

    hub.toggle_direction();
    hub.set_focus(w3);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, tabbed=true, active_tab=1,
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00, direction=Horizontal,
            Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=20.00, w=50.00, h=10.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=20.00, w=50.00, h=10.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=20.00, w=50.00, h=10.00, direction=Vertical,
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=20.00, w=50.00, h=5.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=100.00, y=25.00, w=50.00, h=5.00)
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
    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W4                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W2                       |*                       W3                       *+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W5                       |
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
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.toggle_direction();
    hub.set_focus(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, tabbed=true, active_tab=1,
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
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=10.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=10.00, w=50.00, h=10.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=100.00, y=20.00, w=50.00, h=10.00, direction=Horizontal,
              Window(id=WindowId(4), parent=ContainerId(2), x=100.00, y=20.00, w=12.50, h=10.00)
              Window(id=WindowId(5), parent=ContainerId(2), x=112.50, y=20.00, w=12.50, h=10.00)
              Window(id=WindowId(6), parent=ContainerId(2), x=125.00, y=20.00, w=12.50, h=10.00)
              Window(id=WindowId(7), parent=ContainerId(2), x=137.50, y=20.00, w=12.50, h=10.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W2                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W0                       ||                       W1                       |*                       W3                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |**************************************************
    |                                                ||                                                |+----------++-----------++----------++-----------+
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||    W4    ||     W5    ||    W6    ||     W7    |
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||          ||           ||          ||           |
    |                                                ||                                                ||          ||           ||          ||           |
    +------------------------------------------------++------------------------------------------------++----------++-----------++----------++-----------+
    ");
}

#[test]
fn toggle_tabbed_off_fixes_direction_conflict_with_parent() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w1);
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    let w4 = hub.insert_tiling();
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

    hub.set_focus(w4);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=50.00, h=7.50)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=7.50, w=50.00, h=7.50)
            Window(id=WindowId(4), parent=ContainerId(1), x=50.00, y=15.00, w=50.00, h=7.50)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=22.50, w=50.00, h=7.50, direction=Horizontal,
              Window(id=WindowId(5), parent=ContainerId(2), x=50.00, y=22.50, w=16.67, h=7.50)
              Window(id=WindowId(6), parent=ContainerId(2), x=66.67, y=22.50, w=16.67, h=7.50)
              Container(id=ContainerId(3), parent=ContainerId(2), x=83.33, y=22.50, w=16.67, h=7.50, tabbed=true, active_tab=2,
                Window(id=WindowId(7), parent=ContainerId(3), x=83.33, y=24.50, w=16.67, h=5.50)
                Window(id=WindowId(8), parent=ContainerId(3), x=83.33, y=24.50, w=16.67, h=5.50)
                Container(id=ContainerId(4), parent=ContainerId(3), x=83.33, y=24.50, w=16.67, h=5.50, direction=Horizontal,
                  Window(id=WindowId(9), parent=ContainerId(4), x=83.33, y=24.50, w=5.56, h=5.50)
                  Window(id=WindowId(10), parent=ContainerId(4), x=88.89, y=24.50, w=5.56, h=5.50)
                  Window(id=WindowId(11), parent=ContainerId(4), x=94.44, y=24.50, w=5.56, h=5.50)
                )
              )
            )
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W1                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W3                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                       W0                       |**************************************************|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W4                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |**************************************************|                                                |
    |                                                |+---------------++--------------++---------------+|                                                |
    |                                                ||               ||              || W7  |W8  [C4] ||                                                |
    |                                                ||               ||              |+----++---++----+|                                                |
    |                                                ||      W5       ||      W6      ||    ||   ||    ||                                                |
    |                                                ||               ||              || W9 || W1|| W11||                                                |
    |                                                ||               ||              ||    ||   ||    ||                                                |
    +------------------------------------------------++---------------++--------------++----++---++----++------------------------------------------------+
    ")
}

#[test]
fn toggle_tabbed_off_dont_rotate_child_when_its_already_correct() {
    let mut hub = setup();

    // Create horizontal container with 3 windows
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    // Make it tabbed
    hub.toggle_container_layout();

    // Create a vertical nested container in the middle tab
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    // Focus parent and toggle back to split (horizontal)
    hub.focus_parent();
    hub.focus_parent();
    hub.toggle_container_layout();

    // The nested container should stay vertical (not rotated) since it differs from parent
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=50.00, h=15.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=15.00, w=50.00, h=15.00)
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    ******************************************************************************************************************************************************
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                       W1                       ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                |+------------------------------------------------+|                                                *
    *                       W0                       |+------------------------------------------------+|                       W2                       *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                       W3                       ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    *                                                ||                                                ||                                                *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn replace_focus_should_not_change_active_tab_when_not_replacing_focused() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();
    hub.focus_prev_tab();
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling();
    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=0,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=2.00, w=150.00, h=14.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=16.00, w=150.00, h=14.00)
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                      [W0]                       |                      C1                        |                      W2                         |
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

    hub.delete_window(w3);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=0,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
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
fn focus_tab_on_empty_workspace() {
    let mut hub = setup();

    hub.focus_next_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");

    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");
}

#[test]
fn focus_tab_with_float_focused() {
    use crate::core::node::Dimension;
    let mut hub = setup();

    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });

    hub.focus_next_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Float(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00)
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

    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Float(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00)
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
fn focus_tab_without_tabbed_ancestor() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.insert_tiling();

    hub.focus_next_tab();
    assert_snapshot!(snapshot(&hub), @r"
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

    hub.focus_prev_tab();
    assert_snapshot!(snapshot(&hub), @r"
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
fn toggle_container_layout_on_empty_workspace() {
    let mut hub = setup();

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
    )
    ");
}

#[test]
fn toggle_container_layout_on_single_window() {
    let mut hub = setup();

    hub.insert_tiling();
    hub.toggle_container_layout();

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

#[test]
fn toggle_container_layout_with_float_focused() {
    use crate::core::node::Dimension;
    let mut hub = setup();

    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Float(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00)
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
fn toggle_container_layout_in_nested_tabbed_maintain_direction_invariant() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();

    hub.toggle_container_layout();
    let w2 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.set_focus(w1);
    hub.toggle_container_layout();
    // [w2, w3] was still vertical as toggle_container_layout doesn't change child orientation, nor
    // should it do
    hub.set_focus(w2);
    hub.toggle_direction();
    // Now we have 3 containers where original orientation were horizontal, 2 of which got turned
    // into tabbed
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=4.00, w=150.00, h=26.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=4.00, w=150.00, h=26.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=0.00, y=4.00, w=75.00, h=26.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=4.00, w=75.00, h=26.00)
            )
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [C1]                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W1                                     |                                 [C2]                                    |
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
    *                                    W2                                   *|                                    W3                                   |
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

    hub.set_focus(w1);
    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=2.00, w=75.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=2.00, w=75.00, h=28.00, direction=Vertical,
              Window(id=WindowId(2), parent=ContainerId(2), x=75.00, y=2.00, w=75.00, h=14.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=75.00, y=16.00, w=75.00, h=14.00)
            )
          )
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [C1]                                    |
    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                    W1                                   *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W3                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn toggle_tabbed_when_focused_is_inside_child_container() {
    let mut hub = setup();

    hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    // Creating multiple nested container to cover non focused container branch
    hub.set_focus(w1);
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.set_focus(w2);
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.set_focus(w3);
    hub.focus_parent();

    // After this focus will be on w7
    hub.delete_window(w3);

    hub.toggle_container_layout();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=2,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=2.00, w=150.00, h=9.33)
            Window(id=WindowId(4), parent=ContainerId(1), x=0.00, y=11.33, w=150.00, h=9.33)
            Window(id=WindowId(5), parent=ContainerId(1), x=0.00, y=20.67, w=150.00, h=9.33)
          )
          Container(id=ContainerId(2), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, direction=Vertical,
            Window(id=WindowId(2), parent=ContainerId(2), x=0.00, y=2.00, w=150.00, h=9.33)
            Window(id=WindowId(6), parent=ContainerId(2), x=0.00, y=11.33, w=150.00, h=9.33)
            Window(id=WindowId(7), parent=ContainerId(2), x=0.00, y=20.67, w=150.00, h=9.33)
          )
        )
      )
    )

    ******************************************************************************************************************************************************
    *                       W0                        |                      C1                        |                     [C2]                        *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W2                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W6                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W7                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}
