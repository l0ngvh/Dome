use insta::assert_snapshot;

use crate::{
    config::SizeConstraint,
    core::{
        Length,
        tests::{LayoutConfigBuilder, setup, snapshot, titled},
    },
};

#[test]
fn scroll_vertically_to_focus() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));

    hub.set_window_constraint(w0, None, Some(20.0), None, None);
    hub.set_window_constraint(w1, None, Some(20.0), None, None);
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=20.00, w=150.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=20.00, highlighted, spawn=bottom)
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
    *                                                                         W0                                                                         *
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
    |                                                                                                                                                    |
    |                                                                         W1                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    ");
}

#[test]
fn scroll_horizontally_to_focus() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.insert_tiling(hub.current_workspace(), titled("w3"));
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w5"));

    hub.set_window_constraint(w0, Some(50.0), None, None, None);
    hub.set_window_constraint(w2, Some(90.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=140.00, y=15.00, w=10.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=15.00, w=90.00, h=15.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=100.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w2, Container])
        Container(id=ContainerId(1), x=50.00, y=0.00, w=100.00, h=30.00, titles=[w3, Container])
        Container(id=ContainerId(2), x=50.00, y=15.00, w=100.00, h=15.00, titles=[w4, w5])
      )

    **************************************************+---------------------------------------------------------------------------------------------------
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                W1                                                 
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *|                                                                                                   
    *                                                *+---------------------------------------------------------------------------------------------------
    *                       W0                       *+----------------------------------------------------------------------------------------++---------
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                           W2                                           ||   W3    
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    *                                                *|                                                                                        ||         
    **************************************************+----------------------------------------------------------------------------------------++---------
    ");
}

#[test]
fn scroll_container_into_focus() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.insert_tiling(hub.current_workspace(), titled("w7"));
    hub.toggle_spawn_mode(); // vertical
    hub.insert_tiling(hub.current_workspace(), titled("w8"));
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w9"));

    hub.set_window_constraint(w0, Some(100.0), None, None, None);
    hub.set_window_constraint(w3, Some(100.0), None, None, None);

    hub.focus_parent();
    hub.focus_parent();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=20.00, w=50.00, h=10.00)
        Window(id=WindowId(2), x=100.00, y=10.00, w=50.00, h=10.00)
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right, titles=[w6, Container])
        Container(id=ContainerId(1), x=100.00, y=0.00, w=50.00, h=30.00, titles=[w7, w8, w9])
      )

    ******************************************************************************************************************************************************
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                       W1                        
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                W0                                                ||                       W2                        
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  |+-------------------------------------------------
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                       W3                        
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    *                                                                                                  ||                                                 
    ******************************************************************************************************************************************************
    ");
}

#[test]
fn scroll_window_into_view_in_vertical_child_container() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w10"));
    hub.toggle_spawn_mode();
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w11"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w12"));

    hub.set_window_constraint(w0, Some(100.0), Some(20.0), None, None);
    hub.set_window_constraint(w1, Some(100.0), Some(20.0), None, None);
    hub.set_window_constraint(w2, Some(100.0), Some(20.0), None, None);
    hub.focus_parent();
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w13"));
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=100.00, h=20.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=100.00, h=10.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w13])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00, titles=[w10, w11, w12])
      )

    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                W1                                                ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                       W3                       |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                W2                                                *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    ****************************************************************************************************+------------------------------------------------+
    ");

    hub.delete_window(w0);

    // After deleting w0, w1 expands to full screen width
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=100.00, h=20.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=100.00, h=10.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, w13])
        Container(id=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00, titles=[w11, w12])
      )

    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                W1                                                ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    |                                                                                                  ||                                                |
    +--------------------------------------------------------------------------------------------------+|                                                |
    ****************************************************************************************************|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                       W3                       |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                W2                                                *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    *                                                                                                  *|                                                |
    ****************************************************************************************************+------------------------------------------------+
    ");
}

