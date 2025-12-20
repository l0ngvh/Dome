use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn initial_window_cover_full_screen() {
    let mut hub = setup();
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
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
fn split_window_evenly() {
    let mut hub = setup();
    for _ in 0..4 {
        hub.insert_window();
    }
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=38.50, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=76.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=113.50, y=1.00, w=35.50, h=28.00)
        )
      )
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
fn new_container_preserves_wrapped_window_position() {
    let mut hub = setup();
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    // Focus w1 (middle)
    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();
    // New container wrapping w1 should be in the middle position
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=51.00, y=1.00, w=48.00, h=13.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=51.00, y=16.00, w=48.00, h=13.00)
          )
          Window(id=WindowId(2), parent=ContainerId(0), x=101.00, y=1.00, w=48.00, h=28.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W1                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                       W0                       |**************************************************|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W3                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn insert_window_after_focused_window() {
    let mut hub = setup();
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    // Focus w1 (middle)
    hub.focus_left();
    hub.insert_window();
    // w3 should be inserted right after w1, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=38.50, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(3), parent=ContainerId(0), x=76.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=113.50, y=1.00, w=35.50, h=28.00)
        )
      )
    )

    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                 W0                 ||                W1                 |*                 W3                 *|                W2                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");
}

#[test]
fn insert_window_after_focused_container_with_same_new_window_direction() {
    let mut hub = setup();
    // Create: [w0] [w1, w2] [w3]
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();
    // Focus the middle container and toggle back new window direction
    hub.focus_left();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();
    // w4 should be inserted right after the focused container, not at the end
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=50.00, y=0.00, w=50.00, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=51.00, y=1.00, w=48.00, h=8.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=51.00, y=11.00, w=48.00, h=8.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=51.00, y=21.00, w=48.00, h=8.00)
          )
          Window(id=WindowId(3), parent=ContainerId(0), x=101.00, y=1.00, w=48.00, h=28.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                       W1                       ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W2                       ||                       W3                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                |+------------------------------------------------+|                                                |
    |                                                |**************************************************|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                       W4                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn insert_to_new_container_when_focused_container_window_insert_direction_differ_and_no_parent() {
    let mut hub = setup();
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Vertical,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=150.00, h=15.00, direction=Horizontal,
            Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=48.00, h=13.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=51.00, y=1.00, w=48.00, h=13.00)
            Window(id=WindowId(2), parent=ContainerId(0), x=101.00, y=1.00, w=48.00, h=13.00)
          )
          Window(id=WindowId(3), parent=ContainerId(1), x=1.00, y=16.00, w=148.00, h=13.00)
        )
      )
    )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                       W1                       ||                       W2                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W3                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn insert_to_parent_when_focused_container_window_insert_direction_differ_but_has_parent() {
    let mut hub = setup();
    // Creating [w0, [w1, w2], w3]
    hub.insert_window();
    hub.insert_window();
    hub.insert_window();
    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    // Should be inserted in the root container
    hub.insert_window();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=35.50, h=28.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=37.50, y=0.00, w=37.50, h=30.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=38.50, y=1.00, w=35.50, h=13.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=38.50, y=16.00, w=35.50, h=13.00)
          )
          Window(id=WindowId(4), parent=ContainerId(0), x=76.00, y=1.00, w=35.50, h=28.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=113.50, y=1.00, w=35.50, h=28.00)
        )
      )
    )

    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                W1                 |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    |+-----------------------------------+*                                    *|                                   |
    |                 W0                 |+-----------------------------------+*                 W4                 *|                W2                 |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                W3                 |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");
}

// TODO: test unfocus then insert new window
