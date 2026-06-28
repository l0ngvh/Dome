use insta::assert_snapshot;

use crate::config::{LayoutConfig, PartitionTreeConfig, SizeConstraint};

use crate::core::node::Length;
use crate::core::tests::{
    default_layout_for_tests, default_partition_tree_config_for_tests, setup, snapshot, titled,
};

#[test]
fn set_min_size_respects_minimum_height() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w1"));

    hub.set_window_constraint(w0, None, Some(20.0), None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=20.00, w=150.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=20.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=125.00, y=0.00, w=25.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=100.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, w3, w4])
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
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w7"));

    hub.set_window_constraint(w2, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=50.00, y=15.00, w=100.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w5, Container])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=100.00, h=30.00, titles=[w6, w7])
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
fn children_combined_size_exceeds_screen_size() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w8"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w9"));

    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w1, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w8, w9])
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
fn children_combined_size_exceeds_container_size() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w12"));
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w13"));

    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=50.00, y=15.00, w=100.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w10, Container])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w11, Container])
        Container(id=ContainerId(2), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w12, w13])
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
fn children_combined_size_exceeds_screen_height() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w14"));
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w15"));

    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w14, w15])
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
fn set_min_size_tabbed_child_container() {
    let mut hub = setup();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();
    let w3 = hub.insert_tiling_titled();

    hub.set_window_constraint(w3, Some(100.0), Some(20.0), None, None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=50.00, y=10.00, w=100.00, h=20.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=50.00, y=2.00, w=100.00, h=8.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=100.00, h=30.00, tabbed, active_tab=1, titles=[W1, Container])
        Container(id=ContainerId(2), x=50.00, y=2.00, w=100.00, h=28.00, titles=[W2, W3])
      )

    +------------------------------------------------++--------------------------------------------------------------------------------------------------+
    |                                                ||                       W1                        |                  [Container]                   |
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
fn delete_window_with_min_size_shrinks_parent_container() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w18"));
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w19"));

    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    hub.set_window_constraint(w2, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

    // Container min_width = 300 (w1 + w2 + w3), exceeds screen width 150
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=50.00, y=15.00, w=100.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w16, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w17, w18, w19])
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
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=50.00, y=15.00, w=100.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w16, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w18, w19])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.insert_tiling(hub.current_workspace(), titled("w21"));

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w20, w21])
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
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
fn max_height_centers_window_vertically_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w22"));
    hub.insert_tiling(hub.current_workspace(), titled("w23"));

    hub.set_window_constraint(w0, None, None, None, Some(15.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=7.50, w=75.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w22, w23])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w25"));

    hub.set_window_constraint(w0, None, None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=50.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, w25])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.insert_tiling(hub.current_workspace(), titled("w27"));

    hub.set_window_constraint(w0, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=30.00, y=0.00, w=120.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w26, w27])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w28"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w29"));

    hub.set_window_constraint(w0, None, None, Some(30.0), None);
    hub.set_window_constraint(w1, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=30.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=45.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w28, w29])
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
    hub.insert_tiling_titled();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling_titled();

    hub.set_window_constraint(w1, None, None, Some(60.0), Some(10.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=45.00, y=11.00, w=60.00, h=10.00, highlighted, spawn=top)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w30"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w31"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w32"));

    hub.set_window_constraint(w0, None, None, None, Some(10.0));

    hub.set_window_constraint(w1, None, None, None, Some(10.0));
    hub.set_window_constraint(w2, None, Some(10.0), None, Some(10.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=75.00, y=5.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=10.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w30, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w31, w32])
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
fn global_max_applies_to_all_windows() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w33"));
    hub.insert_tiling(hub.current_workspace(), titled("w34"));

    hub.sync_config(LayoutConfig {
        partition_tree: PartitionTreeConfig {
            automatic_tiling: true,
            ..default_partition_tree_config_for_tests()
        },
        max_width: SizeConstraint::Pixels(Length::new(60.0)),
        ..default_layout_for_tests()
    });

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=60.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=15.00, y=0.00, w=60.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w33, w34])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w35"));
    hub.insert_tiling(hub.current_workspace(), titled("w36"));

    hub.sync_config(LayoutConfig {
        partition_tree: PartitionTreeConfig {
            automatic_tiling: true,
            ..default_partition_tree_config_for_tests()
        },
        max_width: SizeConstraint::Pixels(Length::new(60.0)),
        ..default_layout_for_tests()
    });
    hub.set_window_constraint(w0, None, None, Some(30.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=60.00, y=0.00, w=60.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=30.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w35, w36])
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w37"));

    hub.set_window_constraint(w0, None, None, Some(60.0), Some(15.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=45.00, y=7.50, w=60.00, h=15.00, highlighted, spawn=right)
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
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w38"));

    hub.set_window_constraint(w0, None, None, Some(200.0), Some(50.0));

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
fn clearing_constraint_allows_window_to_resize() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w39"));
    hub.insert_tiling(hub.current_workspace(), titled("w40"));

    hub.set_window_constraint(w0, Some(100.0), None, None, None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w39, w40])
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
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w39, w40])
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
fn new_max_clamps_existing_min() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w41"));
    hub.insert_tiling(hub.current_workspace(), titled("w42"));

    hub.set_window_constraint(w0, Some(100.0), None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w41, w42])
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
fn raising_min_above_existing_max_raises_max() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w43"));
    hub.insert_tiling(hub.current_workspace(), titled("w44"));

    // Set max_h=10. In a horizontal split with screen height 30,
    // w0 height is capped at 10, centered vertically.
    hub.set_window_constraint(w0, None, None, None, Some(10.0));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=10.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w43, w44])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
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
    |                                    W0                                   |*                                    W1                                   *
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
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");

    // Raise min_h=15 above max_h=10. If max stays at 10, the layout
    // is contradictory and the implementation must raise max to 15.
    // Observable: w0 height is now 15, not 10 and not 30.
    hub.set_window_constraint(w0, None, Some(15.0), None, None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=7.50, w=75.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w43, w44])
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
fn setting_max_to_zero_clears_constraint() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w45"));
    hub.insert_tiling(hub.current_workspace(), titled("w46"));

    // Cap w0 height at 10. w0 takes 75x10 centered.
    hub.set_window_constraint(w0, None, None, None, Some(10.0));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=10.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, w46])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
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
    |                                    W0                                   |*                                    W1                                   *
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
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");

    // Clear max_h with Some(-1.0). Negative is normalized to 0.0,
    // which clears the constraint. w0 expands to 75x30.
    hub.set_window_constraint(w0, None, None, None, Some(-1.0));
    let cleared = snapshot(&hub);
    assert_snapshot!(cleared, @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, w46])
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

    // Re-cap w0 height at 10, then clear with Some(0.0).
    hub.set_window_constraint(w0, None, None, None, Some(10.0));
    hub.set_window_constraint(w0, None, None, None, Some(0.0));
    assert_eq!(snapshot(&hub), cleared);
}

