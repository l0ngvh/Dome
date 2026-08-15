use insta::assert_snapshot;

use crate::config::SizeConstraint;

use crate::core::node::{Length, LimitObservation, LimitUpdate, Logical, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, PartitionTreeConfigBuilder, default_rect, setup, snapshot, titled,
};

#[test]
fn set_min_size_respects_minimum_height() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=22.00, w=150.00, h=8.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=22.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
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
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn set_min_size_distributes_remaining_space_equally() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=126.00, y=0.00, w=24.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=102.00, y=0.00, w=24.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, w3, w4])
      )

    +----------------------------------------------------------------------------------------------------++----------------------+************************
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                 W0                                                 ||          W1          |*          W2          *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    |                                                                                                    ||                      |*                      *
    +----------------------------------------------------------------------------------------------------++----------------------+************************
    ");
}

#[test]
fn set_min_size_propagates_to_parent_container() {
    let mut hub = setup();
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w7"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=48.00, y=15.00, w=102.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=48.00, y=0.00, w=102.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=48.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w5, Container])
        Container(id=ContainerId(1), x=48.00, y=0.00, w=102.00, h=30.00, titles=[w6, w7])
      )

    +----------------------------------------------++----------------------------------------------------------------------------------------------------+
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                 W1                                                 |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              |+----------------------------------------------------------------------------------------------------+
    |                      W0                      |******************************************************************************************************
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                 W2                                                 *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    +----------------------------------------------+******************************************************************************************************
    ");
}

#[test]
fn children_combined_size_exceeds_screen_size() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w8"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w9"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=48.00, y=0.00, w=102.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=48.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w8, w9])
      )

    -----------------------------------------------+******************************************************************************************************
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                           W0                      |*                                                 W1                                                 *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
    -----------------------------------------------+******************************************************************************************************
    ");
}

#[test]
fn children_combined_size_exceeds_container_size() {
    let mut hub = setup();
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w12"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w3 = hub
        .insert_window(titled("w13"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=48.00, y=15.00, w=102.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=48.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w10, Container])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w11, Container])
        Container(id=ContainerId(2), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w12, w13])
      )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                              W1                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -----------------------------------------------+******************************************************************************************************
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                           W2                      |*                                                 W3                                                 *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
    -----------------------------------------------+******************************************************************************************************
    ");
}

#[test]
fn children_combined_size_exceeds_screen_height() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=8.00, w=150.00, h=22.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=8.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w14, w15])
      )

    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn set_min_size_tabbed_child_container() {
    let mut hub = setup();
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("W1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("W2"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    let w3 = hub
        .insert_window(titled("W3"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=48.00, y=8.00, w=102.00, h=22.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=48.00, y=2.00, w=102.00, h=6.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=48.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[W0, Container])
        Container(id=ContainerId(1), x=48.00, y=0.00, w=102.00, h=30.00, tabbed, active_tab=1, titles=[W1, Container])
        Container(id=ContainerId(2), x=48.00, y=2.00, w=102.00, h=28.00, titles=[W2, W3])
      )

    +----------------------------------------------++----------------------------------------------------------------------------------------------------+
    |                                              ||                       W1                         |                  [Container]                    |
    |                                              |+----------------------------------------------------------------------------------------------------+
    |                                              ||                                                                                                    |
    |                                              ||                                                                                                    |
    |                                              ||                                                 W2                                                 |
    |                                              ||                                                                                                    |
    |                                              |+----------------------------------------------------------------------------------------------------+
    |                                              |******************************************************************************************************
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                      W0                      |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                 W3                                                 *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    |                                              |*                                                                                                    *
    +----------------------------------------------+******************************************************************************************************
    ");
}

#[test]
fn delete_window_with_min_size_shrinks_parent_container() {
    let mut hub = setup();
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w17"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w18"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w19"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    // Container min_width = 300 (w1 + w2 + w3), exceeds screen width 150
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=48.00, y=15.00, w=102.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=48.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w16, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w17, w18, w19])
      )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                              W0                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -----------------------------------------------+******************************************************************************************************
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                           W2                      |*                                                 W3                                                 *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
    -----------------------------------------------+******************************************************************************************************
    ");

    hub.delete_window(w1);

    // After deleting w1, container min_width drops to 200 (w2 + w3)
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=48.00, y=15.00, w=102.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=15.00, w=48.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w16, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w18, w19])
      )

    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                              W0                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    -----------------------------------------------+******************************************************************************************************
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                           W2                      |*                                                 W3                                                 *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
                                                   |*                                                                                                    *
    -----------------------------------------------+******************************************************************************************************
    ");
}

