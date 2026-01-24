use insta::assert_snapshot;

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
        10.0,
        true,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.sync_config(10.0, true, 0.0, 0.0, 0.0, 0.0);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=50.00 h=50.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=50.00, h=50.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=10.00, w=50.00, h=40.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=10.00, w=50.00, h=40.00)
        )
      )
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
        10.0,
        true,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.focus_workspace("1");
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout();

    hub.sync_config(5.0, true, 0.0, 0.0, 0.0, 0.0);

    hub.focus_workspace("0");
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=50.00 h=50.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=50.00, h=50.00, tabbed=true, active_tab=1,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=5.00, w=50.00, h=45.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=50.00, h=45.00)
        )
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=50.00, h=50.00, tabbed=true, active_tab=1,
          Window(id=WindowId(2), parent=ContainerId(1), x=0.00, y=5.00, w=50.00, h=45.00)
          Window(id=WindowId(3), parent=ContainerId(1), x=0.00, y=5.00, w=50.00, h=45.00)
        )
      )
    )
    ");
}
