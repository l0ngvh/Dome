use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn move_window_to_empty_workspace() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(1), x=1.00, y=1.00, w=148.00, h=28.00)
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
fn move_window_to_workspace_with_windows() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.focus_workspace(1);
    hub.insert_tiling("W2".into());
    hub.focus_workspace(0);
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=76.00, y=1.00, w=73.00, h=28.00)
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
fn move_only_window_to_workspace() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(1), x=1.00, y=1.00, w=148.00, h=28.00)
      )
    )
    ");
}

#[test]
fn move_to_same_workspace_does_nothing() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.move_focused_to_workspace(0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=73.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=76.00, y=1.00, w=73.00, h=28.00)
        )
      )
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
fn move_container_to_workspace() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.toggle_spawn_direction();
    hub.insert_tiling("W2".into());
    hub.focus_parent();
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=ContainerId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Window(id=WindowId(1), parent=ContainerId(1), x=1.00, y=1.00, w=148.00, h=13.00)
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=16.00, w=148.00, h=13.00)
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
fn move_container_to_workspace_with_matching_direction() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.focus_parent();

    hub.focus_workspace(1);
    hub.insert_tiling("W4".into());
    hub.insert_tiling("W5".into());

    hub.focus_workspace(0);
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(3), parent=ContainerId(1), x=38.50, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(0), parent=ContainerId(1), x=76.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(1), x=113.50, y=1.00, w=35.50, h=28.00)
        )
      )
    )
    ");
}

#[test]
fn move_horizontal_container_to_workspace_with_one_window() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.insert_tiling("W1".into());
    hub.focus_parent();

    hub.focus_workspace(1);
    hub.insert_tiling("W2".into());

    hub.focus_workspace(0);
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=1.00, w=48.00, h=28.00)
          Window(id=WindowId(0), parent=ContainerId(1), x=51.00, y=1.00, w=48.00, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(1), x=101.00, y=1.00, w=48.00, h=28.00)
        )
      )
    )
    ");
}

#[test]
fn move_vertical_container_to_workspace_with_one_window() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.toggle_spawn_direction();
    hub.insert_tiling("W1".into());
    hub.focus_parent();

    hub.focus_workspace(1);
    hub.insert_tiling("W2".into());

    hub.focus_workspace(0);
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(0), parent=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=76.00, y=1.00, w=73.00, h=13.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=76.00, y=16.00, w=73.00, h=13.00)
          )
        )
      )
    )
    ");
}

#[test]
fn move_container_to_workspace_with_container_direction_matching_workspace_spawn_direction() {
    let mut hub = setup();

    hub.insert_tiling("W0".into());
    hub.toggle_spawn_direction();
    hub.insert_tiling("W1".into());
    hub.focus_parent();

    hub.focus_workspace(1);
    hub.insert_tiling("W4".into());
    hub.insert_tiling("W5".into());
    hub.toggle_spawn_direction();

    hub.focus_workspace(0);
    hub.move_focused_to_workspace(1);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(1), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(2), parent=ContainerId(1), x=1.00, y=1.00, w=73.00, h=28.00)
          Container(id=ContainerId(2), parent=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, direction=Vertical,
            Window(id=WindowId(3), parent=ContainerId(2), x=76.00, y=1.00, w=73.00, h=8.00)
            Window(id=WindowId(0), parent=ContainerId(2), x=76.00, y=11.00, w=73.00, h=8.00)
            Window(id=WindowId(1), parent=ContainerId(2), x=76.00, y=21.00, w=73.00, h=8.00)
          )
        )
      )
    )
    ");
}
