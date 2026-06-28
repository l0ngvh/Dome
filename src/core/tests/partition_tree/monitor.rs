use crate::action::MonitorTarget;
use crate::core::node::{Dimension, Length};
use crate::core::tests::{setup, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn move_container_to_monitor() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
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
    hub.focus_parent();
    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=200.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=150.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=150.00, y=0.00, w=100.00, h=30.00, titles=[w0, w1])
      )
    ");
}

#[test]
fn move_container_to_monitor_no_target() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
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
    hub.focus_parent();
    // No monitor to the left, should be a no-op
    hub.move_focused_to_monitor(&MonitorTarget::Left);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w2, w3])
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00))

    ******************************************************************************************************************************************************
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                    W0                                   ||                                    W1                                   *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    *                                                                         ||                                                                         *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn move_container_to_monitor_with_floats_on_workspace() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.toggle_float();
    hub.focus_left();
    hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.focus_parent();
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
    // Should move the tiling container (W0+W2), not the float W1
    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, float, highlighted)
      )
      Monitor(id=MonitorId(1), screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=200.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=150.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(1), x=150.00, y=0.00, w=100.00, h=30.00, titles=[w4, w6])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                    F1                                   *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}
