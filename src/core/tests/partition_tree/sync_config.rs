use insta::assert_snapshot;

use crate::core::hub::HubConfig;
use crate::core::{Dimension, Hub, tests::snapshot_text};

#[test]
fn sync_config_updates_tab_bar_height() {
    let mut hub = Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
        HubConfig {
            tab_bar_height: 10.0,
            auto_tile: true,
            ..Default::default()
        },
    );
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.sync_config(HubConfig {
        tab_bar_height: 10.0,
        auto_tile: true,
        ..Default::default()
    });

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=50.00 h=50.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=50.00, h=40.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=50.00, h=50.00, tabbed, active_tab=1, titles=[, ])
      )
    ");
}

#[test]
fn sync_config_recalculates_all_workspaces() {
    let mut hub = Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
        HubConfig {
            tab_bar_height: 10.0,
            auto_tile: true,
            ..Default::default()
        },
    );
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.focus_workspace("1");
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.sync_config(HubConfig {
        tab_bar_height: 5.0,
        auto_tile: true,
        ..Default::default()
    });

    hub.focus_workspace("0");
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=50.00 h=50.00),
        Window(id=WindowId(1), x=0.00, y=5.00, w=50.00, h=45.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=50.00, h=50.00, tabbed, active_tab=1, titles=[, ])
      )
    ");
}
