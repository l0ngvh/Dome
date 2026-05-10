use crate::config::{LayoutConfig, LayoutKind, MasterStackConfig};
use crate::core::hub::{Hub, HubConfig};
use crate::core::node::{Dimension, Length, WindowRestrictions};
use crate::core::strategy::TilingAction;

use super::{
    default_layout_for_tests, default_partition_tree_config_for_tests, setup_hub, snapshot,
};
use insta::assert_snapshot;

fn layout(active: LayoutKind, ratio: f32, count: usize) -> LayoutConfig {
    LayoutConfig {
        active,
        partition_tree: default_partition_tree_config_for_tests(),
        master_stack: MasterStackConfig {
            master_ratio: ratio,
            master_count: count,
        },
    }
}

fn setup_hub_with_layout(layout_cfg: LayoutConfig) -> Hub {
    Hub::new(
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
        HubConfig {
            layout: layout_cfg,
            ..Default::default()
        },
    )
}

#[test]
fn sync_config_no_op_when_layout_unchanged() {
    let mut hub = setup_hub();
    hub.insert_tiling();
    hub.insert_tiling();
    let ws = hub.current_workspace();
    let focus_before = hub.focused_tiling_window(ws);
    let snap_before = snapshot(&hub);
    hub.sync_config(HubConfig::default());
    assert_eq!(hub.focused_tiling_window(ws), focus_before);
    assert_eq!(snapshot(&hub), snap_before);
}

#[test]
fn sync_config_inactive_master_stack_field_change_preserves_tree() {
    let mut hub = setup_hub();
    hub.insert_tiling();
    hub.insert_tiling();
    // Create a tabbed container to verify tree state survives.
    hub.toggle_container_layout();
    let ws = hub.current_workspace();
    let focus_before = hub.focused_tiling_window(ws);

    // Change master-stack params while partition-tree is active.
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            master_stack: MasterStackConfig {
                master_ratio: 0.3,
                master_count: 2,
            },
            ..default_layout_for_tests()
        },
        ..Default::default()
    });

    // Tree state (tabbed container) and focus preserved.
    assert_eq!(hub.focused_tiling_window(ws), focus_before);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[, ])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                          |                                  []                                     |
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
fn sync_config_switches_partition_tree_to_master_stack() {
    let mut hub = setup_hub();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.sync_config(HubConfig {
        layout: layout(LayoutKind::MasterStack, 0.5, 1),
        ..Default::default()
    });

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
fn sync_config_switches_master_stack_to_partition_tree() {
    let mut hub = setup_hub_with_layout(layout(LayoutKind::MasterStack, 0.5, 1));
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.sync_config(HubConfig::default());

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=0.00, w=37.50, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(1), x=37.50, y=0.00, w=37.50, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=37.50, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, , , ])
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
    hub.insert_tiling();
    let float_id = hub.insert_float(float_dim);
    let _fs_id = hub.insert_fullscreen(WindowRestrictions::None);

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

    hub.sync_config(HubConfig {
        layout: layout(LayoutKind::MasterStack, 0.5, 1),
        ..Default::default()
    });

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
    // Verify the float window still exists.
    let _float = hub.get_window(float_id);
}

#[test]
fn sync_config_swap_empty_workspace_no_panic() {
    let mut hub = setup_hub();
    // No windows inserted.
    hub.sync_config(HubConfig {
        layout: layout(LayoutKind::MasterStack, 0.5, 1),
        ..Default::default()
    });

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn sync_config_swap_iterates_every_active_workspace() {
    let mut hub = setup_hub();
    // Workspace "0": two tiling windows.
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.focus_workspace("1");
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(30.0),
        Length::new(20.0),
    );
    let _float_id = hub.insert_float(float_dim);

    // Go back to workspace "0" so post-swap snapshot shows it.
    hub.focus_workspace("0");

    hub.sync_config(HubConfig {
        layout: layout(LayoutKind::MasterStack, 0.5, 1),
        ..Default::default()
    });

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
fn sync_config_swap_preserves_float_focus_flag() {
    let mut hub = setup_hub();
    hub.insert_tiling();
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(30.0),
        Length::new(20.0),
    );
    let float_id = hub.insert_float(float_dim);
    // Focus the float so is_float_focused becomes true.
    hub.set_focus(float_id);

    let ws = hub.current_workspace();
    assert!(
        hub.get_workspace(ws).is_float_focused(),
        "float should be focused before swap"
    );

    hub.sync_config(HubConfig {
        layout: layout(LayoutKind::MasterStack, 0.5, 1),
        ..Default::default()
    });

    // is_float_focused is restored after the swap.
    assert!(
        hub.get_workspace(ws).is_float_focused(),
        "float focus must survive strategy swap"
    );
    // The float window has effective focus.
    let placements = hub.get_visible_placements();
    assert_eq!(placements.focused_window, Some(float_id));
    // validate_hub checks invariants (called inside snapshot).
    assert_snapshot!(snapshot(&hub), @"
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
}
