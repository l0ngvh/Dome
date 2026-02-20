use insta::assert_snapshot;

use crate::config::SizeConstraint;

use super::{HubConfig, setup, snapshot};

#[test]
fn set_min_size_respects_minimum_width() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

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

    hub.set_window_constraint(w0, None, Some(20.0), None, None);

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

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

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
    |                                                W0                                                ||           W1          |*          W2           *
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

    hub.set_window_constraint(w2, Some(100.0), None, None, None);

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

    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w1, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=100.00, h=30.00)
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
                            W0                       |*                                                W1                                                *
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

    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=0.00, h=30.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=0.00, w=200.00, h=15.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=15.00, w=200.00, h=15.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=0.00, y=15.00, w=100.00, h=15.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=100.00, y=15.00, w=100.00, h=15.00)
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
                                                                              W1                                                                         |
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
                            W2                       |*                                                W3                                                *
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

    hub.set_window_constraint(w0, Some(50.0), None, None, None);
    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);
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
    *                                                *|                                                W1                                                 
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

    hub.sync_config(HubConfig {
        tab_bar_height: 2.0,
        min_width: SizeConstraint::Pixels(100.0),
        ..Default::default()
    });

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=100.00, h=30.00)
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
                            W0                       |*                                                W1                                                *
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

    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=40.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=150.00, h=20.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=20.00, w=150.00, h=20.00)
        )
      )
    )

    |                                                                                                                                                    |
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

    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
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
    |                                                                         W1                                                                         |
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

    hub.set_window_constraint(w3, Some(100.0), Some(20.0), None, None);

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

    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

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
    *                                                                                                  ||                       W1                        
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                W0                                                ||                       W2                        
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                       W3                        
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

    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

    // Container min_width = 300 (w1 + w2 + w3), exceeds screen width 150
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=300.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=300.00, h=15.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=15.00, w=300.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=100.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=200.00, y=15.00, w=100.00, h=15.00)
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
                                                                              W0                                                                         |
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
                            W2                       |*                                                W3                                                *
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
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=200.00, h=15.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=15.00, w=200.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=15.00, w=100.00, h=15.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=15.00, w=100.00, h=15.00)
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
                                                                              W0                                                                         |
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
                            W2                       |*                                                W3                                                *
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

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

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

    hub.set_window_constraint(w0, Some(100.0), Some(20.0), None, None);
    hub.set_window_constraint(w1, Some(100.0), Some(20.0), None, None);
    hub.set_window_constraint(w2, Some(100.0), Some(20.0), None, None);
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling();
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=60.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=100.00, h=60.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=20.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=20.00, w=100.00, h=20.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=40.00, w=100.00, h=20.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=60.00)
        )
      )
    )

    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                W1                                                ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                       W3                       |
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
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=40.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=100.00, h=40.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=20.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=20.00, w=100.00, h=20.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=100.00, y=0.00, w=50.00, h=40.00)
        )
      )
    )

    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                W1                                                ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                       W3                       |
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

#[test]
fn max_height_centers_window_vertically_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, None, Some(15.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=7.50, w=75.00, h=15.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
    )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
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
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}

#[test]
fn max_width_centers_window_horizontally_in_vertical_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=15.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=15.00, w=150.00, h=15.00)
        )
      )
    )

                                                      +------------------------------------------------+                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                       W0                       |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      |                                                |                                                  
                                                      +------------------------------------------------+                                                  
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
fn max_width_limits_window_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=30.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=30.00, y=0.00, w=120.00, h=30.00)
        )
      )
    )

    +----------------------------+************************************************************************************************************************
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |             W0             |*                                                          W1                                                          *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    |                            |*                                                                                                                      *
    +----------------------------+************************************************************************************************************************
    ");
}

#[test]
fn both_windows_at_max_centered_collectively() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(30.0), None);
    hub.set_window_constraint(w1, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=45.00, y=0.00, w=30.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=0.00, w=30.00, h=30.00)
        )
      )
    )

                                                 +----------------------------+******************************                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |             W0             |*             W1             *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 |                            |*                            *                                             
                                                 +----------------------------+******************************
    ");
}

#[test]
fn tabbed_window_with_max_size_is_centered() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();

    hub.set_window_constraint(w1, None, None, Some(60.0), Some(10.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=45.00, y=11.00, w=60.00, h=10.00)
        )
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                 ************************************************************                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                            W1                            *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 ************************************************************
    ");
}

#[test]
fn nested_window_center_due_to_max_constraints() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, None, Some(10.0));

    hub.set_window_constraint(w1, None, None, None, Some(10.0));
    hub.set_window_constraint(w2, None, Some(10.0), None, Some(10.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=10.00, w=75.00, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=75.00, y=5.00, w=75.00, h=10.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=75.00, y=15.00, w=75.00, h=10.00)
          )
        )
      )
    )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                               +-------------------------------------------------------------------------+
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
    +-------------------------------------------------------------------------+|                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                    W2                                   *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}

