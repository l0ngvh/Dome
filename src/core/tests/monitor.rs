use insta::assert_snapshot;

use crate::core::node::Dimension;

use super::{setup, snapshot};

#[test]
fn add_monitor_creates_workspace_on_new_monitor() {
    let mut hub = setup();
    hub.insert_tiling();

    hub.add_monitor(
        "monitor-1".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=monitor-1, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    )
    ");
}

#[test]
fn remove_monitor_migrates_workspaces_to_fallback() {
    let mut hub = setup();
    hub.insert_tiling();

    let primary = hub.focused_monitor();
    let m1 = hub.add_monitor(
        "monitor-1".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling();
    hub.insert_tiling();

    hub.remove_monitor(m1, primary);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=monitor-1, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
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
}

#[test]
#[should_panic(expected = "fallback must differ")]
fn remove_monitor_panics_if_fallback_same_as_removed() {
    let mut hub = setup();
    let m1 = hub.add_monitor(
        "monitor-1".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );
    hub.remove_monitor(m1, m1);
}

#[test]
fn update_monitor_dimension_adjusts_workspaces() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    let primary = hub.focused_monitor();
    hub.update_monitor_dimension(
        primary,
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 50.0,
        },
    );

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=200.00 h=50.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=50.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=50.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=100.00, h=50.00)
        )
      )
    )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                W0                                                |*                                                W
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*
    ");
}
