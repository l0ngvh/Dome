use insta::assert_snapshot;

#[cfg(target_os = "windows")]
use super::{LayoutConfigBuilder, PartitionTreeConfigBuilder, TestHubBuilder};
#[cfg(target_os = "windows")]
use crate::config::SizeConstraint;
#[cfg(target_os = "windows")]
use crate::core::node::Logical;
use crate::core::node::{Dimension, Length};

use crate::core::tests::{setup, snapshot, snapshot_text, titled};

#[test]
fn add_monitor_creates_workspace_on_new_monitor() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));

    hub.add_monitor(
        "monitor-1".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling(hub.current_workspace(), titled("w1"));

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
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
fn remove_monitor_migrates_workspaces_to_fallback() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));

    let primary = hub.focused_monitor();
    let m1 = hub.add_monitor(
        "monitor-1".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.insert_tiling(hub.current_workspace(), titled("w4"));

    hub.remove_monitor(m1, primary);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    // Stay on primary, remove external
    hub.remove_monitor(m1, primary);

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
#[should_panic(expected = "fallback must differ")]
fn remove_monitor_panics_if_fallback_same_as_removed() {
    let mut hub = setup();
    let m1 = hub.add_monitor(
        "monitor-1".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );
    hub.remove_monitor(m1, m1);
}

#[test]
fn update_monitor_dimension_adjusts_workspaces() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.insert_tiling(hub.current_workspace(), titled("w6"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    let primary = hub.focused_monitor();
    hub.update_monitor(
        primary,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(200.0),
            Length::new(50.0),
        ),
        1.0,
    );

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=200.00 h=50.00),
        Window(id=WindowId(1), x=100.00, y=0.00, w=100.00, h=50.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=50.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=200.00, h=50.00, titles=[w5, w6])
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    ");
}

#[test]
fn focus_monitor_by_direction() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w7"));

    // Monitor to the right
    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    // Monitor below
    hub.add_monitor(
        "bottom-monitor".to_string(),
        Dimension::new(
            Length::new(0.0),
            Length::new(30.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_monitor(&MonitorTarget::Right);
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    ");

    hub.focus_monitor(&MonitorTarget::Left);
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    ");

    hub.focus_monitor(&MonitorTarget::Down);
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    ");

    hub.focus_monitor(&MonitorTarget::Up);
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    ");

    // Focus by name twice: second call is no-op
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    let after_name = snapshot_text(&hub);
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    assert_eq!(snapshot_text(&hub), after_name);
}

#[test]
fn focus_monitor_by_name() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w8"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    ");
}

#[test]
fn move_to_monitor_moves_focused_window() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.insert_tiling(hub.current_workspace(), titled("w10"));

    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    ");
}

#[test]
fn move_to_monitor_by_name() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w11"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(0), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    ");
}

#[test]
fn move_float_to_monitor() {
    use crate::action::MonitorTarget;
    use crate::core::Dimension;

    let mut hub = setup();
    hub.insert_float(
        hub.current_workspace(),
        Dimension::new(
            Length::new(10.0),
            Length::new(10.0),
            Length::new(50.0),
            Length::new(20.0),
        ),
        titled("w12"),
    );

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    ");
}

#[test]
fn monitor_noop_cases() {
    use crate::action::MonitorTarget;

    // Single monitor: focus_monitor is no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w13"));
        let before = snapshot_text(&hub);
        hub.focus_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Single monitor with tiling: move_focused_to_monitor is no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w14"));
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Two monitors, move to same monitor: no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w15"));
        hub.add_monitor(
            "external".to_string(),
            Dimension::new(
                Length::new(150.0),
                Length::new(0.0),
                Length::new(100.0),
                Length::new(30.0),
            ),
            1.0,
        );
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Name("primary".to_string()));
        assert_eq!(snapshot_text(&hub), before);
    }

    // Two monitors, no windows: move_focused_to_monitor is no-op
    {
        let mut hub = setup();
        hub.add_monitor(
            "right-monitor".to_string(),
            Dimension::new(
                Length::new(150.0),
                Length::new(0.0),
                Length::new(100.0),
                Length::new(30.0),
            ),
            1.0,
        );
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }
}

// Has to be gated behind windows rn, since Hub is not generic over unit type
#[cfg(target_os = "windows")]
#[test]
fn monitor_scale_multiplies_tab_bar_height() {
    let l = LayoutConfigBuilder::new()
        .with_partition_tree_config(
            PartitionTreeConfigBuilder::new()
                .with_tab_bar_height(Length::<Logical>::new(5.0))
                .with_automatic_tiling(true)
                .build(),
        )
        .build();
    let mut hub = TestHubBuilder::new().with_scale(2.0).with_layout(l).build();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.toggle_container_layout();
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");

    let monitor_id = hub.focused_monitor();
    hub.update_monitor(
        monitor_id,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1000.0),
            Length::new(1000.0),
        ),
        3.0,
    );
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=1000.00 h=1000.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=1000.00, h=985.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=1000.00, h=1000.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");
}

#[cfg(target_os = "windows")]
#[test]
fn monitor_scale_multiplies_size_constraints() {
    let mut hub = TestHubBuilder::new()
        .with_scale(2.0)
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Length::<Logical>::new(10.0))
                        .with_automatic_tiling(false)
                        .build(),
                )
                // At scale of 2.0, min width should be 80
                .with_min_width(SizeConstraint::Pixels(Length::new(40.0)))
                .build(),
        )
        .build();
    for i in 0..6 {
        hub.insert_tiling(hub.current_workspace(), titled(format!("w{i}").as_str()));
    }
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=70.00, y=0.00, w=80.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=0.00, y=0.00, w=70.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2, w3, w4, w5])
      )
    ");

    let monitor_id = hub.focused_monitor();
    hub.update_monitor(
        monitor_id,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(500.0),
            Length::new(1000.0),
        ),
        // At scale of 3.0, min width should be 120
        3.0,
    );

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=500.00 h=1000.00),
        Window(id=WindowId(5), x=380.00, y=0.00, w=120.00, h=1000.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=260.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(3), x=140.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(2), x=20.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=20.00, h=1000.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=500.00, h=1000.00, titles=[w0, w1, w2, w3, w4, w5])
      )
    ");
}