#[test]
fn new_max_clamps_existing_min() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_window_constraint(w0, Some(100.0), None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=50.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=50.00, y=0.00, w=100.00, h=30.00)
        )
      )
    )

    +------------------------------------------------+****************************************************************************************************
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                       W0                       |*                                                W1                                                *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
    |                                                |*                                                                                                  *
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
fn global_max_applies_to_all_windows() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.sync_config(HubConfig {
        auto_tile: true,
        max_width: SizeConstraint::Pixels(60.0),
        ..Default::default()
    });

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=15.00, y=0.00, w=60.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=75.00, y=0.00, w=60.00, h=30.00)
        )
      )
    )

                   +----------------------------------------------------------+************************************************************               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                            W0                            |*                            W1                            *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   +----------------------------------------------------------+************************************************************
    ");
}

#[test]
fn per_window_max_overrides_global() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.sync_config(HubConfig {
        auto_tile: true,
        max_width: SizeConstraint::Pixels(60.0),
        ..Default::default()
    });
    hub.set_window_constraint(w0, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=30.00, y=0.00, w=30.00, h=30.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=60.00, y=0.00, w=60.00, h=30.00)
        )
      )
    )

                                  +----------------------------+************************************************************                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |             W0             |*                            W1                            *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  |                            |*                                                          *                              
                                  +----------------------------+************************************************************
    ");
}

#[test]
fn single_window_with_max_size_centered() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(60.0), Some(15.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=45.00, y=7.50, w=60.00, h=15.00)
      )
    )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                 ************************************************************                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                            W0                            *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 *                                                          *                                             
                                                 ************************************************************
    ");
}

#[test]
fn single_window_with_max_larger_than_screen_fills_screen() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(200.0), Some(50.0));

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
fn raising_min_above_existing_max_raises_max() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    // Set max_width=50, max_height=10
    hub.set_window_constraint(w0, None, None, Some(50.0), Some(10.0));
    let (max_w, max_h) = hub.get_window(w0).max_size();
    assert_eq!(max_w, 50.0);
    assert_eq!(max_h, 10.0);

    // Raise min_width=80 (above max_width=50), min_height=15 (above max_height=10)
    hub.set_window_constraint(w0, Some(80.0), Some(15.0), None, None);
    let (min_w, min_h) = hub.get_window(w0).min_size();
    let (max_w, max_h) = hub.get_window(w0).max_size();
    assert_eq!(min_w, 80.0);
    assert_eq!(min_h, 15.0);
    assert_eq!(max_w, 80.0, "max_width should be raised to match min_width");
    assert_eq!(
        max_h, 15.0,
        "max_height should be raised to match min_height"
    );
}

#[test]
fn clearing_constraint_allows_window_to_resize() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

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

    hub.set_window_constraint(w0, Some(0.0), None, None, None);

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
fn setting_max_to_zero_clears_constraint() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    hub.set_window_constraint(w0, Some(50.0), Some(10.0), Some(100.0), Some(20.0));
    let (max_w, max_h) = hub.get_window(w0).max_size();
    assert_eq!(max_w, 100.0);
    assert_eq!(max_h, 20.0);

    hub.set_window_constraint(w0, None, None, Some(0.0), Some(-1.0));
    let (max_w, max_h) = hub.get_window(w0).max_size();
    assert_eq!(max_w, 0.0, "max_width should be cleared to 0");
    assert_eq!(max_h, 0.0, "max_height should be cleared to 0");
}

#[test]
fn setting_min_below_existing_max_keeps_max() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    hub.set_window_constraint(w0, None, None, Some(100.0), Some(20.0));
    hub.set_window_constraint(w0, Some(50.0), Some(10.0), None, None);

    let (min_w, min_h) = hub.get_window(w0).min_size();
    let (max_w, max_h) = hub.get_window(w0).max_size();
    assert_eq!(min_w, 50.0);
    assert_eq!(min_h, 10.0);
    assert_eq!(max_w, 100.0, "max_width unchanged when min <= max");
    assert_eq!(max_h, 20.0, "max_height unchanged when min <= max");
}

#[test]
fn max_height_larger_than_container_fills_height_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.insert_tiling();

    // max_height=50 > container height=30, so window should fill full height
    hub.set_window_constraint(w0, None, None, None, Some(50.0));

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
fn max_width_larger_than_container_fills_width_in_vertical_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.insert_tiling();

    // max_width=200 > container width=150, so window should fill full width
    hub.set_window_constraint(w0, None, None, Some(200.0), None);

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
fn tabbed_window_with_max_larger_than_container_fills_space() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling();

    // max_width=200 > container width=150, max_height=50 > content_height
    hub.set_window_constraint(w1, None, None, Some(200.0), Some(50.0));

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