#[test]
fn delete_window_with_min_size_allows_siblings_to_expand() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w20"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w21"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w20, w21])
      )

    +----------------------------------------------------------------------------------------------------+************************************************
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                 W0                                                 |*                      W1                      *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    +----------------------------------------------------------------------------------------------------+************************************************
    ");

    hub.delete_window(w0);

    // After deleting w0, w1 expands to full screen width
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
fn max_height_centers_window_vertically_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w22"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w23"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(15.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=7.00, w=75.00, h=17.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w22, w23])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W1                                   *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}

#[test]
fn max_width_centers_window_horizontally_in_vertical_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w24"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w25"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=150.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=49.00, y=0.00, w=52.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, w25])
      )

                                                     +--------------------------------------------------+                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                        W0                        |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     |                                                  |                                                 
                                                     +--------------------------------------------------+                                                 
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn max_width_limits_window_in_horizontal_split() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w26"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w27"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=32.00, y=0.00, w=118.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=32.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w26, w27])
      )

    +------------------------------+**********************************************************************************************************************
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |              W0              |*                                                         W1                                                         *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    |                              |*                                                                                                                    *
    +------------------------------+**********************************************************************************************************************
    ");
}

#[test]
fn both_windows_at_max_centered_collectively() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w28"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w29"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=32.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=43.00, y=0.00, w=32.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w28, w29])
      )

                                               +------------------------------+********************************                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |              W0              |*              W1              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               |                              |*                              *                                           
                                               +------------------------------+********************************
    ");
}

#[test]
fn tabbed_window_with_max_size_is_centered() {
    let mut hub = setup();
    hub.insert_window(titled("W0"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("W1"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(60.0)),
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=44.00, y=10.00, w=62.00, h=12.00, highlighted, spawn=top)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[W0, W1])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                   W0                                     |                                 [W1]                                    |
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                **************************************************************                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                             W1                             *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                **************************************************************
    ");
}

#[test]
fn nested_window_center_due_to_max_constraints() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w30"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w31"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w32"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );

    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(10.0)),
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=12.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=75.00, y=3.00, w=75.00, h=12.00)
        Window(id=WindowId(0), x=0.00, y=9.00, w=75.00, h=12.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w30, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w31, w32])
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                               +-------------------------------------------------------------------------+
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
    +-------------------------------------------------------------------------+|                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                    W2                                   *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}

#[test]
fn global_max_applies_to_all_windows() {
    let mut hub = setup();
    hub.insert_window(titled("w33"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w34"), default_rect(), WindowRestrictions::None);

    let l = LayoutConfigBuilder::new()
        .with_partition_tree_config(
            PartitionTreeConfigBuilder::new()
                .with_automatic_tiling(true)
                .build(),
        )
        .with_max_width(SizeConstraint::Pixels(Length::new(60.0)))
        .build();
    hub.sync_configuration(l);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=60.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=15.00, y=0.00, w=60.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w33, w34])
      )

                   +----------------------------------------------------------+************************************************************               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                            W0                            |*                            W1                            *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   |                                                          |*                                                          *               
                   +----------------------------------------------------------+************************************************************
    ");
}

