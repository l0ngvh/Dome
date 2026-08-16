use crate::config::{MasterConfig, SizeConstraint, Strategy};
use crate::core::WindowRestrictions;
use crate::core::node::{Length, LimitObservation, LimitUpdate, Pixels};
use crate::core::strategy::TilingAction;
use crate::core::tests::{LayoutConfigBuilder, TestHubBuilder, default_rect, snapshot, titled};
use insta::assert_snapshot;

#[test]
fn min_height_master_pane_overflows_and_scrolls_to_focus() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 4,
                })
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w3"), default_rect(), WindowRestrictions::None)
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
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=8.00)
        Window(id=WindowId(3), x=0.00, y=8.00, w=150.00, h=22.00, highlighted)
      )

    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
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
    *                                                                         W3                                                                         *
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
fn min_height_stack_pane_overflows_independently_of_master() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 2,
                })
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w4"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w5"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w6"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w7"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w8"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("w9"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w4,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w5,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(5), x=75.00, y=8.00, w=75.00, h=22.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W5                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn both_panes_scroll_independently() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 4,
                })
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w10"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w11"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w12"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w13"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w6 = hub
        .insert_window(titled("w16"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w7 = hub
        .insert_window(titled("w17"), default_rect(), WindowRestrictions::None)
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
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w4,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w5,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w6,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w7,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );

    hub.set_focus(w3);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(3), x=0.00, y=8.00, w=75.00, h=22.00, highlighted)
        Window(id=WindowId(6), x=75.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(7), x=75.00, y=8.00, w=75.00, h=22.00)
      )

    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W2                                   ||                                    W6                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W3                                   *|                                    W7                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn min_width_both_panes_meet_min_layout_overflows_screen() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w18"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w19"), default_rect(), WindowRestrictions::None)
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
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=30.00, highlighted)
      )

    +----------------------------------------------------------------------------------------------------+************************************************
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                 W0                                                 |*                      W1                       
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    |                                                                                                    |*                                               
    +----------------------------------------------------------------------------------------------------+************************************************
    ");
}

#[test]
fn min_width_master_alone_exceeds_screen_layout_overflows() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w20"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w21"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )

    +-----------------------------------------------------------------------------------------------------------------------------------------------------
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                         W0                                                                          
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    |                                                                                                                                                     
    +-----------------------------------------------------------------------------------------------------------------------------------------------------
    ");
}

#[test]
fn min_width_master_expands_when_only_master_constrained() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w22"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w23"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(120.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=122.00, h=30.00)
        Window(id=WindowId(1), x=122.00, y=0.00, w=28.00, h=30.00, highlighted)
      )

    +------------------------------------------------------------------------------------------------------------------------+****************************
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                           W0                                                           |*            W1            *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    |                                                                                                                        |*                          *
    +------------------------------------------------------------------------------------------------------------------------+****************************
    ");
}

#[test]
fn max_height_centers_window_in_pane_slot() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w24"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w25"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=9.00, w=75.00, h=12.00, highlighted)
      )

    +-------------------------------------------------------------------------+                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |***************************************************************************
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
    |                                                                         |***************************************************************************
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    |                                                                         |                                                                           
    +-------------------------------------------------------------------------+
    ");
}

#[test]
fn max_width_centers_window_in_stack_pane() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w26"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w27"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=97.00, y=0.00, w=32.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+                      ********************************                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                    W0                                   |                      *              W1              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    |                                                                         |                      *                              *                     
    +-------------------------------------------------------------------------+                      ********************************
    ");
}

#[test]
fn max_width_centers_window_in_master_pane() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w28"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w29"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=17.00, y=0.00, w=42.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

                     +----------------------------------------+                ***************************************************************************
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                   W0                   |                *                                    W1                                   *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     |                                        |                *                                                                         *
                     +----------------------------------------+                ***************************************************************************
    ");
}

#[test]
fn global_min_above_window_max_caps_min_to_max() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_min_width(SizeConstraint::Pixels(Pixels::new(100)))
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w30"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    // The global 100 min exceeds the window's own 40 max. Without the cap the
    // layout would receive an inverted (min=100, max=40) pair.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=54.00, y=0.00, w=42.00, h=30.00, highlighted)
      )

                                                          ******************************************                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                   W0                   *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          *                                        *                                                      
                                                          ******************************************
    ");
}

#[test]
fn master_count_increment_clamps_stack_scroll() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w30"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w31"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w32"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w33"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w34"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w4,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.handle_tiling_action(TilingAction::MoreMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(1), x=0.00, y=8.00, w=75.00, h=22.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(4), x=75.00, y=8.00, w=75.00, h=22.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
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
    |                                    W1                                   |*                                    W4                                   *
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
fn master_count_decrement_clamps_master_scroll() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 4,
                })
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w35"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w36"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w37"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w38"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("w39"), default_rect(), WindowRestrictions::None)
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
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    // focus_left lands on w3, the last-inserted master window, scrolling master to the bottom
    // so the offset is out of range once master shrinks.
    hub.focus_left();
    hub.handle_tiling_action(TilingAction::FewerMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(2), x=0.00, y=8.00, w=75.00, h=22.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=22.00, highlighted)
        Window(id=WindowId(4), x=75.00, y=22.00, w=75.00, h=8.00)
      )

    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |***************************************************************************
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn detach_clamps_scroll() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .with_master_config(MasterConfig {
                    master_ratio: 0.5,
                    master_count: 4,
                })
                .build(),
        )
        .build();
    let w0 = hub
        .insert_window(titled("w40"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w41"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w42"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w43"), default_rect(), WindowRestrictions::None)
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
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.delete_window(w3);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=8.00)
        Window(id=WindowId(2), x=0.00, y=8.00, w=150.00, h=22.00, highlighted)
      )

    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
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
    *                                                                         W2                                                                         *
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
fn attach_does_not_disturb_other_pane_scroll() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w44"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w45"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w46"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w47"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w48"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w4,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    let w5 = hub
        .insert_window(titled("w49"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w5,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(5), x=75.00, y=8.00, w=75.00, h=22.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W5                                   *
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
fn apply_config_relayouts_and_clamps_scroll() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    let _w0 = hub
        .insert_window(titled("w50"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w51"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w52"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w53"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w54"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w4,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    let l = LayoutConfigBuilder::new()
        .with_strategy(Strategy::Master)
        .with_master_config(MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
        })
        .build();
    hub.sync_configuration(l);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=8.00)
        Window(id=WindowId(4), x=75.00, y=8.00, w=75.00, h=22.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
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
