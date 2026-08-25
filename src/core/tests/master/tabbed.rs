use crate::config::Strategy;
use crate::core::ContainerId;
use crate::core::WindowRestrictions;
use crate::core::allocator::NodeId;
use crate::core::hub::Hub;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, snapshot,
    titled,
};
use insta::assert_snapshot;

fn master_hub(master_count: usize) -> Hub {
    TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_strategy(Strategy::Master)
                .with_master_count(master_count)
                .build(),
        ])
        .build()
}

#[test]
fn toggle_master_pane_to_tabbed() {
    let mut hub = master_hub(2);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn single_window_pane_tabbed_shows_no_bar() {
    let mut hub = master_hub(1);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted)
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
fn secondary_pane_tabbed() {
    let mut hub = master_hub(1);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    // Master holds W0, secondary holds W1 and W2. Focus is on W2 in secondary.
    hub.toggle_container_layout();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(2), x=75.00, y=2.00, w=75.00, h=28.00, highlighted)
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, tabbed, active_tab=1, titles=[W1, W2])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                W1                  |               [W2]                 |
    |                                                                         |***************************************************************************
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
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                    W2                                   *
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
fn focus_tab_cycles_active_window() {
    let mut hub = master_hub(3);
    let w0 = hub
        .insert_window(titled("W0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("W1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    let ws = hub.current_workspace();
    hub.toggle_container_layout();

    // Focus starts on W2 (index 2). Forward wraps to W0, then to W1.
    hub.focus_next_tab();
    assert_eq!(hub.focused_window(ws), Some(w0));
    hub.focus_next_tab();
    assert_eq!(hub.focused_window(ws), Some(w1));
    hub.focus_prev_tab();
    assert_eq!(hub.focused_window(ws), Some(w0));
}

#[test]
fn focus_tab_wraps_backward() {
    let mut hub = master_hub(3);
    let w0 = hub
        .insert_window(titled("W0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    hub.set_focus(w0);
    hub.toggle_container_layout();

    // W0 is at index 0. Backward wraps to the last tab, W2.
    hub.focus_prev_tab();
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn tab_click_focuses_window() {
    let mut hub = master_hub(3);
    let w0 = hub
        .insert_window(titled("W0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("W1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    let ws = hub.current_workspace();
    hub.toggle_container_layout();

    hub.focus_tab_index(ContainerId::new(0), 0);
    assert_eq!(hub.focused_window(ws), Some(w0));
    hub.focus_tab_index(ContainerId::new(0), 1);
    assert_eq!(hub.focused_window(ws), Some(w1));
}

#[test]
fn tab_click_out_of_range_is_noop() {
    let mut hub = master_hub(3);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    hub.toggle_container_layout();

    hub.focus_tab_index(ContainerId::new(0), 99);
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn focus_tab_is_noop_on_tiled_pane() {
    let mut hub = master_hub(3);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    // The pane is still tiled, so cycling tabs is a no-op.
    hub.focus_next_tab();
    assert_eq!(hub.focused_window(ws), Some(w2));
}

#[test]
fn toggle_off_restores_tiled() {
    let mut hub = master_hub(3);
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    let w2 = hub
        .insert_window(titled("W2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();
    hub.toggle_container_layout();
    hub.toggle_container_layout();

    hub.focus_next_tab();
    assert_eq!(hub.focused_window(ws), Some(w2));
}
