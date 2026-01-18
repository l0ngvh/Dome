use insta::assert_snapshot;

use super::{setup, snapshot};

#[test]
fn set_min_size_respects_minimum_width() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                W0                                                |*                       W1                       *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    +--------------------------------------------------------------------------------------------------+**************************************************
    ");
}

#[test]
fn set_min_size_respects_minimum_height() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    hub.set_min_size(w0, 0.0, 20.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=20.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=10.00)
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
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
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
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn set_min_size_distributes_remaining_space_equally() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=25.00, h=30.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=125.00, y=0.00, w=25.00, h=30.00)
        )
      )
    )

    +--------------------------------------------------------------------------------------------------++-----------------------+*************************
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                W0                                                ||          W1           |*          W2           *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    |                                                                                                  ||                       |*                       *
    +--------------------------------------------------------------------------------------------------++-----------------------+*************************
    ");
}

#[test]
fn set_min_size_propagates_to_parent_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling();

    hub.set_min_size(w2, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=100.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=50.00, y=15.00, w=100.00, h=15.00)
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
    |                       W0                       |****************************************************************************************************
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                W2                                                *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    +------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn set_min_size_exceeds_screen_size() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 0.0);
    hub.set_min_size(w1, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=-50.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=-50.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00)
        )
      )
    )

    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    0                                                |*                                                W1                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn set_min_size_exceeds_container_size() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling();

    hub.set_min_size(w2, 100.0, 0.0);
    hub.set_min_size(w3, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=-50.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=-50.00, y=0.00, w=0.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=-50.00, y=0.00, w=200.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=-50.00, y=0.00, w=200.00, h=15.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=-50.00, y=15.00, w=200.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=-50.00, y=15.00, w=100.00, h=15.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=50.00, y=15.00, w=100.00, h=15.00)
            )
          )
        )
      )
    )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                     W1                                                                                                  |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    2                                                |*                                                W3                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn set_min_size_exceeds_container_size_focus_first() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling();

    hub.set_min_size(w0, 50.0, 0.0);
    hub.set_min_size(w2, 100.0, 0.0);
    hub.set_min_size(w3, 100.0, 0.0);
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=250.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=200.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=0.00, w=200.00, h=15.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=15.00, w=200.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=50.00, y=15.00, w=100.00, h=15.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=150.00, y=15.00, w=100.00, h=15.00)
            )
          )
        )
      )
    )

    **************************************************+---------------------------------------------------------------------------------------------------
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                  W
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *+---------------------------------------------------------------------------------------------------
    *                       W0                       *+--------------------------------------------------------------------------------------------------+
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                W2                                                |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    *                                                *|                                                                                                  |
    **************************************************+--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn set_min_size_global_exceeds_screen_size() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.sync_config(2.0, false, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=-50.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=-50.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00)
        )
      )
    )

    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    0                                                |*                                                W1                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn set_min_size_exceeds_screen_height() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();

    hub.set_min_size(w0, 0.0, 20.0);
    hub.set_min_size(w1, 0.0, 20.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=-10.00, w=150.00, h=40.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=-10.00, w=150.00, h=20.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=150.00, h=20.00)
        )
      )
    )

    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn set_min_size_exceeds_screen_height_scroll_to_focus() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();

    hub.set_min_size(w0, 0.0, 20.0);
    hub.set_min_size(w1, 0.0, 20.0);
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=40.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=20.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=20.00)
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
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    ");
}

#[test]
fn set_min_size_tabbed_child_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.toggle_container_layout();
    let w3 = hub.insert_tiling();

    hub.set_min_size(w3, 100.0, 20.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00, tabbed=true, active_tab=1,
            Window(id=WindowId(1), parent=ContainerId(1), x=50.00, y=2.00, w=100.00, h=28.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=50.00, y=2.00, w=100.00, h=28.00, direction=Vertical,
              Window(id=WindowId(2), parent=ContainerId(2), x=50.00, y=2.00, w=100.00, h=8.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=50.00, y=10.00, w=100.00, h=20.00)
            )
          )
        )
      )
    )

    +------------------------------------------------++--------------------------------------------------------------------------------------------------+
    |                                                ||                       W1                        |                     [C2]                       |
    |                                                |+--------------------------------------------------------------------------------------------------+
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                ||                                                W2                                                |
    |                                                ||                                                                                                  |
    |                                                ||                                                                                                  |
    |                                                |+--------------------------------------------------------------------------------------------------+
    |                                                |****************************************************************************************************
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                       W0                       |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                W3                                                *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    +------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn scroll_into_view_with_focused_container() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_mode(); // vertical
    hub.insert_tiling();
    let w3 = hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 0.0);
    hub.set_min_size(w3, 100.0, 0.0);

    hub.focus_parent();
    hub.focus_parent();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=ContainerId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=100.00, y=0.00, w=100.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=100.00, y=0.00, w=100.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=10.00, w=100.00, h=10.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=20.00, w=100.00, h=10.00)
          )
        )
      )
    )

    ******************************************************************************************************************************************************
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                W
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                W0                                                ||                                                W
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                W
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn delete_window_with_min_size_shrinks_parent_container() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();

    hub.set_min_size(w1, 100.0, 0.0);
    hub.set_min_size(w2, 100.0, 0.0);
    hub.set_min_size(w3, 100.0, 0.0);

    // Container min_width = 300 (w1 + w2 + w3), exceeds screen width 150
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=-150.00, y=0.00, w=300.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=-150.00, y=0.00, w=300.00, h=15.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=-150.00, y=15.00, w=300.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=-150.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=-50.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=15.00, w=100.00, h=15.00)
          )
        )
      )
    )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    0                                                                                                                                                    |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    2                                                |*                                                W3                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");

    hub.delete_window(w1);

    // After deleting w1, container min_width drops to 200 (w2 + w3)
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=-50.00, y=0.00, w=200.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=-50.00, y=0.00, w=200.00, h=15.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=-50.00, y=15.00, w=200.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(2), parent=ContainerId(1), x=-50.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=50.00, y=15.00, w=100.00, h=15.00)
          )
        )
      )
    )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                     W0                                                                                                  |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    2                                                |*                                                W3                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn delete_window_with_min_size_allows_siblings_to_expand() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 0.0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        )
      )
    )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                W0                                                |*                       W1                       *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    |                                                                                                  |*                                                *
    +--------------------------------------------------------------------------------------------------+**************************************************
    ");

    hub.delete_window(w0);

    // After deleting w0, w1 expands to full screen width
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
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
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn scroll_window_into_view_in_vertical_child_container() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();

    hub.set_min_size(w0, 100.0, 20.0);
    hub.set_min_size(w1, 100.0, 20.0);
    hub.set_min_size(w2, 100.0, 20.0);
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=-30.00, w=150.00, h=60.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=-30.00, w=100.00, h=60.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=-30.00, w=100.00, h=20.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=-10.00, w=100.00, h=20.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=10.00, w=100.00, h=20.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=-30.00, w=50.00, h=60.00)
        )
      )
    )

    |                                                W1                                                ||                       W3                       |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                W2                                                *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    ****************************************************************************************************+------------------------------------------------+
    ");

    hub.delete_window(w0);

    // After deleting w0, w1 expands to full screen width
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=-10.00, w=150.00, h=40.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=-10.00, w=100.00, h=40.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=-10.00, w=100.00, h=20.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=10.00, w=100.00, h=20.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=-10.00, w=50.00, h=40.00)
        )
      )
    )

    |                                                W1                                                ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                       W3                       |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                W2                                                *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    ****************************************************************************************************+------------------------------------------------+
    ");
}
