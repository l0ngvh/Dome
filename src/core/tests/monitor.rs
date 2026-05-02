use insta::assert_snapshot;

use crate::config::SizeConstraint;
use crate::core::hub::HubConfig;
use crate::core::node::Dimension;

use crate::core::tests::{setup, snapshot, snapshot_text};

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
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling();

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
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_tiling();
    hub.insert_tiling();

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
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
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
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
        1.0,
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
        1.0,
    );

    let primary = hub.focused_monitor();
    hub.update_monitor(
        primary,
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 50.0,
        },
        1.0,
    );

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=200.00 h=50.00),
        Window(id=WindowId(1), x=100.00, y=0.00, w=100.00, h=50.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=50.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=200.00, h=50.00, titles=[, ])
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))
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
        1.0,
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
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
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
    hub.insert_tiling();

    hub.add_monitor(
        "external".to_string(),
        Dimension {
            x: 150.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
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
        hub.insert_tiling();
        let before = snapshot_text(&hub);
        hub.focus_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Single monitor with tiling: move_focused_to_monitor is no-op
    {
        let mut hub = setup();
        hub.insert_tiling();
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Two monitors, move to same monitor: no-op
    {
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
            Dimension {
                x: 150.0,
                y: 0.0,
                width: 100.0,
                height: 30.0,
            },
            1.0,
        );
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }
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
        1.0,
    );

    let original_monitor = hub.focused_monitor();
    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_eq!(hub.focused_monitor(), original_monitor);
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
fn monitor_scale_multiplies_config_lengths() {
    use crate::core::hub::{Hub, MonitorLayout};

    /// Extract tiling window frames from a Hub's visible placements.
    fn tiling_frames(hub: &Hub) -> Vec<Dimension> {
        let vp = hub.get_visible_placements();
        let mut frames = Vec::new();
        for mp in &vp.monitors {
            if let MonitorLayout::Normal { tiling_windows, .. } = &mp.layout {
                for tw in tiling_windows {
                    frames.push(tw.frame);
                }
            }
        }
        frames
    }

    // Part 1: tab_bar_height is multiplied by scale.
    // scale=2.0, tab_bar_height config=20.0 -> physical tab bar = 40px.
    // Need 2+ windows so insert_tiling creates a split container that
    // toggle_container_layout can convert to Tabbed. A single root-level
    // window has no parent container to toggle.
    let mut hub = Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        },
        2.0,
        HubConfig {
            tab_bar_height: 20.0,
            auto_tile: true,
            min_width: SizeConstraint::Pixels(0.0),
            min_height: SizeConstraint::Pixels(0.0),
            max_width: SizeConstraint::Pixels(0.0),
            max_height: SizeConstraint::Pixels(0.0),
        },
    );
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_container_layout(); // switch split -> Tabbed

    // Tabbed container shows only the active tab's window.
    let frames = tiling_frames(&hub);
    assert_eq!(frames.len(), 1);
    let w = frames[0];
    // tab_bar_height 20.0 * scale 2.0 = 40.0 consumed from monitor height
    assert!(
        (w.y - 40.0).abs() < 0.01,
        "expected y=40.0 (tab_bar_height*scale), got {}",
        w.y
    );
    assert!(
        (w.height - 960.0).abs() < 0.01,
        "expected height=960.0 (1000-40), got {}",
        w.height
    );

    // After update_monitor with scale=3.0 -> tab_bar = 20*3 = 60
    let monitor_id = hub.focused_monitor();
    hub.update_monitor(
        monitor_id,
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        },
        3.0,
    );
    let frames = tiling_frames(&hub);
    let w = frames[0];
    assert!(
        (w.y - 60.0).abs() < 0.01,
        "expected y=60.0 (tab_bar_height*3.0), got {}",
        w.y
    );
    assert!(
        (w.height - 940.0).abs() < 0.01,
        "expected height=940.0 (1000-60), got {}",
        w.height
    );

    // Part 2: SizeConstraint::Pixels is multiplied by scale.
    // 6 windows, screen 1000px wide, scale=2.0, min_width=Pixels(100).
    // Equal split would give ~166.7px each, but scaled min = 100*2 = 200,
    // so the min-width clamp must apply.
    let mut hub2 = Hub::new(
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        },
        2.0,
        HubConfig {
            tab_bar_height: 20.0,
            auto_tile: true,
            min_width: SizeConstraint::Pixels(100.0),
            min_height: SizeConstraint::Pixels(0.0),
            max_width: SizeConstraint::Pixels(0.0),
            max_height: SizeConstraint::Pixels(0.0),
        },
    );
    for _ in 0..6 {
        hub2.insert_tiling();
    }
    let min_w: f32 = tiling_frames(&hub2)
        .iter()
        .map(|f| f.width)
        .fold(f32::INFINITY, f32::min);
    assert!(
        (min_w - 200.0).abs() < 0.01,
        "expected min width=200.0 (Pixels(100)*scale 2.0), got {}",
        min_w
    );

    // After update_monitor with scale=3.0 -> min_width = 100*3 = 300
    let monitor_id2 = hub2.focused_monitor();
    hub2.update_monitor(
        monitor_id2,
        Dimension {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 1000.0,
        },
        3.0,
    );
    let min_w: f32 = tiling_frames(&hub2)
        .iter()
        .map(|f| f.width)
        .fold(f32::INFINITY, f32::min);
    assert!(
        (min_w - 300.0).abs() < 0.01,
        "expected min width=300.0 (Pixels(100)*scale 3.0), got {}",
        min_w
    );
}
