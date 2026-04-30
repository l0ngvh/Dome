use crate::core::allocator::NodeId;
use crate::core::node::{Dimension, WindowId};
use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn insert_float_window() {
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    assert_snapshot!(snapshot(&hub), @"
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });
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
    let mut hub = setup();
    hub.insert_tiling();
    let f0 = hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });
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
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    let f1 = hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    // f1 is focused
    hub.delete_window(f1);
    // Focus should fall back to f0
    assert_snapshot!(snapshot(&hub), @"
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
    hub.insert_tiling();
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
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 40.0,
        height: 15.0,
    });
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
    hub.insert_tiling();
    hub.insert_tiling();

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
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[, ])
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
    let mut hub = setup();

    hub.insert_tiling();

    hub.focus_workspace("1");
    let f0 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });

    // Insert a tiling window on workspace 1
    let w1 = hub.insert_tiling();

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

    // Delete the tiling window on workspace 1 (workspace 1 should NOT be deleted because it has a float)
    hub.delete_window(w1);

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
    hub.delete_window(f0);

    assert_eq!(snapshot(&hub), after_tiling_delete);
}

#[test]
fn delete_unfocused_float_window() {
    use crate::core::node::Dimension;
    let mut hub = setup();

    let f0 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.insert_tiling();

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
fn delete_float_workspace_pruning() {
    // Scenario 1: delete float on current workspace -- workspace kept
    {
        let mut hub = setup();
        let f0 = hub.insert_float(Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        });
        hub.delete_window(f0);
        assert_snapshot!(snapshot(&hub), @"
        Hub(focused=None)
          Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
        ");
    }

    // Scenario 2: non-current workspace kept because tiling exists
    {
        let mut hub = setup();
        hub.focus_workspace("1");
        hub.insert_tiling();
        let f0 = hub.insert_float(Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        });
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_snapshot!(snapshot(&hub), @"
        Hub(focused=None)
          Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
        ");
        assert_eq!(
            hub.all_workspaces().len(),
            2,
            "ws1 should still exist (has tiling window)"
        );
    }

    // Scenario 3: non-current workspace kept because other float exists
    {
        let mut hub = setup();
        hub.focus_workspace("1");
        let f0 = hub.insert_float(Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        });
        hub.insert_float(Dimension {
            x: 50.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        });
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_snapshot!(snapshot(&hub), @"
        Hub(focused=None)
          Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
        ");
        assert_eq!(
            hub.all_workspaces().len(),
            2,
            "ws1 should still exist (has another float)"
        );
    }

    // Scenario 4: empty non-current workspace pruned
    {
        let mut hub = setup();
        hub.focus_workspace("1");
        let f0 = hub.insert_float(Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        });
        hub.focus_workspace("0");
        hub.delete_window(f0);
        assert_snapshot!(snapshot(&hub), @"
        Hub(focused=None)
          Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
        ");
        assert_eq!(
            hub.all_workspaces().len(),
            1,
            "ws1 should be pruned (was empty)"
        );
    }
}

#[test]
fn insert_float_offscreen_does_not_scroll_viewport() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 200.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });

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
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.update_float_dimension(
        WindowId::new(0),
        Dimension {
            x: 50.0,
            y: 20.0,
            width: 60.0,
            height: 40.0,
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=50.00, y=20.00, w=60.00, h=10.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                      ************************************************************                                        
                                                      *                                                          *                                        
                                                      *                                                          *                                        
                                                      *                                                          *                                        
                                                      *                                                          *                                        
                                                      *                            F0                            *                                        
                                                      *                                                          *                                        
                                                      *                                                          *                                        
                                                      *                                                          *                                        
                                                      *                                                          *
    ");
}

#[test]
fn update_float_dimension_preserves_z_order() {
    let mut hub = setup();
    let a = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    hub.insert_float(Dimension {
        x: 90.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    // Move a (index 0) without changing z-order or focus (c stays topmost/focused)
    hub.update_float_dimension(
        a,
        Dimension {
            x: 15.0,
            y: 10.0,
            width: 30.0,
            height: 20.0,
        },
    );
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=15.00, y=10.00, w=30.00, h=20.00, float)
        Window(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=20.00, float)
        Window(id=WindowId(2), x=90.00, y=5.00, w=30.00, h=20.00, float, highlighted)
      )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                      +----------------------------+          ******************************                              
                                                      |                            |          *                            *                              
                                                      |                            |          *                            *                              
                                                      |                            |          *                            *                              
                                                      |                            |          *                            *                              
                   +----------------------------+     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |             F1             |          *             F2             *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |             F0             |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     |                            |          *                            *                              
                   |                            |     +----------------------------+          ******************************                              
                   |                            |                                                                                                         
                   |                            |                                                                                                         
                   |                            |                                                                                                         
                   |                            |                                                                                                         
                   +----------------------------+
    ");
}

#[test]
#[should_panic(expected = "is not Float")]
fn update_float_dimension_on_tiling_panics() {
    let mut hub = setup();
    let w = hub.insert_tiling();
    hub.update_float_dimension(
        w,
        Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        },
    );
}

#[test]
#[should_panic]
fn update_float_dimension_on_unknown_panics() {
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 20.0,
    });
    // WindowId(999) was never inserted -- panics in allocator.get()
    hub.update_float_dimension(
        WindowId::new(999),
        Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        },
    );
}
