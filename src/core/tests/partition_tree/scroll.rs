use insta::assert_snapshot;

use crate::{
    config::SizeConstraint,
    core::{
        Length, LimitObservation, LimitUpdate, Pixels, WindowRestrictions,
        tests::{LayoutConfigBuilder, default_rect, setup, snapshot, titled},
    },
};

#[test]
fn scroll_vertically_to_focus() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
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
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=22.00, w=150.00, h=8.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=22.00, highlighted, spawn=bottom)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
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
    ******************************************************************************************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    ");
}

#[test]
fn scroll_horizontally_to_focus() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w4"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w3 = hub
        .insert_window(titled("w5"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(90.0)),
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
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=144.00, y=15.00, w=6.00, h=15.00)
        Window(id=WindowId(2), x=52.00, y=15.00, w=92.00, h=15.00)
        Window(id=WindowId(1), x=52.00, y=0.00, w=98.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=52.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, Container])
        Container(id=ContainerId(1), x=52.00, y=0.00, w=98.00, h=30.00, titles=[w3, Container])
        Container(id=ContainerId(2), x=52.00, y=15.00, w=98.00, h=15.00, titles=[w4, w5])
      )

    ****************************************************+-------------------------------------------------------------------------------------------------
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                               W1                                                
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *|                                                                                                 
    *                                                  *+-------------------------------------------------------------------------------------------------
    *                        W0                        *+------------------------------------------------------------------------------------------++-----
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                            W2                                            || W3  
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    *                                                  *|                                                                                          ||     
    ****************************************************+------------------------------------------------------------------------------------------++-----
    ");
}

#[test]
fn scroll_container_into_focus() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w6"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode(); // vertical
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);
    let w3 = hub
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
        w3,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            ..Default::default()
        },
    );

    hub.focus_parent();
    hub.focus_parent();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=102.00, y=20.00, w=48.00, h=10.00)
        Window(id=WindowId(2), x=102.00, y=10.00, w=48.00, h=10.00)
        Window(id=WindowId(1), x=102.00, y=0.00, w=48.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=102.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w6, Container])
        Container(id=ContainerId(1), x=102.00, y=0.00, w=48.00, h=30.00, titles=[w7, w8, w9])
      )

    ******************************************************************************************************************************************************
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                      W1                       
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    |+-----------------------------------------------
    *                                                                                                    |+-----------------------------------------------
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                 W0                                                 ||                      W2                       
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    |+-----------------------------------------------
    *                                                                                                    |+-----------------------------------------------
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                      W3                       
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    *                                                                                                    ||                                               
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn scroll_window_into_view_in_vertical_child_container() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w10"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let w1 = hub
        .insert_window(titled("w11"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w12"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w1,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            min_height: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=102.00, y=0.00, w=48.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=8.00, w=102.00, h=22.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=102.00, h=8.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w13])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=102.00, h=30.00, titles=[w10, w11, w12])
      )

    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                 W1                                                 ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    +----------------------------------------------------------------------------------------------------+|                                              |
    ******************************************************************************************************|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                      W3                      |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                 W2                                                 *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    ******************************************************************************************************+----------------------------------------------+
    ");

    hub.delete_window(w0);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=102.00, y=0.00, w=48.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=8.00, w=102.00, h=22.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=102.00, h=8.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w13])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=102.00, h=30.00, titles=[w11, w12])
      )

    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    |                                                 W1                                                 ||                                              |
    |                                                                                                    ||                                              |
    |                                                                                                    ||                                              |
    +----------------------------------------------------------------------------------------------------+|                                              |
    ******************************************************************************************************|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                      W3                      |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                 W2                                                 *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    *                                                                                                    *|                                              |
    ******************************************************************************************************+----------------------------------------------+
    ");
}

#[test]
fn scroll_view_port_also_scroll_max_constrained_window() {
    let mut hub = setup();

    let l = LayoutConfigBuilder::new()
        .with_max_height(SizeConstraint::Pixels(Pixels::new(10)))
        .with_min_height(SizeConstraint::Pixels(Pixels::new(7)))
        .build();
    hub.sync_configuration(l);

    let w0 = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w15"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w18"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w19"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w20"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w21"), default_rect(), WindowRestrictions::None);
    hub.set_focus(w0);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w22"), default_rect(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(8))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(7), x=75.00, y=23.00, w=75.00, h=7.00)
        Window(id=WindowId(6), x=75.00, y=16.00, w=75.00, h=7.00)
        Window(id=WindowId(5), x=75.00, y=9.00, w=75.00, h=7.00)
        Window(id=WindowId(4), x=75.00, y=2.00, w=75.00, h=7.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=2.00)
        Window(id=WindowId(8), x=0.00, y=15.00, w=75.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=0.00, y=5.00, w=75.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[w15, w16, w17, w18, w19, w20, w21])
        Container(id=ContainerId(2), x=0.00, y=0.00, w=75.00, h=30.00, titles=[w14, w22])
      )

                                                                               |                                                                         |
                                                                               +------------------------------------W3-----------------------------------+
                                                                               +-------------------------------------------------------------------------+
                                                                               |                                                                         |
                                                                               |                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W0                                   ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W5                                   |
    +-------------------------------------------------------------------------+|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W8                                   *|                                    W6                                   |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                                                         *+-------------------------------------------------------------------------+
    ***************************************************************************|                                                                         |
                                                                               |                                                                         |
                                                                               |                                                                         |
                                                                               |                                    W7                                   |
                                                                               |                                                                         |
                                                                               +-------------------------------------------------------------------------+
    ");
}

