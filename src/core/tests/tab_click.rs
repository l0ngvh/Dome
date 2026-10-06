use crate::core::{
    ContainerPlacement, Hub, MonitorId, MonitorLayout, MonitorSelector, PaneDisplay, Strategy,
    WindowRestrictions,
};

use super::{
    LayoutWorkspaceConfigBuilder, PRIMARY_MONITOR, TestHubBuilder, default_rect, reported_monitor,
    setup, snapshot, titled, titled_matcher, work_area_at,
};

/// The one tab bar that `get_visible_placements` gives for `monitor`, so a test clicks the
/// container id an overlay would send back.
fn drawn_tab_bar(hub: &Hub, monitor: MonitorId) -> ContainerPlacement {
    let placements = hub
        .get_visible_placements()
        .monitors
        .into_iter()
        .find(|placements| placements.monitor_id == monitor)
        .expect("the monitor shows a workspace");
    let MonitorLayout::Normal { containers, .. } = placements.layout else {
        panic!("monitor {monitor} shows a fullscreen window");
    };
    let mut tab_bars = containers.into_iter().filter(|c| c.is_tabbed);
    let tab_bar = tab_bars.next().expect("the monitor draws a tab bar");
    assert!(
        tab_bars.next().is_none(),
        "monitor {monitor} draws more than one tab bar"
    );
    tab_bar
}

#[test]
fn tab_click_on_another_monitor_reaches_its_master_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout_on(
            "monitor-1",
            [LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .with_master_display(PaneDisplay::Tabbed)
                .build()],
        )
        .build();
    let primary_ws = hub.current_workspace();
    let p0 = hub
        .insert_window(titled("P0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let monitor_1 = hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("monitor-1".to_string()));
    let master_ws = hub.current_workspace();
    let m0 = hub
        .insert_window(titled("M0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("M1"), default_rect(), WindowRestrictions::None);
    hub.focus_monitor(&MonitorSelector::Name(PRIMARY_MONITOR.to_string()));
    let tab_bar = drawn_tab_bar(&hub, monitor_1);
    assert_eq!(tab_bar.active_tab_index, 1);

    hub.focus_tab_index(tab_bar.id, 0);

    assert_eq!(drawn_tab_bar(&hub, monitor_1).active_tab_index, 0);
    assert_eq!(hub.focused_window(master_ws), Some(m0));
    assert_eq!(hub.current_workspace(), primary_ws);
    assert_eq!(hub.focused_window(primary_ws), Some(p0));
    hub.validate();
}

#[test]
fn tab_click_on_another_monitor_reaches_its_partition_tree_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(Strategy::Master)
            .build()])
        .build();
    let primary_ws = hub.current_workspace();
    let p0 = hub
        .insert_window(titled("P0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let monitor_1 = hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("monitor-1".to_string()));
    let tree_ws = hub.current_workspace();
    let t0 = hub
        .insert_window(titled("T0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("T1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_monitor(&MonitorSelector::Name(PRIMARY_MONITOR.to_string()));
    let tab_bar = drawn_tab_bar(&hub, monitor_1);
    assert_eq!(tab_bar.active_tab_index, 1);

    hub.focus_tab_index(tab_bar.id, 0);

    assert_eq!(drawn_tab_bar(&hub, monitor_1).active_tab_index, 0);
    assert_eq!(hub.focused_window(tree_ws), Some(t0));
    assert_eq!(hub.current_workspace(), primary_ws);
    assert_eq!(hub.focused_window(primary_ws), Some(p0));
    hub.validate();
}

#[test]
fn tab_click_on_a_container_that_no_longer_exists_does_nothing() {
    let mut hub = setup();
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("W1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_container_layout();
    let freed = drawn_tab_bar(&hub, hub.primary_monitor()).id;
    hub.delete_window(w1);
    let before = snapshot(&hub);

    hub.focus_tab_index(freed, 0);

    assert_eq!(snapshot(&hub), before);
}

#[test]
fn tab_click_is_blocked_while_a_block_all_window_has_focus() {
    let mut hub = setup();
    let monitor_1 = hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("monitor-1".to_string()));
    hub.insert_window(titled("T0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("T1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.focus_monitor(&MonitorSelector::Name(PRIMARY_MONITOR.to_string()));
    hub.insert_window(
        titled("locked"),
        default_rect(),
        WindowRestrictions::BlockAll,
    );
    let tab_bar = drawn_tab_bar(&hub, monitor_1);
    assert_eq!(tab_bar.active_tab_index, 1);
    let before = snapshot(&hub);

    hub.focus_tab_index(tab_bar.id, 0);

    assert_eq!(snapshot(&hub), before);
}

#[test]
fn tab_click_takes_focus_from_a_float_on_a_master_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(Strategy::Master)
            .with_master_count(2)
            .with_master_display(PaneDisplay::Tabbed)
            .with_float(vec![titled_matcher("F")])
            .build()])
        .build();
    let ws = hub.current_workspace();
    let m0 = hub
        .insert_window(titled("M0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("M1"), default_rect(), WindowRestrictions::None);
    let f = hub
        .insert_window(titled("F"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_eq!(hub.focused_window(ws), Some(f));
    let tab_bar = drawn_tab_bar(&hub, hub.primary_monitor());

    hub.focus_tab_index(tab_bar.id, 0);

    assert_eq!(
        drawn_tab_bar(&hub, hub.primary_monitor()).active_tab_index,
        0
    );
    assert_eq!(hub.focused_window(ws), Some(m0));
    hub.validate();
}
