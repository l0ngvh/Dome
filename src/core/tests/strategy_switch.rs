use crate::core::TilingConfig;
use crate::core::hub::Hub;
use crate::core::node::{Length, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::core::tests::{master_entry, preferred_layout};
use crate::core::{MasterConfig, PreferredMaster, SplitMode, Strategy, TreeLayoutNode};

use super::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, TilingConfigBuilder, default_rect, setup_hub,
    snapshot, titled, titled_matcher,
};
use insta::assert_snapshot;

fn tiling(strategy: Strategy, ratio: f32, count: usize) -> TilingConfig {
    TilingConfigBuilder::new()
        .with_strategy(strategy)
        .with_master_config(MasterConfig {
            master_ratio: ratio,
            master_count: count,
        })
        .build()
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
fn sync_config_default_layout_change_applies_only_to_new_workspaces() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    let existing = hub.current_workspace();

    hub.sync_configuration(tiling(Strategy::Master, 0.3, 1));
    assert_eq!(hub.strategies.kind_of(existing), Strategy::PartitionTree);

    // The master strategy owned no workspace at the reload, so this also checks that it
    // took the new ratio.
    hub.focus_workspace("1", None);
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=45.00, h=30.00)
        Window(id=WindowId(3), x=45.00, y=0.00, w=105.00, h=30.00, highlighted)
      )

    +-------------------------------------------+*********************************************************************************************************
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                     W2                    |*                                                   W3                                                  *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    +-------------------------------------------+*********************************************************************************************************
    ");
}

#[test]
fn per_workspace_switch_leaves_sibling_unchanged() {
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout([("1".to_string(), master_entry(PreferredMaster::default()))])
        .build();

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
    let mut hub = TestHubBuilder::new()
        .with_tiling(tiling(Strategy::Master, 0.5, 1))
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .build(),
        ])
        .build();
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
    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
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
fn a_size_limit_observed_on_a_partition_tree_workspace_holds_after_a_switch_to_master() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_strategy(Strategy::Master)
        .build()]));

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(1), x=92.00, y=0.00, w=42.00, h=30.00)
      )

    ***************************************************************************                 +----------------------------------------+                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                    W0                                   *                 |                   W1                   |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    *                                                                         *                 |                                        |                
    ***************************************************************************                 +----------------------------------------+
    ");
}

#[test]
fn switching_a_workspace_to_master_frees_its_containers() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w33"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w34"), default_rect(), WindowRestrictions::None);

    // Without a container the switch below would have nothing to free and would pass
    // whether or not it frees anything.
    assert_eq!(hub.access.containers.sorted_ids().len(), 1);

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_strategy(Strategy::Master)
        .build()]));

    // `snapshot` runs the arena reachability assertion, which is what checks the free.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00, highlighted)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
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
    *                                    W0                                   *|                                    W1                                   |
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
fn switching_a_workspace_to_master_and_back_keeps_its_float_and_fullscreen_windows() {
    let mut hub = setup_hub();
    hub.insert_window(titled("w39"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w40"), default_rect(), WindowRestrictions::None);
    hub.toggle_float();
    let w41 = hub
        .insert_window(titled("w41"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_fullscreen();

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_strategy(Strategy::Master)
        .build()]));
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

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
        .with_strategy(Strategy::PartitionTree)
        .build()]));
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

    // w41 covers the monitor in both snapshots above, so w40 is checked once w41 closes.
    hub.delete_window(w41);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, float, highlighted)
      )

    +--------------------------------------------------------------------------***************************************************************************
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                         W*                                    F1                                   *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    +--------------------------------------------------------------------------***************************************************************************
    ");
}

/// Workspace "0" stays partition tree, "1" runs master with one slot so the pane split is
/// observable with two windows.
fn setup_master_on_workspace_one() -> Hub {
    TestHubBuilder::new()
        .with_preferred_layout([(
            "1".to_string(),
            master_entry(PreferredMaster {
                master_count: Some(1),
                ..PreferredMaster::default()
            }),
        )])
        .build()
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
