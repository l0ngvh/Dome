use crate::core::GlobalLayoutConfig;
use crate::core::allocator::NodeId;
use crate::core::node::{Dimension, Length, MonitorId, WindowId, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, default_dim, setup, setup_with_layout, snapshot, titled, titled_matcher,
};
use insta::assert_snapshot;

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn insert_float_window() {
    let mut hub = setup_with_layout(layout_floating(&["w0"]));
    hub.insert_window(
        titled("w0"),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ******************************                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *             F0             *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              *                            *                                                                                                              
              ******************************
    ");
}

#[test]
fn float_window_with_tiling() {
    let mut hub = setup_with_layout(layout_floating(&["w2"]));
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.insert_window(
        titled("w2"),
        Dimension::new(
            Length::new(50.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(15.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=5.00, w=40.00, h=15.00, float, highlighted)
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                 ****************************************                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                  F1                  *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 *                                      *                                                           |
    |                                                 ****************************************                                                           |
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
fn move_float_to_workspace() {
    let mut hub = setup_with_layout(layout_floating(&["w4"]));
    hub.insert_window(titled("w3"), default_dim(), WindowRestrictions::None);
    hub.insert_window(
        titled("w4"),
        Dimension::new(
            Length::new(50.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(15.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.move_focused_to_workspace("1");
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
fn focus_falls_back_to_tiling_after_float_delete() {
    let mut hub = setup_with_layout(layout_floating(&["w6"]));
    hub.insert_window(titled("w5"), default_dim(), WindowRestrictions::None);
    let f0 = hub
        .insert_window(
            titled("w6"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(40.0),
                Length::new(15.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    // Float is focused after insert
    hub.delete_window(f0);
    // Focus should fall back to tiling window
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
fn focus_falls_back_to_last_float() {
    let mut hub = setup_with_layout(layout_floating(&["w7", "w8"]));
    hub.insert_window(
        titled("w7"),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(10.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    let f1 = hub
        .insert_window(
            titled("w8"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(30.0),
                Length::new(10.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    // f1 is focused
    hub.delete_window(f1);
    // Focus should fall back to f0
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
fn toggle_tiling_to_float() {
    let mut hub = setup();
    hub.insert_window(titled("w9"), default_dim(), WindowRestrictions::None);
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, float, highlighted)
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
    *                                                                         F0                                                                         *
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
fn toggle_float_to_tiling() {
    let mut hub = setup_with_layout(layout_floating(&["w10"]));
    hub.insert_window(
        titled("w10"),
        Dimension::new(
            Length::new(50.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(15.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.toggle_float();
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
fn toggle_tiling_to_float_scenarios() {
    let mut hub = setup();
    hub.insert_window(titled("w11"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("w12"), default_dim(), WindowRestrictions::None);

    // Toggle W1 to float (covers toggle with existing tiling + position preservation at x=75)
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, float, highlighted)
      )

    +--------------------------------------------------------------------------***************************************************************************
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                         W*                                    F1                                   *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    +--------------------------------------------------------------------------***************************************************************************
    ");

    // Toggle W1 back to tiling
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w11, w12])
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
fn workspace_with_only_floats_not_deleted_prematurely() {
    // Regression test: workspace should not be deleted if it still has floats
    let mut hub = setup_with_layout(layout_floating(&["w14"]));

    hub.insert_window(titled("w13"), default_dim(), WindowRestrictions::None);

    hub.focus_workspace("1");
    let f1 = hub
        .insert_window(
            titled("w14"),
            Dimension::new(
                Length::new(10.0),
                Length::new(5.0),
                Length::new(30.0),
                Length::new(20.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    let w2 = hub
        .insert_window(titled("w15"), default_dim(), WindowRestrictions::None)
        .unwrap();

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=20.00, float)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *         +----------------------------+                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |             F1             |                                  W2                                                                         *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         |                            |                                                                                                             *
    *         +----------------------------+                                                                                                             *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");

    hub.focus_workspace("0");

    hub.delete_window(w2);

    let after_tiling_delete = snapshot(&hub);
    assert_snapshot!(after_tiling_delete, @"
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

    // Now delete the float - this should not panic
    hub.delete_window(f1);

    assert_eq!(snapshot(&hub), after_tiling_delete);
}

#[test]
fn delete_unfocused_float_window() {
    use crate::core::node::{Dimension, Length};
    let mut hub = setup_with_layout(layout_floating(&["w16"]));

    let f0 = hub
        .insert_window(
            titled("w16"),
            Dimension::new(
                Length::new(10.0),
                Length::new(5.0),
                Length::new(30.0),
                Length::new(20.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.insert_window(titled("w17"), default_dim(), WindowRestrictions::None);

    hub.delete_window(f0);

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
fn delete_float_keeps_workspace_alive() {
    // Scenario 1: delete float on current workspace -- workspace kept
    let canonical = {
        let mut hub = setup_with_layout(layout_floating(&["w18"]));
        let f0 = hub
            .insert_window(
                titled("w18"),
                Dimension::new(
                    Length::new(10.0),
                    Length::new(5.0),
                    Length::new(30.0),
                    Length::new(20.0),
                ),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.delete_window(f0);
        let snap = snapshot(&hub);
        assert_snapshot!(snap, @"
        Hub(focused=None)
          Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
        ");
        snap
    };

    // Scenario 2: non-current workspace kept because tiling exists
    {
        let mut hub = setup_with_layout(layout_floating(&["w20"]));
        hub.focus_workspace("1");
        hub.insert_window(titled("w19"), default_dim(), WindowRestrictions::None);
        let f0 = hub
            .insert_window(
                titled("w20"),
                Dimension::new(
                    Length::new(10.0),
                    Length::new(5.0),
                    Length::new(30.0),
                    Length::new(20.0),
                ),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_eq!(snapshot(&hub), canonical);
        assert_eq!(
            hub.query_workspaces().len(),
            2,
            "ws1 should still exist (has tiling window)"
        );
    }

    // Scenario 3: non-current workspace kept because other float exists
    {
        let mut hub = setup_with_layout(layout_floating(&["w21", "w22"]));
        hub.focus_workspace("1");
        let f0 = hub
            .insert_window(
                titled("w21"),
                Dimension::new(
                    Length::new(10.0),
                    Length::new(5.0),
                    Length::new(30.0),
                    Length::new(20.0),
                ),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.insert_window(
            titled("w22"),
            Dimension::new(
                Length::new(50.0),
                Length::new(5.0),
                Length::new(30.0),
                Length::new(20.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_eq!(snapshot(&hub), canonical);
        assert_eq!(
            hub.query_workspaces().len(),
            2,
            "ws1 should still exist (has another float)"
        );
    }

    {
        let mut hub = setup_with_layout(layout_floating(&["w23"]));
        hub.focus_workspace("1");
        let f0 = hub
            .insert_window(
                titled("w23"),
                Dimension::new(
                    Length::new(10.0),
                    Length::new(5.0),
                    Length::new(30.0),
                    Length::new(20.0),
                ),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_eq!(snapshot(&hub), canonical);
        assert_eq!(
            hub.query_workspaces().len(),
            2,
            "ws1 should still exist (pruning disabled)"
        );
    }
}

#[test]
fn insert_float_offscreen_does_not_scroll_viewport() {
    let mut hub = setup_with_layout(layout_floating(&["w25"]));
    let _w0 = hub
        .insert_window(titled("w24"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(
        titled("w25"),
        Dimension::new(
            Length::new(200.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();

    let _ws_id = hub.current_workspace();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
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
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn update_float_dimension_writes_new_dim() {
    let mut hub = setup_with_layout(layout_floating(&["w26"]));
    hub.insert_window(
        titled("w26"),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.update_float_dimension(
        WindowId::new(0),
        Dimension::new(
            Length::new(50.0),
            Length::new(20.0),
            Length::new(60.0),
            Length::new(40.0),
        ),
        MonitorId::new(0),
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=49.00, y=19.00, w=62.00, h=11.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                     **************************************************************                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                             F0                             *                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                                                            *                                       
                                                     *                                                            *
    ");
}

#[test]
fn update_float_dimension_preserves_z_order() {
    let mut hub = setup_with_layout(layout_floating(&["w27", "w28", "w29"]));
    let a = hub
        .insert_window(
            titled("w27"),
            Dimension::new(
                Length::new(10.0),
                Length::new(5.0),
                Length::new(30.0),
                Length::new(20.0),
            ),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.insert_window(
        titled("w28"),
        Dimension::new(
            Length::new(50.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.insert_window(
        titled("w29"),
        Dimension::new(
            Length::new(90.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    // Move a (index 0) without changing z-order or focus (c stays topmost/focused)
    hub.update_float_dimension(
        a,
        Dimension::new(
            Length::new(15.0),
            Length::new(10.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        MonitorId::new(0),
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=14.00, y=9.00, w=32.00, h=21.00, float)
        Window(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=20.00, float)
        Window(id=WindowId(2), x=90.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                      +----------------------------+          ******************************                              
                                                      |                            |          *                            *                              
                                                      |                            |          *                            *                              
                                                      |                            |          *                            *                              
                  +------------------------------+    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |             F1             |          *             F2             *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |              F0              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    |                            |          *                            *                              
                  |                              |    +----------------------------+          ******************************                              
                  |                              |                                                                                                        
                  |                              |                                                                                                        
                  |                              |                                                                                                        
                  |                              |                                                                                                        
                  |                              |
    ");
}

#[test]
#[should_panic(expected = "is not Float")]
fn update_float_dimension_on_tiling_panics() {
    let mut hub = setup();
    let w = hub
        .insert_window(titled("w30"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.update_float_dimension(
        w,
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        MonitorId::new(0),
    );
}

#[test]
#[should_panic]
fn update_float_dimension_on_unknown_panics() {
    let mut hub = setup_with_layout(layout_floating(&["w31"]));
    hub.insert_window(
        titled("w31"),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        WindowRestrictions::None,
    )
    .unwrap();
    // WindowId(999) was never inserted -- panics in allocator.get()
    hub.update_float_dimension(
        WindowId::new(999),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(30.0),
            Length::new(20.0),
        ),
        MonitorId::new(0),
    );
}
