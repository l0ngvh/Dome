use crate::config::{MasterConfig, Strategy};
use crate::core::WindowRestrictions;
use crate::core::strategy::TilingAction;
use crate::core::tests::{LayoutConfigBuilder, TestHubBuilder, default_dim, snapshot, titled};
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
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w3"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    // w3 is already focused after insert. Scroll brought it into view.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=10.00)
        Window(id=WindowId(3), x=0.00, y=10.00, w=150.00, h=20.00, highlighted)
      )

    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
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
    *                                                                         W3                                                                         *
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
        .insert_window(titled("w4"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w5"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w6"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w7"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w8"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("w9"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    hub.set_window_constraint(w5, None, Some(20.0), None, None);
    // Focus last stack window (w5 is already focused after insert)
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(5), x=75.00, y=10.00, w=75.00, h=20.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W5                                   *
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
        .insert_window(titled("w10"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w11"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w12"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w13"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w14"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("w15"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w6 = hub
        .insert_window(titled("w16"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w7 = hub
        .insert_window(titled("w17"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    hub.set_window_constraint(w5, None, Some(20.0), None, None);
    hub.set_window_constraint(w6, None, Some(20.0), None, None);
    hub.set_window_constraint(w7, None, Some(20.0), None, None);

    hub.set_focus(w3);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(3), x=0.00, y=10.00, w=75.00, h=20.00, highlighted)
        Window(id=WindowId(6), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(7), x=75.00, y=10.00, w=75.00, h=20.00)
      )

    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W2                                   ||                                    W6                                   |
    |                                                                         ||                                                                         |
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
    *                                    W3                                   *|                                    W7                                   |
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
        .insert_window(titled("w18"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w19"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00)
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=30.00, highlighted)
      )

    +--------------------------------------------------------------------------------------------------+**************************************************
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                W0                                                |*                       W1                        
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    |                                                                                                  |*                                                 
    +--------------------------------------------------------------------------------------------------+**************************************************
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
        .insert_window(titled("w20"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w21"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, Some(200.0), None, None, None);
    hub.set_window_constraint(w1, Some(50.0), None, None, None);
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
        .insert_window(titled("w22"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w23"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, Some(120.0), None, None, None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=120.00, h=30.00)
        Window(id=WindowId(1), x=120.00, y=0.00, w=30.00, h=30.00, highlighted)
      )

    +----------------------------------------------------------------------------------------------------------------------+******************************
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                          W0                                                          |*             W1             *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    |                                                                                                                      |*                            *
    +----------------------------------------------------------------------------------------------------------------------+******************************
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
        .insert_window(titled("w24"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w25"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, None, None, None, Some(10.0));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=10.00, w=75.00, h=10.00, highlighted)
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
    |                                                                         |                                                                           
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
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
        .insert_window(titled("w26"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w27"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, None, None, Some(30.0), None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=98.00, y=0.00, w=30.00, h=30.00, highlighted)
      )

    +-------------------------------------------------------------------------+                       ******************************                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                    W0                                   |                       *             W1             *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    |                                                                         |                       *                            *                      
    +-------------------------------------------------------------------------+                       ******************************
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
        .insert_window(titled("w28"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("w29"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, None, None, Some(40.0), None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=18.00, y=0.00, w=40.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted)
      )

                      +--------------------------------------+                 ***************************************************************************
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                  W0                  |                 *                                    W1                                   *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      |                                      |                 *                                                                         *
                      +--------------------------------------+                 ***************************************************************************
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
        .insert_window(titled("w30"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w31"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w32"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w33"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w34"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    // w4 is the last stack window, already focused after insert. Stack scrolled
    // MoreMaster: first stack window becomes second master window
    hub.handle_tiling_action(TilingAction::MoreMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=0.00, y=10.00, w=75.00, h=20.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=10.00, w=75.00, h=20.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W3                                   |
    |                                                                         ||                                                                         |
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
    |                                    W1                                   |*                                    W4                                   *
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
        .insert_window(titled("w35"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w36"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w37"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w38"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("w39"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    // focus_left lands on w3, the last-inserted master window, scrolling master to the bottom
    // so the offset is out of range once master shrinks.
    hub.focus_left();
    // FewerMaster: last master becomes first stack window
    hub.handle_tiling_action(TilingAction::FewerMaster);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=75.00, h=20.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=20.00, highlighted)
        Window(id=WindowId(4), x=75.00, y=20.00, w=75.00, h=10.00)
      )

    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+*                                                                         *
    +-------------------------------------------------------------------------+*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |***************************************************************************
    |                                    W2                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
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
        .insert_window(titled("w40"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w41"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w42"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w43"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    // w3 is already focused (last master) and scroll brought it into view
    // Detach last master window
    hub.delete_window(w3);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=150.00, h=20.00, highlighted)
      )

    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
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
    *                                                                         W2                                                                         *
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
        .insert_window(titled("w44"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w45"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w46"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w47"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w48"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    // w4 is already focused (last stack window). Stack scrolled to show it.
    // Attach a new window (lands in stack since master_count=1 is full)
    let w5 = hub
        .insert_window(titled("w49"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w5, None, Some(20.0), None, None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(5), x=75.00, y=10.00, w=75.00, h=20.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W4                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
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
        .insert_window(titled("w50"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w51"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("w52"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("w53"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("w54"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    // w4 is already focused (last stack window). Stack scrolled to show it.
    // Apply same config (relayout, clamp is idempotent)
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
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(4), x=75.00, y=10.00, w=75.00, h=20.00, highlighted)
      )

    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W3                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                                                         *
    |                                                                         |*                                                                         *
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
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}
