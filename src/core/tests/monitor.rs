use insta::assert_snapshot;

use crate::core::node::Dimension;

use super::{setup, snapshot, snapshot_text};

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
    Hub(focused=WorkspaceId(1), monitor=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
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
fn remove_non_focused_monitor() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let m1 = hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    // Stay on primary, remove external
    hub.remove_monitor(m1, primary);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=external)
    )
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

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

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

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=200.00 h=50.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=200.00, h=50.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=100.00, h=50.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=100.00, y=0.00, w=100.00, h=50.00)
        )
      )
      Workspace(id=WorkspaceId(1), name=external)
    )
    ");
}

#[test]
fn focus_monitor_by_direction() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    // Monitor to the right
    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    // Monitor below
    hub.add_monitor(
        "bottom-monitor".to_string(),
        Dimension {
            x: 0.0,
            y: 30.0,
            width: 150.0,
            height: 30.0,
        },
    );

    hub.focus_monitor(&MonitorTarget::Right);
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(1), monitor=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=right-monitor)
      Workspace(id=WorkspaceId(2), name=bottom-monitor)
    )
    ");

    hub.focus_monitor(&MonitorTarget::Left);
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=right-monitor)
      Workspace(id=WorkspaceId(2), name=bottom-monitor)
    )
    ");

    hub.focus_monitor(&MonitorTarget::Down);
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(2), monitor=MonitorId(2), screen=(x=0.00 y=30.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=right-monitor)
      Workspace(id=WorkspaceId(2), name=bottom-monitor)
    )
    ");

    hub.focus_monitor(&MonitorTarget::Up);
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=right-monitor)
      Workspace(id=WorkspaceId(2), name=bottom-monitor)
    )
    ");
}

#[test]
fn focus_monitor_noop_when_already_focused() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_monitor(&MonitorTarget::Name("external".to_string())); // no-op

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(1), monitor=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=external)
    )
    ");
}

#[test]
fn focus_monitor_by_name() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(1), monitor=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=external)
    )
    ");
}

#[test]
fn move_to_monitor_moves_focused_window() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=right-monitor, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    )
    ");
}

#[test]
fn move_to_monitor_by_name() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=external, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    )
    ");
}

#[test]
fn move_float_to_monitor() {
    use crate::action::MonitorTarget;
    use crate::core::Dimension;

    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 10.0,
        width: 50.0,
        height: 20.0,
    });

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=external, focused=WindowId(0),
        Float(id=WindowId(0), x=150.00, y=10.00, w=50.00, h=20.00)
      )
    )
    ");
}

#[test]
fn focus_monitor_noop_when_no_monitor_in_direction() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    hub.focus_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
    )
    ");
}

#[test]
fn move_to_monitor_noop_when_no_monitor_in_direction() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
    )
    ");
}

#[test]
fn move_to_monitor_noop_when_same_monitor() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("primary".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=external)
    )
    ");
}

#[test]
fn move_to_monitor_noop_when_no_focused_window() {
    use crate::action::MonitorTarget;

    let mut hub = setup();

    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0)
      Workspace(id=WorkspaceId(1), name=right-monitor)
    )
    ");
}

#[test]
fn move_to_monitor_does_not_change_focus() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
    );

    let original_monitor = hub.focused_monitor();
    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_eq!(hub.focused_monitor(), original_monitor);
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WorkspaceId(0), monitor=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=external, focused=WindowId(1),
        Window(id=WindowId(1), parent=WorkspaceId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    )
    ");
}