#[test]
fn per_window_max_overrides_global() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w35"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w36"), default_rect(), WindowRestrictions::None);

    let l = LayoutConfigBuilder::new()
        .with_partition_tree_config(
            PartitionTreeConfigBuilder::new()
                .with_automatic_tiling(true)
                .build(),
        )
        .with_max_width(SizeConstraint::Pixels(Length::new(60.0)))
        .build();
    hub.sync_configuration(l);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=61.00, y=0.00, w=60.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=29.00, y=0.00, w=32.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w35, w36])
      )

                                 +------------------------------+************************************************************                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |              W0              |*                            W1                            *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 |                              |*                                                          *                             
                                 +------------------------------+************************************************************
    ");
}

#[test]
fn single_window_with_max_size_centered() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w37"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(60.0)),
            max_height: LimitUpdate::Set(Length::new(15.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=44.00, y=7.00, w=62.00, h=17.00, highlighted, spawn=right)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                **************************************************************                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                             W0                             *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                *                                                            *                                            
                                                **************************************************************
    ");
}

#[test]
fn single_window_with_max_larger_than_screen_fills_screen() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w38"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(200.0)),
            max_height: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

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
fn clearing_constraint_allows_window_to_resize() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w39"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w40"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w39, w40])
      )

    +----------------------------------------------------------------------------------------------------+************************************************
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                 W0                                                 |*                      W1                      *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    +----------------------------------------------------------------------------------------------------+************************************************
    ");

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w39, w40])
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
    |                                    W0                                   |*                                    W1                                   *
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
fn new_max_clamps_existing_min() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w41"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w42"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            max_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=52.00, y=0.00, w=98.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=52.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w41, w42])
      )

    +--------------------------------------------------+**************************************************************************************************
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                        W0                        |*                                               W1                                               *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    |                                                  |*                                                                                                *
    +--------------------------------------------------+**************************************************************************************************
    ");
}

#[test]
fn setting_max_keeps_a_min_set_earlier() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w60"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w61"), default_rect(), WindowRestrictions::None)
        .unwrap();

    // macOS observes either a min or a max per axis, never both, so the two arrive in
    // separate calls. See LimitUpdate.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(140.0)),
            ..Default::default()
        },
    );

    // min_width still binds at 100 against a natural 75. Were the max call treated as
    // a replace, min would be gone and w0 would sit at 75.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w60, w61])
      )

    +----------------------------------------------------------------------------------------------------+************************************************
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                 W0                                                 |*                      W1                      *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    +----------------------------------------------------------------------------------------------------+************************************************
    ");

    // Removing the sibling frees the whole 150, so max_width binds and proves the
    // second call was stored too.
    hub.delete_window(w1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=4.00, y=0.00, w=142.00, h=30.00, highlighted, spawn=right)
      )

        **********************************************************************************************************************************************    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                     W0                                                                     *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        *                                                                                                                                            *    
        **********************************************************************************************************************************************
    ");
}

#[test]
fn raising_min_above_existing_max_raises_max() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w43"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w44"), default_rect(), WindowRestrictions::None);

    // Set max_h=10. In a horizontal split with screen height 30,
    // w0 height is capped at 10, centered vertically.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=9.00, w=75.00, h=12.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w43, w44])
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
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
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

    // Raise min_h=15 above max_h=10. If max stays at 10, the layout
    // is contradictory and the implementation must raise max to 15.
    // Observable: w0 height is now 15, not 10 and not 30.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(15.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=7.00, w=75.00, h=17.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w43, w44])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W1                                   *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");
}

#[test]
fn clearing_max_removes_the_limit() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w45"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w46"), default_rect(), WindowRestrictions::None);

    // Cap w0 height at 10. w0 takes 75x10 centered.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=9.00, w=75.00, h=12.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, w46])
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
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
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

    // Clear max_h. w0 expands to 75x30.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    let cleared = snapshot(&hub);
    assert_snapshot!(cleared, @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w45, w46])
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
    |                                    W0                                   |*                                    W1                                   *
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

    // Capping again and clearing again returns to the same layout, so clearing is
    // repeatable rather than a one-shot.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    assert_eq!(snapshot(&hub), cleared);
}

