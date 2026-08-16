use crate::core::node::WindowRestrictions;
use crate::core::tests::{
    LayoutConfigBuilder, PartitionTreeConfigBuilder, TestHubBuilder, default_rect, snapshot, titled,
};
use insta::assert_snapshot;

#[test]
fn auto_tile_sets_horizontal_spawn_mode_when_width_greater_than_height() {
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
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2])
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
    // Going on a round trip to ensure that we can always create a horizontal container with 6
    // direct children, as the auto tile logic can get confused when width is approximately equal
    // to height, due to floating precision lost
    let w0 = hub
        .insert_window(titled("w3"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);
    hub.toggle_direction();
    // Each window is 25x30, height > width, so spawn mode should be vertical
    hub.set_focus(w0);
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=125.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(4), x=100.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(1), x=25.00, y=0.00, w=25.00, h=30.00)
        Window(id=WindowId(6), x=0.00, y=15.00, w=25.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=25.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w4, w5, w6, w7, w8])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=25.00, h=30.00, titles=[w3, w9])
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
    *************************|           W1          ||           W2          ||           W3          ||           W4          ||           W5          |
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
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=top)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W1, W2])
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
    let w0 = hub
        .insert_window(titled("w10"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w12"), default_rect(), WindowRestrictions::None);
    hub.toggle_direction();
    hub.set_focus(w0);
    hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=20.00, w=150.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w11, w12])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=10.00, titles=[w10, w13])
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
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=top)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=2, titles=[W0, W1, W2])
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
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_left();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W3"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=2.00, w=75.00, h=28.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=top, titles=[W1, W3])
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
    hub.insert_window(titled("w14"), default_rect(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=2.00, w=75.00, h=28.00, highlighted, spawn=top)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=2, titles=[W1, W3, w14])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||          W1            |         W3            |         [w14]          |
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
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}
