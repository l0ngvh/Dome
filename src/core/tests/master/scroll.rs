use crate::config::{LayoutConfig, MasterConfig, Strategy};
use crate::core::hub::{Hub, HubConfig};
use crate::core::node::{Dimension, Length};
use crate::core::strategy::TilingAction;
use crate::core::tests::default_partition_tree_config_for_tests;
use crate::core::tests::snapshot;
use insta::assert_snapshot;

fn setup_master() -> Hub {
    let mut config = HubConfig::default();
    config.layout.strategy = Strategy::Master;
    Hub::new(
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
        config,
    )
}

#[test]
fn min_height_master_pane_overflows_and_scrolls_to_focus() {
    let mut hub = setup_master();
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 4,
            },
        },
        ..Default::default()
    });
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
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
    let mut hub = setup_master();
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 2,
            },
        },
        ..Default::default()
    });
    let _w0 = hub.insert_tiling();
    let _w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let w4 = hub.insert_tiling();
    let w5 = hub.insert_tiling();
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
    let mut hub = setup_master();
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 4,
            },
        },
        ..Default::default()
    });
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let w4 = hub.insert_tiling();
    let w5 = hub.insert_tiling();
    let w6 = hub.insert_tiling();
    let w7 = hub.insert_tiling();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    hub.set_window_constraint(w5, None, Some(20.0), None, None);
    hub.set_window_constraint(w6, None, Some(20.0), None, None);
    hub.set_window_constraint(w7, None, Some(20.0), None, None);
    // w7 is focused after insert, stack already scrolled to show it.
    // Focus master and scroll it to the bottom.
    hub.focus_left();
    hub.focus_down();
    hub.focus_down();
    hub.focus_down();
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
    let mut hub = setup_master();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w1, Some(100.0), None, None, None);
    assert_snapshot!(snapshot(&hub), @"
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
    let mut hub = setup_master();
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w0, Some(200.0), None, None, None);
    hub.set_window_constraint(w1, Some(50.0), None, None, None);
    assert_snapshot!(snapshot(&hub), @"
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
    let mut hub = setup_master();
    let w0 = hub.insert_tiling();
    let _w1 = hub.insert_tiling();
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
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, None, None, None, Some(10.0));
    assert_snapshot!(snapshot(&hub), @"
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
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    hub.set_window_constraint(w1, None, None, Some(30.0), None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(1), x=97.50, y=0.00, w=30.00, h=30.00, highlighted)
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
    let mut hub = setup_master();
    let w0 = hub.insert_tiling();
    let _w1 = hub.insert_tiling();
    hub.set_window_constraint(w0, None, None, Some(40.0), None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=17.50, y=0.00, w=40.00, h=30.00)
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
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let w4 = hub.insert_tiling();
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
    let mut hub = setup_master();
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 4,
            },
        },
        ..Default::default()
    });
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let _w4 = hub.insert_tiling();
    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    // Navigate to last master window to scroll master pane to bottom
    hub.focus_left();
    hub.focus_down();
    hub.focus_down();
    hub.focus_down();
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
    let mut hub = setup_master();
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 4,
            },
        },
        ..Default::default()
    });
    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
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
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let w4 = hub.insert_tiling();
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    // w4 is already focused (last stack window). Stack scrolled to show it.
    // Attach a new window (lands in stack since master_count=1 is full)
    let w5 = hub.insert_tiling();
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
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();
    let w2 = hub.insert_tiling();
    let w3 = hub.insert_tiling();
    let w4 = hub.insert_tiling();
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_window_constraint(w2, None, Some(20.0), None, None);
    hub.set_window_constraint(w3, None, Some(20.0), None, None);
    hub.set_window_constraint(w4, None, Some(20.0), None, None);
    // w4 is already focused (last stack window). Stack scrolled to show it.
    // Apply same config (relayout, clamp is idempotent)
    hub.sync_config(HubConfig {
        layout: LayoutConfig {
            strategy: Strategy::Master,
            partition_tree: default_partition_tree_config_for_tests(),
            master: MasterConfig {
                master_ratio: 0.5,
                master_count: 1,
            },
        },
        ..Default::default()
    });
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

#[test]
fn single_window_no_scroll_state() {
    let mut hub = setup_master();
    let _w0 = hub.insert_tiling();
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
