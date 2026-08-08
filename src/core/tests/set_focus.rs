use crate::core::GlobalLayoutConfig;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, default_rect, setup, setup_with_layout, snapshot, titled, titled_matcher,
};
use insta::assert_snapshot;

/// Float and fullscreen matchers by exact title, since this file also inserts
/// tiling windows named `wN`. Two lists because one test needs both modes on one hub.
fn layout_modes(floats: &[&str], fullscreens: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(floats.iter().map(|t| titled_matcher(t)).collect())
        .with_fullscreen(fullscreens.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn set_focus_same_workspace_tiling_and_float() {
    let mut hub = setup_with_layout(layout_modes(&["w2"], &[]));

    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();

    // Tiling: focus w0, then w1
    hub.set_focus(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
      )

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
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W0                                   *|                                    W1                                   |
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
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");

    hub.set_focus(w1);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
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

    // Float: insert float, focus tiling then float
    hub.insert_window(
        titled("w2"),
        PixelRect::new(10, 5, 30, 10),
        WindowRestrictions::None,
    )
    .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(2), x=10.00, y=5.00, w=30.00, h=10.00, float, highlighted)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |         ******************************                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *             F2             *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         *                            *                                  ||                                                                         |
    |         ******************************                                  ||                                                                         |
    |                                    W0                                   ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");
}

#[test]
fn set_focus_switches_workspace() {
    // Tiling: switch workspace via set_focus
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w3"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_workspace("1", None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.set_focus(w0);
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

    // Float: switch workspace via set_focus
    let mut hub = setup_with_layout(layout_modes(&["w5"], &[]));
    let f0 = hub
        .insert_window(
            titled("w5"),
            PixelRect::new(10, 5, 30, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.focus_workspace("1", None);
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.set_focus(f0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=10.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn set_focus_in_other_workspace_keeps_origin_workspace() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w7"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.move_focused_to_workspace("2", None);
    hub.set_focus(w0);

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
    assert_eq!(hub.query_workspaces().len(), 2);
}

#[test]
fn float_focus_changes_float_z_order() {
    let mut hub = setup_with_layout(layout_modes(&["w8", "w9", "w10"], &[]));
    let w0 = hub
        .insert_window(
            titled("w8"),
            PixelRect::new(10, 5, 30, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    let w1 = hub
        .insert_window(
            titled("w9"),
            PixelRect::new(50, 5, 30, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    let _w2 = hub
        .insert_window(
            titled("w10"),
            PixelRect::new(100, 5, 30, 10),
            WindowRestrictions::None,
        )
        .unwrap();

    hub.set_focus(w0);
    hub.set_focus(w1);
    // Now z-order from top to bottom should be [w1, w0, w2]
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=5.00, w=30.00, h=10.00, float)
        Window(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                      ******************************                    +----------------------------+                    
                                                      *                            *                    |                            |                    
                                                      *                            *                    |                            |                    
                                                      *                            *                    |                            |                    
                                                      *                            *                    |                            |                    
                                                      *             F1             *                    |             F2             |                    
                                                      *                            *                    |                            |                    
                                                      *                            *                    |                            |                    
                                                      *                            *                    |                            |                    
                                                      ******************************                    +----------------------------+
    ");
}

#[test]
fn detach_topmost_fullscreen_focuses_next_fullscreen() {
    let mut hub = setup_with_layout(layout_modes(&[], &["w12", "w13"]));
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w12"), default_rect(), WindowRestrictions::None);
    let fs2 = hub
        .insert_window(titled("w13"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.delete_window(fs2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(1))
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
    |                                                                         W1                                                                         |
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
fn detach_only_fullscreen_focuses_tiling_even_in_presence_of_float() {
    let mut hub = setup_with_layout(layout_modes(&["w15"], &["w16"]));
    hub.insert_window(titled("w14"), default_rect(), WindowRestrictions::None);
    hub.insert_window(
        titled("w15"),
        PixelRect::new(50, 5, 30, 10),
        WindowRestrictions::None,
    )
    .unwrap();
    let fs = hub
        .insert_window(titled("w16"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.delete_window(fs);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00, float)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                 +----------------------------+                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |             F1             |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 |                            |                                                                     *
    *                                                 +----------------------------+                                                                     *
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
fn detach_last_tiling_with_floats_focuses_float() {
    let mut hub = setup_with_layout(layout_modes(&["w18"], &[]));
    let t = hub
        .insert_window(titled("w17"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(
        titled("w18"),
        PixelRect::new(10, 5, 30, 10),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.set_focus(t);
    hub.delete_window(t);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=10.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F1             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn detach_non_topmost_keeps_focus() {
    // Float: delete non-topmost, topmost stays focused
    let mut hub = setup_with_layout(layout_modes(&["w19", "w20"], &[]));
    let a = hub
        .insert_window(
            titled("w19"),
            PixelRect::new(10, 5, 30, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.insert_window(
        titled("w20"),
        PixelRect::new(50, 5, 30, 10),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.delete_window(a);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                      ******************************                                                                      
                                                      *                            *                                                                      
                                                      *                            *                                                                      
                                                      *                            *                                                                      
                                                      *                            *                                                                      
                                                      *             F1             *                                                                      
                                                      *                            *                                                                      
                                                      *                            *                                                                      
                                                      *                            *                                                                      
                                                      ******************************
    ");

    // Fullscreen: delete non-topmost, topmost stays focused
    let mut hub = setup_with_layout(layout_modes(&[], &["w21", "w22"]));
    let fs1 = hub
        .insert_window(titled("w21"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w22"), default_rect(), WindowRestrictions::None);
    hub.delete_window(fs1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(1))
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
    |                                                                         W1                                                                         |
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
