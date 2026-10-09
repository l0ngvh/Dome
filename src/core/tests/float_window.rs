use crate::core::MonitorSelector;
use crate::core::Strategy;
use crate::core::allocator::NodeId;
use crate::core::hub::MonitorLayout;
use crate::core::node::{MonitorId, PixelRect, Pixels, WindowId, WindowRestrictions};
use crate::core::tests::{
    TilingConfigBuilder, default_rect, reported_monitor, setup, setup_modes_on, setup_with_modes,
    snapshot, snapshot_text, titled, validate_hub,
};
use insta::assert_snapshot;

#[test]
fn insert_float_window() {
    let mut hub = setup_with_modes("0", &["w0"], &[]);
    hub.insert_window(
        titled("w0"),
        PixelRect::new(10, 5, 30, 20),
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
    let mut hub = setup_with_modes("0", &["w2"], &[]);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(
        titled("w2"),
        PixelRect::new(50, 5, 40, 15),
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
    let mut hub = setup_with_modes("0", &["w4"], &[]);
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(
        titled("w4"),
        PixelRect::new(50, 5, 40, 15),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.move_focused_to_workspace("1", None);
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
    let mut hub = setup_with_modes("0", &["w6"], &[]);
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    let f0 = hub
        .insert_window(
            titled("w6"),
            PixelRect::new(50, 5, 40, 15),
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
    let mut hub = setup_with_modes("0", &["w7", "w8"], &[]);
    hub.insert_window(
        titled("w7"),
        PixelRect::new(10, 5, 30, 10),
        WindowRestrictions::None,
    )
    .unwrap();
    let f1 = hub
        .insert_window(
            titled("w8"),
            PixelRect::new(50, 5, 30, 10),
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
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
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
    let mut hub = setup_with_modes("0", &["w10"], &[]);
    hub.insert_window(
        titled("w10"),
        PixelRect::new(50, 5, 40, 15),
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
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w12"), default_rect(), WindowRestrictions::None);

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
    let mut hub = setup_with_modes("1", &["w14"], &[]);

    hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);

    hub.focus_workspace("1", None);
    let f1 = hub
        .insert_window(
            titled("w14"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .unwrap();
    let w2 = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
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

    hub.focus_workspace("0", None);

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
    let mut hub = setup_with_modes("0", &["w16"], &[]);

    let f0 = hub
        .insert_window(
            titled("w16"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);

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
        let mut hub = setup_with_modes("0", &["w18"], &[]);
        let f0 = hub
            .insert_window(
                titled("w18"),
                PixelRect::new(10, 5, 30, 20),
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
        let mut hub = setup_with_modes("1", &["w20"], &[]);
        hub.focus_workspace("1", None);
        hub.insert_window(titled("w19"), default_rect(), WindowRestrictions::None);
        let f0 = hub
            .insert_window(
                titled("w20"),
                PixelRect::new(10, 5, 30, 20),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.focus_workspace("0", None);
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
        let mut hub = setup_with_modes("1", &["w21", "w22"], &[]);
        hub.focus_workspace("1", None);
        let f0 = hub
            .insert_window(
                titled("w21"),
                PixelRect::new(10, 5, 30, 20),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.insert_window(
            titled("w22"),
            PixelRect::new(50, 5, 30, 20),
            WindowRestrictions::None,
        )
        .unwrap();
        hub.focus_workspace("0", None);
        hub.delete_window(f0);
        assert_eq!(snapshot(&hub), canonical);
        assert_eq!(
            hub.query_workspaces().len(),
            2,
            "ws1 should still exist (has another float)"
        );
    }

    {
        let mut hub = setup_with_modes("1", &["w23"], &[]);
        hub.focus_workspace("1", None);
        let f0 = hub
            .insert_window(
                titled("w23"),
                PixelRect::new(10, 5, 30, 20),
                WindowRestrictions::None,
            )
            .unwrap();
        hub.focus_workspace("0", None);
        hub.delete_window(f0);
        assert_eq!(snapshot(&hub), canonical);
        assert_eq!(
            hub.access.workspaces.sorted_ids().len(),
            2,
            "ws1 should still exist (pruning disabled)"
        );
    }
}

#[test]
fn update_float_rect_writes_new_dim() {
    let mut hub = setup_with_modes("0", &["w26"], &[]);
    hub.insert_window(
        titled("w26"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.update_float_rect(
        WindowId::new(0),
        PixelRect::new(50, 20, 60, 40),
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
fn update_float_rect_preserves_z_order() {
    let mut hub = setup_with_modes("0", &["w27", "w28", "w29"], &[]);
    let a = hub
        .insert_window(
            titled("w27"),
            PixelRect::new(10, 5, 30, 20),
            WindowRestrictions::None,
        )
        .unwrap();
    hub.insert_window(
        titled("w28"),
        PixelRect::new(50, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.insert_window(
        titled("w29"),
        PixelRect::new(90, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    // Move a (index 0) without changing z-order or focus (c stays topmost/focused)
    hub.update_float_rect(a, PixelRect::new(15, 10, 30, 20), MonitorId::new(0));
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
fn dragged_float_moves_to_the_active_workspace_of_the_monitor_it_lands_on() {
    insta::allow_duplicates! {
        for strategy in [Strategy::PartitionTree, Strategy::Master] {
            let mut hub = setup_modes_on(strategy, "0", &["chat"], &[]);
            hub.add_monitor(reported_monitor(
                "external".to_string(),
                PixelRect::new(150, 0, 100, 30),
                1.0,
            ));
            let w0 = hub
                .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
                .unwrap();
            let chat = hub
                .insert_window(
                    titled("chat"),
                    PixelRect::new(10, 5, 30, 20),
                    WindowRestrictions::None,
                )
                .unwrap();
            hub.focus_monitor(&MonitorSelector::Name("external".to_string()));
            hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
            hub.focus_monitor(&MonitorSelector::Name("primary".to_string()));

            hub.update_float_rect(chat, PixelRect::new(160, 5, 30, 20), MonitorId::new(1));
            validate_hub(&hub);
            assert_eq!(
                hub.get_visible_placements().focused_window,
                Some(w0),
                "{strategy:?}: the drag leaves keyboard focus on the primary monitor"
            );

            hub.focus_monitor(&MonitorSelector::Name("external".to_string()));
            assert_snapshot!(snapshot_text(&hub), @r#"
            Hub(focused=WindowId(1))
              Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
                Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
              )
              Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
                Window(id=WindowId(2), x=150.00, y=0.00, w=100.00, h=30.00)
                Window(id=WindowId(1), x=159.00, y=4.00, w=32.00, h=22.00, float, highlighted)
              )
            "#);
        }
    }
}

#[test]
fn update_float_rect_ignores_a_window_that_no_longer_floats() {
    for (strategy, app_fullscreen) in [
        (Strategy::PartitionTree, false),
        (Strategy::PartitionTree, true),
        (Strategy::Master, false),
        (Strategy::Master, true),
        (Strategy::Scrolling, false),
        (Strategy::Scrolling, true),
    ] {
        let mut hub = setup_modes_on(strategy, "0", &["chat"], &[]);
        hub.add_monitor(reported_monitor(
            "external".to_string(),
            PixelRect::new(150, 0, 100, 30),
            1.0,
        ));
        let chat = hub
            .insert_window(
                titled("chat"),
                PixelRect::new(10, 5, 30, 20),
                WindowRestrictions::None,
            )
            .unwrap();
        if app_fullscreen {
            hub.set_fullscreen(chat, WindowRestrictions::ProtectFullscreen);
        } else {
            hub.toggle_float();
        }
        let before = snapshot_text(&hub);

        hub.update_float_rect(chat, PixelRect::new(160, 5, 30, 20), MonitorId::new(1));
        validate_hub(&hub);
        assert_eq!(
            snapshot_text(&hub),
            before,
            "{strategy:?}, app_fullscreen={app_fullscreen}"
        );
    }
}

#[test]
#[should_panic(expected = "Node WindowId(999) not found or was deleted")]
fn update_float_rect_on_unknown_panics() {
    let mut hub = setup_with_modes("0", &["w31"], &[]);
    hub.insert_window(
        titled("w31"),
        PixelRect::new(10, 5, 30, 20),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.update_float_rect(
        WindowId::new(999),
        PixelRect::new(10, 5, 30, 20),
        MonitorId::new(0),
    );
}

#[test]
fn float_whose_border_leaves_no_room_for_content_gets_no_placement() {
    let mut hub = setup_with_modes("0", &["w27"], &[]);
    hub.sync_configuration(
        TilingConfigBuilder::new()
            .with_border_size(Pixels::new(20))
            .build(),
    );
    let float = hub
        .insert_window(
            titled("w27"),
            PixelRect::new(10, 0, 30, 30),
            WindowRestrictions::None,
        )
        .unwrap();

    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { float_windows, .. } = &placements.monitors[0].layout else {
        panic!("the monitor shows a fullscreen window");
    };
    assert!(
        float_windows.iter().all(|placement| placement.id != float),
        "{float_windows:?}"
    );
}
