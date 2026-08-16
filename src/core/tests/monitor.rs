use insta::assert_snapshot;

use super::LayoutConfigBuilder;
#[cfg(target_os = "windows")]
use super::{PartitionTreeConfigBuilder, TestHubBuilder};
#[cfg(target_os = "windows")]
use crate::config::SizeConstraint;
use crate::core::GlobalLayoutConfig;
#[cfg(target_os = "windows")]
use crate::core::hub::MonitorLayout;
#[cfg(target_os = "windows")]
use crate::core::node::Pixels;
use crate::core::node::{PixelRect, WindowRestrictions};

use crate::core::tests::{
    default_rect, setup, setup_with_layout, snapshot, snapshot_text, titled, titled_matcher,
};

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn add_monitor_creates_workspace_on_new_monitor() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(
        "monitor-1".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);

    let primary = hub.focused_monitor();
    let m1 = hub.add_monitor(
        "monitor-1".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    );

    hub.focus_workspace("monitor-1");
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);

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
    let m1 = hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);

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
        PixelRect::new(150, 0, 100, 30),
        1.0,
    );
    hub.remove_monitor(m1, m1);
}

#[test]
fn update_monitor_dimension_adjusts_workspaces() {
    let mut hub = setup();
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);

    hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);

    let primary = hub.focused_monitor();
    hub.update_monitor(primary, PixelRect::new(0, 0, 200, 50), 1.0);

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
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(
        "right-monitor".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    );

    hub.add_monitor(
        "bottom-monitor".to_string(),
        PixelRect::new(0, 30, 150, 30),
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

    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    let after_name = snapshot_text(&hub);
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    assert_eq!(snapshot_text(&hub), after_name);
}

#[test]
fn focus_monitor_by_name() {
    use crate::action::MonitorTarget;

    let mut hub = setup();
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);

    hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);

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
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(
        "right-monitor".to_string(),
        PixelRect::new(150, 0, 100, 30),
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
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);

    hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);

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

    let mut hub = setup_with_layout(layout_floating(&["w12"]));
    hub.insert_window(
        titled("w12"),
        PixelRect::new(10, 10, 50, 20),
        WindowRestrictions::None,
    )
    .unwrap();

    hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);

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

    {
        let mut hub = setup();
        hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);
        let before = snapshot_text(&hub);
        hub.focus_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    {
        let mut hub = setup();
        hub.insert_window(titled("w14"), default_rect(), WindowRestrictions::None);
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // The hub starts focused on the monitor named "primary".
    {
        let mut hub = setup();
        hub.insert_window(titled("w15"), default_rect(), WindowRestrictions::None);
        hub.add_monitor("external".to_string(), PixelRect::new(150, 0, 100, 30), 1.0);
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Name("primary".to_string()));
        assert_eq!(snapshot_text(&hub), before);
    }

    {
        let mut hub = setup();
        hub.add_monitor(
            "right-monitor".to_string(),
            PixelRect::new(150, 0, 100, 30),
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
                .with_tab_bar_height(Pixels::new(5))
                .with_automatic_tiling(true)
                .build(),
        )
        .build();
    let mut hub = TestHubBuilder::new().with_scale(2.0).with_layout(l).build();
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");

    let monitor_id = hub.focused_monitor();
    hub.update_monitor(monitor_id, PixelRect::new(0, 0, 1000, 1000), 3.0);
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
    use crate::core::node::Pixels;

    let mut hub = TestHubBuilder::new()
        .with_scale(2.0)
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Pixels::new(10))
                        .with_automatic_tiling(false)
                        .build(),
                )
                .with_min_width(SizeConstraint::Pixels(Pixels::new(40)))
                .build(),
        )
        .build();
    for i in 0..6 {
        hub.insert_window(
            titled(format!("w{i}").as_str()),
            default_rect(),
            WindowRestrictions::None,
        );
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
    hub.update_monitor(monitor_id, PixelRect::new(0, 0, 500, 1000), 3.0);

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

/// Only `Unit = Physical` scales the tab bar height, so this is the one target where an
/// integral configured height can still yield a fractional band. The scale and the odd work
/// area together put the container origin and the band height on half units, where
/// `round(y) + round(h)` diverges by a unit from the `round(y + h)` the content box uses.
#[cfg(target_os = "windows")]
#[test]
fn tabbed_band_bottom_lands_on_the_content_top() {
    let mut hub = setup_with_layout(
        LayoutConfigBuilder::new()
            .with_partition_tree_config(
                PartitionTreeConfigBuilder::new()
                    .with_tab_bar_height(Pixels::new(25))
                    .build(),
            )
            .build(),
    );
    let monitor_id = hub.focused_monitor();
    hub.update_monitor(monitor_id, PixelRect::new(0, 0, 1000, 201), 1.5);

    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();

    let placements = hub.get_visible_placements();
    let active_tab = placements.focused_window.expect("focus on the active tab");
    let MonitorLayout::Normal {
        tiling_windows,
        containers,
        ..
    } = &placements.monitors[0].layout
    else {
        panic!("expected a normally tiled monitor");
    };
    let tabbed = containers
        .iter()
        .find(|c| c.is_tabbed)
        .expect("a tabbed container");
    let content_top = tiling_windows
        .iter()
        .find(|w| w.id == active_tab)
        .expect("the active tab is placed")
        .border_box
        .y();

    assert_eq!(tabbed.tab_bar_band.bottom(), content_top);
}