#[test]
fn setting_min_below_existing_max_keeps_max() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w47"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w48"), default_rect(), WindowRestrictions::None);

    // Cap w0 height at 20. w0 takes 75x20 centered.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    let capped = snapshot(&hub);
    assert_snapshot!(capped, @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=4.00, w=75.00, h=22.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w47, w48])
      )

                                                                               ***************************************************************************
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
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
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               *                                                                         *
                                                                               ***************************************************************************
    ");

    // Set min_h=10 below max_h=20. If max were incorrectly lowered
    // to 10, w0 would render at height 10. It should stay at 20.
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    assert_eq!(snapshot(&hub), capped);
}

#[test]
fn window_max_smaller_than_global_min_width() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w49"), default_rect(), WindowRestrictions::None)
        .unwrap();

    let l = LayoutConfigBuilder::new()
        .with_min_width(SizeConstraint::Pixels(Length::new(300.0)))
        .build();
    hub.sync_configuration(l);

    // Window max (50) < global min (300) - should not panic, window max takes precedence
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=49.00, y=0.00, w=52.00, h=30.00, highlighted, spawn=right)
      )

                                                     ****************************************************                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                        W0                        *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     *                                                  *                                                 
                                                     ****************************************************
    ");
}

#[test]
fn window_max_height_smaller_than_global_min_height() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w50"), default_rect(), WindowRestrictions::None)
        .unwrap();

    let l = LayoutConfigBuilder::new()
        .with_min_height(SizeConstraint::Pixels(Length::new(300.0)))
        .build();
    hub.sync_configuration(l);

    // Window max (10) < global min (300) - should not panic
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=9.00, w=150.00, h=12.00, highlighted, spawn=right)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
    ******************************************************************************************************************************************************
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
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn window_max_width_smaller_than_global_min_width() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w51"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w52"), default_rect(), WindowRestrictions::None);

    let l = LayoutConfigBuilder::new()
        .with_min_width(SizeConstraint::Pixels(Length::new(100.0)))
        .build();
    hub.sync_configuration(l);

    // Window max (50) < global min (100) - should not panic
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w51, w52])
      )

    -------------------------------------------------+****************************************************************************************************
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                            W0                       |*                                                W1                                                *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
                                                     |*                                                                                                  *
    -------------------------------------------------+****************************************************************************************************
    ");
}

#[test]
fn zero_valued_max_leaves_the_window_unconstrained() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w62"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w63"), default_rect(), WindowRestrictions::None);
    let unconstrained = snapshot(&hub);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::ZERO),
            max_height: LimitUpdate::Set(Length::ZERO),
            ..Default::default()
        },
    );

    assert_eq!(
        snapshot(&hub),
        unconstrained,
        "a zero-valued max must not become a 2 * border cap"
    );
}

#[test]
fn constraint_survives_border_size_change() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w53"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w54"), default_rect(), WindowRestrictions::None);

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w53, w54])
      )

    +----------------------------------------------------------------------------------------------------+************************************************
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                 W0                                                 |*                      W1                      *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    |                                                                                                    |*                                              *
    +----------------------------------------------------------------------------------------------------+************************************************
    ");

    hub.sync_configuration(
        LayoutConfigBuilder::new()
            .with_border_size(Length::<Logical>::new(5.0))
            .build(),
    );

    // The stored limit is content-box, so w53 keeps 100 of content and its
    // border box grows with the border instead.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=110.00, y=0.00, w=40.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=110.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w53, w54])
      )

    +------------------------------------------------------------------------------------------------------------+****************************************
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                     W0                                                     |*                  W1                  *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    |                                                                                                            |*                                      *
    +------------------------------------------------------------------------------------------------------------+****************************************
    ");
}
