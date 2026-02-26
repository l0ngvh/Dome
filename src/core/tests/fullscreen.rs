use super::{setup, snapshot};
use crate::core::hub::MonitorLayout;
use crate::core::node::{Child, Dimension, DisplayMode};
use insta::assert_snapshot;

#[test]
fn set_fullscreen_from_tiling() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();
    let _w2 = hub.insert_tiling();
    hub.set_focus(w1);

    hub.set_fullscreen(w1);

    assert_eq!(hub.get_window(w1).mode, DisplayMode::Fullscreen);
    let ws = hub.get_workspace(hub.current_workspace());
    assert_eq!(ws.fullscreen_windows(), &[w1]);
    assert_eq!(ws.focused(), Some(Child::Window(w1)));
    assert_eq!(ws.viewport_offset, (0.0, 0.0));
    assert!(ws.root().is_some());
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Fullscreen(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
      )
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
    |                                                                         W0                                                                         |
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
}

#[test]
fn set_fullscreen_from_float() {
    let mut hub = setup();
    let _w1 = hub.insert_tiling();
    let w2 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 40.0,
        height: 10.0,
    });

    hub.set_fullscreen(w2);

    assert_eq!(hub.get_window(w2).mode, DisplayMode::Fullscreen);
    let ws = hub.get_workspace(hub.current_workspace());
    assert_eq!(ws.fullscreen_windows(), &[w2]);
    assert!(ws.float_windows().is_empty());
    assert_eq!(ws.focused(), Some(Child::Window(w2)));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Fullscreen(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00)
      )
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
    |                                                                         W1                                                                         |
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
}

#[test]
fn set_fullscreen_already_fullscreen() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();
    hub.set_fullscreen(w1);

    let ws_before = hub.get_workspace(hub.current_workspace()).clone();
    hub.set_fullscreen(w1);
    let ws_after = hub.get_workspace(hub.current_workspace());

    assert_eq!(ws_before.fullscreen_windows(), ws_after.fullscreen_windows());
}

#[test]
fn unset_fullscreen_to_tiling() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();
    let _w2 = hub.insert_tiling();
    hub.set_focus(w1);
    hub.set_fullscreen(w1);

    hub.unset_fullscreen(w1);

    assert_eq!(hub.get_window(w1).mode, DisplayMode::Tiling);
    let ws = hub.get_workspace(hub.current_workspace());
    assert!(ws.fullscreen_windows().is_empty());
    assert!(ws.root().is_some());
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
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
    |                                    W1                                   |*                                    W0                                   *
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
fn unset_fullscreen_not_fullscreen() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();

    hub.unset_fullscreen(w1);

    assert_eq!(hub.get_window(w1).mode, DisplayMode::Tiling);
}

#[test]
fn fullscreen_only_topmost_in_placements() {
    let mut hub = setup();
    let _w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let _w3 = hub.insert_tiling();

    hub.set_focus(_w1);
    hub.set_fullscreen(_w1);
    hub.set_focus(w2);
    hub.set_fullscreen(w2);

    let placements = hub.get_visible_placements();
    let mp = &placements[0];
    let MonitorLayout::Fullscreen(fs_id) = &mp.layout else {
        panic!("expected Fullscreen layout");
    };
    assert_eq!(*fs_id, w2);
}

#[test]
fn delete_fullscreen_window() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    hub.set_focus(w1);
    hub.set_fullscreen(w1);

    hub.delete_window(w1);

    let ws = hub.get_workspace(hub.current_workspace());
    assert!(ws.fullscreen_windows().is_empty());
    assert_eq!(ws.focused(), Some(Child::Window(w2)));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
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
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn toggle_fullscreen_on_off() {
    let mut hub = setup();
    let w1 = hub.insert_tiling();
    let _w2 = hub.insert_tiling();
    hub.set_focus(w1);

    hub.toggle_fullscreen();
    assert_eq!(hub.get_window(w1).mode, DisplayMode::Fullscreen);
    let ws = hub.get_workspace(hub.current_workspace());
    assert_eq!(ws.fullscreen_windows(), &[w1]);

    hub.toggle_fullscreen();
    assert_eq!(hub.get_window(w1).mode, DisplayMode::Tiling);
    let ws = hub.get_workspace(hub.current_workspace());
    assert!(ws.fullscreen_windows().is_empty());
    assert!(ws.root().is_some());
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(0), parent=ContainerId(0), x=75.00, y=0.00, w=75.00, h=30.00)
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
    |                                    W1                                   |*                                    W0                                   *
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
