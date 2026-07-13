use crate::config::{LayoutWorkspaceConfig, MasterConfig, Strategy};
use crate::core::GlobalLayoutConfig;
use crate::core::hub::Hub;
use crate::core::node::{Dimension, Length, WindowRestrictions};

use super::{LayoutConfigBuilder, setup_hub, snapshot, titled};
use insta::assert_snapshot;

fn layout(strategy: Strategy, ratio: f32, count: usize) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_strategy(strategy)
        .with_master_config(MasterConfig {
            master_ratio: ratio,
            master_count: count,
        })
        .build()
}

fn setup_hub_with_layout(
    layout: GlobalLayoutConfig,
    overrides: Vec<LayoutWorkspaceConfig>,
) -> Hub {
    Hub::new(
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
        layout,
        overrides,
        Vec::new(),
    )
}

#[test]
fn sync_config_no_op_when_layout_unchanged() {
    let mut hub = setup_hub();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let ws = hub.current_workspace();
    let focus_before = hub.focused_window(ws);
    let snap_before = snapshot(&hub);
    hub.sync_configuration(GlobalLayoutConfig::default());
    assert_eq!(hub.focused_window(ws), focus_before);
    assert_eq!(snapshot(&hub), snap_before);
}

#[test]
fn sync_config_inactive_master_field_change_preserves_tree() {
    let mut hub = setup_hub();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    // Create a tabbed container to verify tree state survives.
    hub.toggle_container_layout();
    let ws = hub.current_workspace();
    let focus_before = hub.focused_window(ws);

    // Change master-stack params while partition-tree is active.
    let l = LayoutConfigBuilder::new()
        .with_master_config(MasterConfig {
            master_ratio: 0.3,
            master_count: 2,
        })
        .build();
    hub.sync_configuration(l);

    // Tree state (tabbed container) and focus preserved.
    assert_eq!(hub.focused_window(ws), focus_before);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w2, w3])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   w2                                     |                                 [w3]                                    |
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
fn sync_config_switches_partition_tree_to_master() {
    let mut hub = setup_hub();
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.insert_tiling(hub.current_workspace(), titled("w7"));

    let l = layout(Strategy::Master, 0.5, 1);
    hub.sync_configuration(l);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=10.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_switches_master_to_partition_tree() {
    let mut hub = setup_hub_with_layout(layout(Strategy::Master, 0.5, 1), Vec::new());
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.insert_tiling(hub.current_workspace(), titled("w11"));

    hub.sync_configuration(GlobalLayoutConfig::default());

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=0.00, w=37.50, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(1), x=37.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=37.50, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w8, w9, w10, w11])
      )

    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W0                 ||                W1                 ||                 W2                 |*                W3                 *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    ");
}

#[test]
fn sync_config_swap_preserves_float_and_fullscreen() {
    let mut hub = setup_hub();
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(30.0),
        Length::new(20.0),
    );
    hub.insert_tiling(hub.current_workspace(), titled("w12"));
    let _float_id = hub.insert_float(hub.current_workspace(), float_dim, titled("w13"));
    let _fs_id = hub.insert_fullscreen(
        hub.current_workspace(),
        WindowRestrictions::None,
        titled("w14"),
    );

    // With fullscreen on top, only it is visible.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(2))
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
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");

    let l = layout(Strategy::Master, 0.5, 1);
    hub.sync_configuration(l);

    // Remove fullscreen to expose tiling + float layer.
    hub.delete_window(_fs_id);
    // Float survives with original dimension; tiling is laid out by master-stack.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
        Window(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=20.00, float)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *         +----------------------------+                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |             F1             |                                  W0                                                                         *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         +----------------------------+                                                                                                             *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn sync_config_swap_empty_workspace_no_panic() {
    let mut hub = setup_hub();
    // No windows inserted.
    let l = layout(Strategy::Master, 0.5, 1);
    hub.sync_configuration(l);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn sync_config_swap_iterates_every_active_workspace() {
    let mut hub = setup_hub();
    // Workspace "0": two tiling windows.
    hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.insert_tiling(hub.current_workspace(), titled("w18"));

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.insert_tiling(hub.current_workspace(), titled("w22"));
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(30.0),
        Length::new(20.0),
    );
    let _float_id = hub.insert_float(hub.current_workspace(), float_dim, titled("w23"));

    // Go back to workspace "0" so post-swap snapshot shows it.
    hub.focus_workspace("0");

    let l = layout(Strategy::Master, 0.5, 1);
    hub.sync_configuration(l);

    // Workspace "0" re-laid-out by master-stack.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=20.00, w=75.00, h=10.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(8))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(5), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(6), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(7), x=75.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(8), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |         ******************************                                  ||                                    W5                                   |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  |+-------------------------------------------------------------------------+
    |         *                            *                                  |+-------------------------------------------------------------------------+
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *             F8             *                                  ||                                    W6                                   |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  |+-------------------------------------------------------------------------+
    |         *                            *                                  |+-------------------------------------------------------------------------+
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         ******************************                                  ||                                                                         |
    |                                                                         ||                                    W7                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn sync_config_swap_preserves_float_focus() {
    let mut hub = setup_hub();
    hub.insert_tiling(hub.current_workspace(), titled("w24"));
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(30.0),
        Length::new(20.0),
    );
    let float_id = hub.insert_float(hub.current_workspace(), float_dim, titled("w25"));
    // Focus the float so is_float_focused becomes true.
    hub.set_focus(float_id);

    let before = snapshot(&hub);
    assert_snapshot!(before, @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |         ******************************                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *             F1             *                                  W0                                                                         |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         ******************************                                                                                                             |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");

    let l = layout(Strategy::Master, 0.5, 1);
    hub.sync_configuration(l);

    assert_eq!(snapshot(&hub), before);
}

#[test]
fn per_workspace_switch_leaves_sibling_unchanged() {
    let mut hub = setup_hub_with_layout(
        LayoutConfigBuilder::new().build(),
        vec![LayoutWorkspaceConfig::Master {
            name: "1".to_string(),
            master_ratio: None,
            master_count: None,
            master: Vec::new(),
            secondary: Vec::new(),
            float: Vec::new(),
            fullscreen: Vec::new(),
        }],
    );

    hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.insert_tiling(hub.current_workspace(), titled("w27"));

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w28"));
    hub.insert_tiling(hub.current_workspace(), titled("w29"));

    // Reload with same config: workspace "1" stays master, "0" stays partition-tree.
    let l = LayoutConfigBuilder::new().build();
    hub.sync_configuration(l);

    // Workspace "1" uses master layout (big left pane + stack).
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
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
    |                                    W2                                   |*                                    W3                                   *
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

    // Workspace "0" still uses partition-tree (equal horizontal split).
    hub.focus_workspace("0");
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w26, w27])
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
fn same_kind_cross_workspace_move_preserves_container() {
    let mut hub = setup_hub();

    // Build a tabbed container with two windows on workspace "0".
    hub.insert_tiling_titled();
    hub.insert_tiling_titled();
    hub.toggle_container_layout();

    // Focus the container so focused_tiling is Child::Container.
    hub.focus_parent();

    // Move to workspace "1" (same kind: both partition-tree by default).
    hub.move_focused_to_workspace("1");

    // The container should arrive intact on workspace "1".
    hub.focus_workspace("1");
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, highlighted, spawn=right, titles=[W0, W1])
      )

    ******************************************************************************************************************************************************
    *                                   W0                                     |                                 [W1]                                    *
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