#[test]
fn setting_min_below_existing_max_keeps_max() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w47"));
    hub.insert_tiling(hub.current_workspace(), titled("w48"));

    // Cap w0 height at 20. w0 takes 75x20 centered.
    hub.set_window_constraint(w0, None, None, None, Some(20.0));
    let capped = snapshot(&hub);
    assert_snapshot!(capped, @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=5.00, w=75.00, h=20.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w47, w48])
      )

                                                                               ***************************************************************************
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
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");

    // Set min_h=10 below max_h=20. If max were incorrectly lowered
    // to 10, w0 would render at height 10. It should stay at 20.
    hub.set_window_constraint(w0, None, Some(10.0), None, None);
    assert_eq!(snapshot(&hub), capped);
}

#[test]
fn window_max_smaller_than_global_min_width() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w49"));

    hub.sync_config(LayoutConfig {
        min_width: SizeConstraint::Pixels(Length::new(300.0)),
        ..default_layout_for_tests()
    });

    // Window max (50) < global min (300) - should not panic, window max takes precedence
    hub.set_window_constraint(w0, None, None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
      )

                                                      **************************************************                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                       W0                       *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      *                                                *                                                  
                                                      **************************************************
    ");
}

#[test]
fn window_max_height_smaller_than_global_min_height() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w50"));

    hub.sync_config(LayoutConfig {
        min_height: SizeConstraint::Pixels(Length::new(300.0)),
        ..default_layout_for_tests()
    });

    // Window max (10) < global min (300) - should not panic
    hub.set_window_constraint(w0, None, None, None, Some(10.0));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=10.00, w=150.00, h=10.00, highlighted, spawn=right)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W0                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn window_max_width_smaller_than_global_min_width() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.insert_tiling(hub.current_workspace(), titled("w52"));

    hub.sync_config(LayoutConfig {
        min_width: SizeConstraint::Pixels(Length::new(100.0)),
        ..default_layout_for_tests()
    });

    // Window max (50) < global min (100) - should not panic
    hub.set_window_constraint(w0, None, None, Some(50.0), None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w51, w52])
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
