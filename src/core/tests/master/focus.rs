use crate::config::Strategy;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, snapshot, titled,
};
use insta::assert_snapshot;

#[test]
fn focus_direction_left_right() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w6")); // W0 = master
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w7")); // W1 = stack (focused)

    // Focus is on W1 (stack). Move left to master.
    hub.focus_left();
    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(w0));

    // Move right back to stack.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Right from stack is no-op.
    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Focus master, then left from master is no-op.
    hub.focus_left();
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
}

#[test]
fn focus_across_panes_restores_last_focused() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let w4 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    let ws = hub.current_workspace();

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

    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w3));

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.focus_right();
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w2));

    hub.focus_left();
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w0));

    hub.focus_right();
    hub.focus_up();
    assert_eq!(hub.focused_window(ws), Some(w4));

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w0));
}

#[test]
fn focus_across_panes_after_moving_a_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    let ws = hub.current_workspace();

    hub.set_focus(w1);
    hub.move_right();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=75.00, y=20.00, w=75.00, h=10.00, highlighted)
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    +-------------------------------------------------------------------------+|                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    // w1 left master, so master answers with the next most recent member it still holds.
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w2));

    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(w1));
}

#[test]
fn focus_across_panes_after_remembered_window_deleted() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let w4 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    let ws = hub.current_workspace();

    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00, highlighted)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=20.00, w=75.00, h=10.00)
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
    ***************************************************************************|                                    W3                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W1                                   *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W4                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");

    hub.delete_window(w1);
    assert_eq!(hub.focused_window(ws), Some(w4));

    // The deletion promoted w2 into master, and w2 is the last remembered master member.
    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn removal_focuses_most_recent_surviving_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("w3"));
    let w4 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    let ws = hub.current_workspace();

    hub.set_focus(w2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=10.00, highlighted)
        Window(id=WindowId(3), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=20.00, w=75.00, h=10.00)
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W2                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |***************************************************************************
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
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");

    hub.delete_window(w2);
    assert_eq!(hub.focused_window(ws), Some(w4));

    hub.focus_left();
    assert_eq!(hub.focused_window(ws), Some(w1));

    // Deleting the focused master window falls back across panes instead of to a master
    // neighbour, because w4 is the most recently focused survivor.
    hub.delete_window(w1);
    assert_eq!(hub.focused_window(ws), Some(w4));
}
