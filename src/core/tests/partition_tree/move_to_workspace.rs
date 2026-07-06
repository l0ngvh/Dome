use crate::core::tests::{
    LayoutConfigBuilder, PartitionTreeConfigBuilder, TestHubBuilder, setup, snapshot, titled,
};
use insta::assert_snapshot;

#[test]
fn move_container_to_workspace() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.focus_parent();
    hub.move_focused_to_workspace("1");

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
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=bottom, titles=[w1, w2])
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
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
    *----------------------------------------------------------------------------------------------------------------------------------------------------*
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
fn move_container_to_workspace_with_matching_direction() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.focus_parent();

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.insert_tiling(hub.current_workspace(), titled("w6"));

    hub.focus_workspace("0");
    hub.move_focused_to_workspace("1");

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=100.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(3), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w5, w6, Container])
        Container(id=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right, titles=[w3, w4])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                       W0                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*------------------------------------------------*
    |                       W2                       ||                       W3                       |*------------------------------------------------*
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                       W1                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_horizontal_container_to_workspace_with_one_window() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w7"));
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    hub.focus_parent();

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w9"));

    hub.focus_workspace("0");
    hub.move_focused_to_workspace("1");

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w9, Container])
        Container(id=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right, titles=[w7, w8])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W0                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*-------------------------------------------------------------------------*
    |                                    W2                                   |*-------------------------------------------------------------------------*
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn move_vertical_container_to_workspace_with_one_window() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.focus_parent();

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w12"));

    hub.focus_workspace("0");
    hub.move_focused_to_workspace("1");

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w12, Container])
        Container(id=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right, titles=[w10, w11])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W0                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*-------------------------------------------------------------------------*
    |                                    W2                                   |*-------------------------------------------------------------------------*
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn move_container_to_workspace_with_container_direction_matching_workspace_spawn_direction() {
    let mut hub = setup();

    hub.insert_tiling(hub.current_workspace(), titled("w13"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w14"));
    hub.focus_parent();

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.toggle_spawn_mode();

    hub.focus_workspace("0");
    hub.move_focused_to_workspace("1");

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=112.50, y=15.00, w=37.50, h=15.00)
        Window(id=WindowId(0), x=75.00, y=15.00, w=37.50, h=15.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w15, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w16, Container])
        Container(id=ContainerId(0), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom, titles=[w13, w14])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W2                                   |***************************************************************************
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                 W0                 ||                W1                 *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    |                                                                         |*                                    ||                                   *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn move_container_to_tabbed_workspace() {
    let mut hub = setup();

    // Create container with 2 windows on workspace 0
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.focus_parent();

    // Create tabbed container on workspace 1
    hub.focus_workspace("1");
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();

    // Go back and move container to workspace 1
    hub.focus_workspace("0");
    hub.move_focused_to_workspace("1");

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=2.00, w=75.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=2.00, w=75.00, h=28.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=2, titles=[W2, W3, Container])
        Container(id=ContainerId(0), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                       W2                        |                      W3                        |                  [Container]                    |
    ******************************************************************************************************************************************************
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W0                                   ||                                    W1                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_to_empty_workspace_resets_spawn_mode() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_automatic_tiling(true)
                        .build(),
                )
                .build(),
        )
        .build();

    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w18"));

    hub.move_focused_to_workspace("1");
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w19"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w18, w19])
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
    |                                    W1                                   |*                                    W2                                   *
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

    // Move a container (not just a window) to verify spawn mode resets for containers too
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_automatic_tiling(true)
                        .build(),
                )
                .build(),
        )
        .build();

    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.focus_parent();

    hub.move_focused_to_workspace("1");
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w22"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w22])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w20, w21])
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
fn move_to_workspace_insert_to_last_focused_tiling_when_float_is_focused() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_automatic_tiling(true)
                        .build(),
                )
                .build(),
        )
        .build();

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w23"));
    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.insert_tiling(hub.current_workspace(), titled("w25"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w27"));
    hub.insert_tiling(hub.current_workspace(), titled("w28"));
    hub.insert_tiling(hub.current_workspace(), titled("w29"));

    hub.toggle_float();

    hub.set_focus(w0);

    hub.move_focused_to_workspace("1");
    hub.focus_workspace("1");

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=125.00, y=20.00, w=25.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(5), x=100.00, y=20.00, w=25.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=20.00, w=25.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(6), x=125.00, y=20.00, w=25.00, h=10.00, float)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w25, w26, Container])
        Container(id=ContainerId(2), x=75.00, y=20.00, w=75.00, h=10.00, titles=[w27, w28, w23])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-----------------------++-----------------------++-----------------------+
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||           W4          ||           W5          ||          F6           |
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||                       ||                       ||                       |
    |                                                                         ||                       ||                       ||                       |
    +-------------------------------------------------------------------------++-----------------------++-----------------------++-----------------------+
    ");
}

#[test]
fn move_container_to_same_workspace_noop() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w30"));
    hub.insert_tiling(hub.current_workspace(), titled("w31"));
    hub.focus_parent();
    // Target == current workspace ("0" is the default). Should be a no-op.
    hub.move_focused_to_workspace("0");

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w30, w31])
      )

    ******************************************************************************************************************************************************
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W0                                   ||                                    W1                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    ");
}
