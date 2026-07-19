use insta::assert_snapshot;

use crate::core::node::{Length, Logical};
use crate::core::tests::{
    LayoutConfigBuilder, PartitionTreeConfigBuilder, TestHubBuilder, snapshot,
};

#[test]
fn sync_config_updates_tab_bar_height() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Length::<Logical>::new(5.0))
                        .with_automatic_tiling(true)
                        .build(),
                )
                .build(),
        )
        .build();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();

    hub.sync_configuration(
        LayoutConfigBuilder::new()
            .with_partition_tree_config(
                PartitionTreeConfigBuilder::new()
                    .with_tab_bar_height(Length::<Logical>::new(10.0))
                    .with_automatic_tiling(true)
                    .build(),
            )
            .build(),
    );

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
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
fn sync_config_recalculates_all_workspaces() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Length::<Logical>::new(10.0))
                        .with_automatic_tiling(true)
                        .build(),
                )
                .build(),
        )
        .build();
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();

    hub.focus_workspace("1");
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();

    hub.sync_configuration(
        LayoutConfigBuilder::new()
            .with_partition_tree_config(
                PartitionTreeConfigBuilder::new()
                    .with_tab_bar_height(Length::<Logical>::new(5.0))
                    .with_automatic_tiling(true)
                    .build(),
            )
            .build(),
    );

    hub.focus_workspace("0");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=5.00, w=150.00, h=25.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
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
    ******************************************************************************************************************************************************
    ");
}