#[test]
fn laying_out_max_constrained_windows_leaves_no_hole() {
    let mut hub = setup();

    let l = LayoutConfigBuilder::new()
        .with_max_height(SizeConstraint::Pixels(Pixels::new(30)))
        .with_min_height(SizeConstraint::Pixels(Pixels::new(7)))
        .with_min_width(SizeConstraint::Pixels(Pixels::new(30)))
        .build();
    hub.sync_configuration(l);

    let w0 = hub
        .insert_window(titled("w23"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w24"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(120.0)),
            ..Default::default()
        },
    );
    hub.toggle_spawn_mode();
    let w2 = hub
        .insert_window(titled("w25"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w2,
        LimitObservation {
            min_height: LimitUpdate::Set(Length::new(25.)),
            ..Default::default()
        },
    );
    hub.toggle_spawn_mode();
    let w3 = hub
        .insert_window(titled("w26"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(
        w3,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(50.)),
            ..Default::default()
        },
    );
    hub.insert_window(titled("w27"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    let w5 = hub
        .insert_window(titled("w28"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w29"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w30"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w31"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w32"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w33"), default_rect(), WindowRestrictions::None);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(10))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(10), x=120.00, y=17.00, w=30.00, h=13.00, highlighted, spawn=right)
        Window(id=WindowId(9), x=90.00, y=17.00, w=30.00, h=13.00)
        Window(id=WindowId(8), x=60.00, y=17.00, w=30.00, h=13.00)
        Window(id=WindowId(7), x=30.00, y=17.00, w=30.00, h=13.00)
        Window(id=WindowId(6), x=0.00, y=17.00, w=30.00, h=13.00)
        Window(id=WindowId(4), x=0.00, y=3.00, w=150.00, h=14.00)
        Window(id=WindowId(1), x=14.00, y=0.00, w=122.00, h=3.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, Container])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(2), x=0.00, y=3.00, w=150.00, h=27.00, titles=[w25, w26, Container])
        Container(id=ContainerId(3), x=0.00, y=3.00, w=150.00, h=27.00, titles=[w27, Container])
        Container(id=ContainerId(4), x=0.00, y=17.00, w=150.00, h=13.00, titles=[w28, w29, w30, w31, w32, w33])
      )

                  |                                                                                                                        |              
                  |                                                                                                                        |              
                  +-----------------------------------------------------------W1-----------------------------------------------------------+              
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                              W4                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
                                                                                                                                                         |
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
    +----------------------------++----------------------------++----------------------------++----------------------------+******************************
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |             W6             ||             W7             ||             W8             ||             W9             |*             W10            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    +----------------------------++----------------------------++----------------------------++----------------------------+******************************
    ");

    // reset viewport
    hub.set_focus(w0);

    hub.set_focus(w5);

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(6), x=142.00, y=17.00, w=8.00, h=13.00)
        Window(id=WindowId(5), x=112.00, y=17.00, w=30.00, h=13.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=112.00, y=3.00, w=38.00, h=14.00)
        Window(id=WindowId(3), x=60.00, y=3.00, w=52.00, h=27.00)
        Window(id=WindowId(2), x=30.00, y=3.00, w=30.00, h=27.00)
        Window(id=WindowId(1), x=30.00, y=0.00, w=120.00, h=3.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, Container])
        Container(id=ContainerId(1), x=30.00, y=0.00, w=120.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(2), x=30.00, y=3.00, w=120.00, h=27.00, titles=[w25, w26, Container])
        Container(id=ContainerId(3), x=112.00, y=3.00, w=38.00, h=27.00, titles=[w27, Container])
        Container(id=ContainerId(4), x=112.00, y=17.00, w=38.00, h=13.00, titles=[w28, w29, w30, w31, w32, w33])
      )

    +----------------------------+|                                                                                                                       
    |                            ||                                                                                                                       
    |                            |+----------------------------------------------------------W1-----------------------------------------------------------
    |                            |+----------------------------++--------------------------------------------------++-------------------------------------
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                 W4                  
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  ||                                     
    |             W0             ||                            ||                                                  ||                                     
    |                            ||                            ||                                                  |+-------------------------------------
    |                            ||             W2             ||                        W3                        |******************************+-------
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*             W5             *|  W6   
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    |                            ||                            ||                                                  |*                            *|       
    +----------------------------++----------------------------++--------------------------------------------------+******************************+-------
    ");
}
