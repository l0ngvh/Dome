use crate::config::tests as fixtures;
use crate::core::ReportedMonitor;
use crate::core::TilingConfig;
use crate::core::hub::Hub;
use crate::core::master::PaneConfig;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{PRIMARY_MONITOR, preferred_layout, setup_logger_with_level};
use crate::core::{MasterConfig, PreferredWorkspace, SplitMode, Strategy, TreeLayoutNode};

use super::{
    LayoutWorkspaceConfigBuilder, TilingConfigBuilder, default_rect, setup_hub, setup_with_tiling,
    snapshot, titled, titled_matcher,
};
use insta::assert_snapshot;

fn tiling(
    strategy: Strategy,
    ratio: f32,
    count: usize,
    floats: &[&str],
    fullscreens: &[&str],
) -> TilingConfig {
    TilingConfigBuilder::new()
        .with_strategy(strategy)
        .with_master_config(MasterConfig {
            master_ratio: ratio,
            master_count: count,
        })
        // One call each with every title, `with_float` and `with_fullscreen`
        // replace their field.
        .with_float(floats.iter().map(|t| titled_matcher(t)).collect())
        .with_fullscreen(fullscreens.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

fn setup_hub_with_tiling(
    tiling: TilingConfig,
    overrides: impl IntoIterator<Item = (String, PreferredWorkspace)>,
) -> Hub {
    Hub::new(
        ReportedMonitor {
            device_name: PRIMARY_MONITOR.to_string(),
            work_area: PixelRect::new(0, 0, 150, 30),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: None,
        },
        tiling,
        preferred_layout(overrides),
    )
}

#[test]
fn sync_config_inactive_master_field_change_preserves_tree() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    // Create a tabbed container to verify tree state survives.
    hub.toggle_container_layout();
    let ws = hub.current_workspace();
    let focus_before = hub.focused_window(ws);

    // Change master-stack params while partition-tree is active.
    let l = TilingConfigBuilder::new()
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
    setup_logger_with_level("trace");
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);

    let l = tiling(Strategy::Master, 0.5, 1, &[], &[]);
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
    let mut hub = setup_hub_with_tiling(tiling(Strategy::Master, 0.5, 1, &[], &[]), Vec::new());
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);

    hub.sync_configuration(fixtures::tiling_config());

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=113.00, y=0.00, w=37.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=38.00, h=30.00)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=38.00, h=30.00)
        Container(id=ContainerId(2), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w8, w9, w10, w11])
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
    |                 W0                 ||                 W1                ||                 W2                 |*                 W3                *
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
    let mut hub = setup_with_tiling(tiling(Strategy::PartitionTree, 0.5, 1, &["w13"], &["w14"]));
    let float_dim = PixelRect::new(10, 5, 30, 20);
    hub.insert_window(titled("w12"), default_rect(), WindowRestrictions::None);
    let _float_id = hub
        .insert_window(titled("w13"), float_dim, WindowRestrictions::None)
        .unwrap();
    let _fs_id = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();

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

    let l = tiling(Strategy::Master, 0.5, 1, &["w13"], &["w14"]);
    hub.sync_configuration(l);

    // Remove fullscreen to expose tiling + float layer.
    hub.delete_window(_fs_id);
    // Float survives with original dimension. Tiling is laid out by master-stack.
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
    let l = tiling(Strategy::Master, 0.5, 1, &[], &[]);
    hub.sync_configuration(l);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn sync_config_swap_iterates_every_active_workspace() {
    let mut hub = setup_with_tiling(tiling(Strategy::PartitionTree, 0.5, 1, &["w23"], &[]));
    // Workspace "0": two tiling windows.
    hub.insert_window(titled("w15"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w18"), default_rect(), WindowRestrictions::None);

    hub.focus_workspace("1", None);
    hub.insert_window(titled("w19"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w20"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w21"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w22"), default_rect(), WindowRestrictions::None);
    let float_dim = PixelRect::new(10, 5, 30, 20);
    let _float_id = hub
        .insert_window(titled("w23"), float_dim, WindowRestrictions::None)
        .unwrap();

    // Go back to workspace "0" so post-swap snapshot shows it.
    hub.focus_workspace("0", None);

    let l = tiling(Strategy::Master, 0.5, 1, &["w23"], &[]);
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

    hub.focus_workspace("1", None);
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
fn per_workspace_switch_leaves_sibling_unchanged() {
    let mut hub = setup_hub_with_tiling(
        TilingConfigBuilder::new().build(),
        [(
            "1".to_string(),
            PreferredWorkspace::Master {
                master_ratio: None,
                master_count: None,
                master: PaneConfig::tiled(Vec::new()),
                secondary: PaneConfig::tiled(Vec::new()),
                float: Vec::new(),
                fullscreen: Vec::new(),
            },
        )],
    );

    hub.insert_window(titled("w26"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w27"), default_rect(), WindowRestrictions::None);

    hub.focus_workspace("1", None);
    hub.insert_window(titled("w28"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w29"), default_rect(), WindowRestrictions::None);

    // Reload with same config: workspace "1" stays master, "0" stays partition-tree.
    let l = TilingConfigBuilder::new().build();
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
    hub.focus_workspace("0", None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(2), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w26, w27])
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
fn switch_into_preferred_tree_layout_focuses_every_migrated_window_in_turn() {
    let mut hub = setup_hub_with_tiling(
        tiling(Strategy::Master, 0.5, 1, &[], &[]),
        vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .build(),
        ],
    );
    let w30 = hub
        .insert_window(titled("w30"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w31 = hub
        .insert_window(titled("w31"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w32 = hub
        .insert_window(titled("w32"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    // Only w30 and w31 match a slot, so w31 reaches the tree through a
    // preferred-layout attach helper, not the spawn-mode path.
    hub.sync_preferred_layout(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_tree(TreeLayoutNode::Container {
            split: Some(SplitMode::Vertical),
            children: vec![
                TreeLayoutNode::Leaf(titled_matcher("w30")),
                TreeLayoutNode::Leaf(titled_matcher("w31")),
            ],
        })
        .build()]));

    // Migration attaches all three but focuses one. Closing the focused window has
    // to land on another migrated window until none are left.
    let mut recovered = vec![hub.focused_window(ws).expect("migration focuses a window")];
    while recovered.len() < 3 {
        hub.delete_window(*recovered.last().unwrap());
        recovered.push(
            hub.focused_window(ws)
                .expect("focus falls back to a migrated window"),
        );
    }
    assert!(recovered.contains(&w30));
    assert!(recovered.contains(&w31));
    assert!(recovered.contains(&w32));
}

#[test]
fn switching_a_workspace_to_master_frees_its_containers() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w33"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w34"), default_rect(), WindowRestrictions::None);

    // Without a container the switch below would have nothing to free and would pass
    // whether or not it frees anything.
    assert_eq!(hub.access.containers.all_active().len(), 1);

    hub.sync_configuration(tiling(Strategy::Master, 0.5, 1, &[], &[]));

    // `snapshot` runs the arena reachability assertion, which is what checks the free.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
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

/// Workspace "0" stays partition tree, "1" runs master with one slot so the pane split is
/// observable with two windows.
fn setup_master_on_workspace_one() -> Hub {
    setup_hub_with_tiling(
        TilingConfigBuilder::new().build(),
        [(
            "1".to_string(),
            PreferredWorkspace::Master {
                master_ratio: None,
                master_count: Some(1),
                master: PaneConfig::tiled(Vec::new()),
                secondary: PaneConfig::tiled(Vec::new()),
                float: Vec::new(),
                fullscreen: Vec::new(),
            },
        )],
    )
}

#[test]
fn moving_a_highlighted_container_into_master_flattens_it() {
    let mut hub = setup_master_on_workspace_one();
    hub.insert_window(titled("w35"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w36"), default_rect(), WindowRestrictions::None);
    hub.focus_parent();

    hub.move_focused_to_workspace("1", None);

    hub.focus_workspace("1", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00)
      )

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
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                    W0                                   |
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
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn master_focuses_its_master_pane_after_a_container_arrives() {
    let mut hub = setup_master_on_workspace_one();
    hub.insert_window(titled("w37"), default_rect(), WindowRestrictions::None);
    let w38 = hub
        .insert_window(titled("w38"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_parent();

    hub.move_focused_to_workspace("1", None);
    hub.focus_workspace("1", None);

    // The dissolve hands over w38 first, so the single master slot takes it and w37,
    // attached last, lands in the stack. Focusing the last attachment would pick w37, so
    // this pins the pane rule rather than the order.
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w38));
}