#[test]
fn scroll_view_port_also_scroll_max_constrained_window() {
    let mut hub = setup();

    let l = LayoutConfigBuilder::new()
        .with_max_height(SizeConstraint::Pixels(Length::new(10.0)))
        .with_min_height(SizeConstraint::Pixels(Length::new(7.0)))
        .build();
    hub.sync_configuration(l);

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w14"));
    hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.insert_tiling(hub.current_workspace(), titled("w18"));
    hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.insert_tiling(hub.current_workspace(), titled("w20"));
    hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.set_focus(w0);
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w22"));

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
        .with_max_height(SizeConstraint::Pixels(Length::new(30.0)))
        .with_min_height(SizeConstraint::Pixels(Length::new(7.0)))
        .with_min_width(SizeConstraint::Pixels(Length::new(30.0)))
        .build();
    hub.sync_configuration(l);

    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w23"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w24"));
    hub.set_window_constraint(w1, None, None, Some(120.0), None);
    hub.toggle_spawn_mode();
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("w25"));
    hub.set_window_constraint(w2, None, Some(25.), None, None);
    hub.toggle_spawn_mode();
    let w3 = hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.set_window_constraint(w3, Some(50.), None, None, None);
    hub.insert_tiling(hub.current_workspace(), titled("w27"));
    hub.toggle_spawn_mode();
    let w5 = hub.insert_tiling(hub.current_workspace(), titled("w28"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w29"));
    hub.insert_tiling(hub.current_workspace(), titled("w30"));
    hub.insert_tiling(hub.current_workspace(), titled("w31"));
    hub.insert_tiling(hub.current_workspace(), titled("w32"));
    hub.insert_tiling(hub.current_workspace(), titled("w33"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(10))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(10), x=120.00, y=17.50, w=30.00, h=12.50, highlighted, spawn=right)
        Window(id=WindowId(9), x=90.00, y=17.50, w=30.00, h=12.50)
        Window(id=WindowId(8), x=60.00, y=17.50, w=30.00, h=12.50)
        Window(id=WindowId(7), x=30.00, y=17.50, w=30.00, h=12.50)
        Window(id=WindowId(6), x=0.00, y=17.50, w=30.00, h=12.50)
        Window(id=WindowId(4), x=0.00, y=5.00, w=150.00, h=12.50)
        Window(id=WindowId(1), x=15.00, y=0.00, w=120.00, h=5.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, Container])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(2), x=0.00, y=5.00, w=150.00, h=25.00, titles=[w25, w26, Container])
        Container(id=ContainerId(3), x=0.00, y=5.00, w=150.00, h=25.00, titles=[w27, Container])
        Container(id=ContainerId(4), x=0.00, y=17.50, w=150.00, h=12.50, titles=[w28, w29, w30, w31, w32, w33])
      )

                   |                                                                                                                      |               
                   |                                                                                                                      |               
                   |                                                                                                                      |               
                   |                                                          W1                                                          |               
                   +----------------------------------------------------------------------------------------------------------------------+               
    -----------------------------------------------------------------------------------------------------------------------------------------------------+
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

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(6), x=140.00, y=17.50, w=10.00, h=12.50)
        Window(id=WindowId(5), x=110.00, y=17.50, w=30.00, h=12.50, highlighted, spawn=right)
        Window(id=WindowId(4), x=110.00, y=5.00, w=40.00, h=12.50)
        Window(id=WindowId(3), x=60.00, y=5.00, w=50.00, h=25.00)
        Window(id=WindowId(2), x=30.00, y=5.00, w=30.00, h=25.00)
        Window(id=WindowId(1), x=30.00, y=0.00, w=120.00, h=5.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w23, Container])
        Container(id=ContainerId(1), x=30.00, y=0.00, w=120.00, h=30.00, titles=[w24, Container])
        Container(id=ContainerId(2), x=30.00, y=5.00, w=120.00, h=25.00, titles=[w25, w26, Container])
        Container(id=ContainerId(3), x=110.00, y=5.00, w=40.00, h=25.00, titles=[w27, Container])
        Container(id=ContainerId(4), x=110.00, y=17.50, w=40.00, h=12.50, titles=[w28, w29, w30, w31, w32, w33])
      )

    +----------------------------+|                                                                                                                      |
    |                            ||                                                                                                                      |
    |                            ||                                                                                                                      |
    |                            ||                                                          W1                                                          |
    |                            |+----------------------------------------------------------------------------------------------------------------------+
    |                            |+----------------------------++------------------------------------------------++---------------------------------------
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                  W4                   
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |             W0             ||                            ||                                                ||                                       
    |                            ||                            ||                                                ||                                       
    |                            ||                            ||                                                |+---------------------------------------
    |                            ||             W2             ||                       W3                       |******************************+---------
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*             W5             *|   W6    
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    |                            ||                            ||                                                |*                            *|         
    +----------------------------++----------------------------++------------------------------------------------+******************************+---------
    ");
}
