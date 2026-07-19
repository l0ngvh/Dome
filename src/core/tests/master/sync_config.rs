use crate::config::{MasterConfig, Strategy};
use crate::core::strategy::TilingAction;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, snapshot, titled,
};
use insta::assert_snapshot;

#[test]
fn sync_config_fill_master() {
    // Global master_count increase promotes unmatched windows from stack
    // into master for workspaces without a per-workspace override.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w36"));
    hub.insert_tiling(hub.current_workspace(), titled("w37"));
    hub.insert_tiling(hub.current_workspace(), titled("w38"));
    hub.insert_tiling(hub.current_workspace(), titled("w39"));
    hub.insert_tiling(hub.current_workspace(), titled("w40"));

    let ws = hub.current_workspace();
    let focus_before = hub.focused_window(ws);

    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.5,
            master_count: 2,
        })
        .build();
    hub.sync_configuration(l);

    assert_eq!(hub.focused_window(ws), focus_before);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=20.00, w=75.00, h=10.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    +-------------------------------------------------------------------------+|                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_drop_masters() {
    // Config values seed new workspaces via attach_window. After a reload with
    // master_ratio=0.3, a previously-untouched workspace gets that ratio on
    // its first attach.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();

    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.3,
            master_count: 1,
        })
        .build();
    hub.sync_configuration(l);

    hub.focus_workspace("1");
    hub.insert_tiling(hub.current_workspace(), titled("w41"));
    hub.insert_tiling(hub.current_workspace(), titled("w42"));
    hub.insert_tiling(hub.current_workspace(), titled("w43"));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=45.00, h=30.00)
        Window(id=WindowId(1), x=45.00, y=0.00, w=105.00, h=15.00)
        Window(id=WindowId(2), x=45.00, y=15.00, w=105.00, h=15.00, highlighted)
      )

    +-------------------------------------------++-------------------------------------------------------------------------------------------------------+
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                   W1                                                  |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           ||                                                                                                       |
    |                                           |+-------------------------------------------------------------------------------------------------------+
    |                     W0                    |*********************************************************************************************************
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                   W2                                                  *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    |                                           |*                                                                                                       *
    +-------------------------------------------+*********************************************************************************************************
    ");
}

#[test]
fn sync_config_preserves_runtime_tuned_master_ratio() {
    // Runtime GrowMaster tuning persists across config reload. A hot-reload
    // does NOT reset the ratio back to the file value (preserve semantics).
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w49"));
    hub.insert_tiling(hub.current_workspace(), titled("w50"));

    // GrowMaster 3 times: 0.5 -> 0.55 -> 0.60 -> 0.65
    hub.handle_tiling_action(TilingAction::GrowMaster);
    hub.handle_tiling_action(TilingAction::GrowMaster);
    hub.handle_tiling_action(TilingAction::GrowMaster);

    // Hot-reload with a different file value does NOT override runtime tuning.
    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.4,
            master_count: 1,
        })
        .build();
    hub.sync_configuration(l);

    // Layout still shows runtime-tuned 0.65 ratio.
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=97.00, h=30.00)
        Window(id=WindowId(1), x=97.00, y=0.00, w=53.00, h=30.00, highlighted)
      )

    +-----------------------------------------------------------------------------------------------+*****************************************************
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                               W0                                              |*                         W1                        *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    |                                                                                               |*                                                   *
    +-----------------------------------------------------------------------------------------------+*****************************************************
    ");
}

#[test]
fn sync_config_preserves_runtime_tuned_master_count() {
    // Runtime MoreMaster tuning persists across config reload. A hot-reload
    // does NOT reset the count back to the file value (preserve semantics).
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.insert_tiling(hub.current_workspace(), titled("w52"));
    hub.insert_tiling(hub.current_workspace(), titled("w53"));
    hub.insert_tiling(hub.current_workspace(), titled("w54"));

    // MoreMaster: master_count 1 -> 2
    hub.handle_tiling_action(TilingAction::MoreMaster);

    // Hot-reload with a different file value does NOT override runtime tuning.
    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.5,
            master_count: 3,
        })
        .build();
    hub.sync_configuration(l);

    // Layout still shows runtime-tuned master_count=2 (not config's 3).
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(3), x=75.00, y=15.00, w=75.00, h=15.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_preserves_workspace_master_count_override() {
    // Global master_count increase promotes unmatched windows on workspace 0
    // (which has no override). The preferred layout override on workspace 1
    // is unaffected.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.insert_tiling(hub.current_workspace(), titled("w52"));
    hub.insert_tiling(hub.current_workspace(), titled("w53"));
    hub.insert_tiling(hub.current_workspace(), titled("w54"));

    hub.sync_configuration(
        LayoutConfigBuilder::new()
            .with_strategy(Strategy::Master)
            .with_master_config(MasterConfig {
                master_ratio: 0.5,
                master_count: 3,
            })
            .build(),
    );

    // Workspace 0 (no override) now gets 3 masters from the global increase.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_preserves_workspace_master_ratio_override() {
    // Global master_count increase promotes unmatched windows on workspace 0
    // (which has no override). The preferred layout ratio override on
    // workspace 1 is unaffected.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master_ratio(0.3)
                .build(),
        ])
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w51"));
    hub.insert_tiling(hub.current_workspace(), titled("w52"));
    hub.insert_tiling(hub.current_workspace(), titled("w53"));
    hub.insert_tiling(hub.current_workspace(), titled("w54"));

    hub.sync_configuration(
        LayoutConfigBuilder::new()
            .with_strategy(Strategy::Master)
            .with_master_config(MasterConfig {
                master_ratio: 0.5,
                master_count: 3,
            })
            .build(),
    );

    // Workspace 0 (no override) now gets 3 masters from the global increase.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=20.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn sync_config_global_count_decrease() {
    // Global master_count decrease demotes excess masters to stack
    // for workspaces without a per-workspace override.
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 3,
                })
                .build(),
        )
        .build();
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));

    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
        })
        .build();
    hub.sync_configuration(l);

    assert_snapshot!(snapshot(&hub), @r"
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
