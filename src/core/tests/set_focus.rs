use crate::core::Dimension;
use crate::core::node::WindowRestrictions;
use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn set_focus_same_workspace() {
    let mut hub = setup();

    let w0 = hub.insert_tiling();
    let w1 = hub.insert_tiling();

    hub.set_focus(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
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
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, direction=Horizontal,
          Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
          Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        )
      )
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
fn set_focus_switches_workspace() {
    let mut hub = setup();

    let w0 = hub.insert_tiling();
    hub.focus_workspace("1");
    hub.insert_tiling();

    hub.set_focus(w0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00)
      )
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
fn set_focus_float_same_workspace() {
    let mut hub = setup();

    let w0 = hub.insert_tiling();
    let f0 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });

    hub.set_focus(w0);
    hub.set_focus(f0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Float(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=10.00)
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |         ******************************                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *             F1             *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         *                            *                                                                                                             |
    |         ******************************                                                                                                             |
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
fn set_focus_float_switches_workspace() {
    let mut hub = setup();

    let f0 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    hub.focus_workspace("1");
    hub.insert_tiling();

    hub.set_focus(f0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Float(id=WindowId(0), x=10.00, y=5.00, w=30.00, h=10.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=WindowId(1),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00)
      )
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
fn set_focus_to_a_different_workspace_prune_previous_workspace() {
    let mut hub = setup();
    let w0 = hub.insert_tiling();

    hub.move_focused_to_workspace("2");
    hub.set_focus(w0);

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(1), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(1), name=2, focused=WindowId(0),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
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
fn float_focus_changes_float_z_order() {
    let mut hub = setup();
    let w0 = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    let w1 = hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    let _w2 = hub.insert_float(Dimension {
        x: 100.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });

    hub.set_focus(w0);
    hub.set_focus(w1);
    // Now z-order from top to bottom should be [w1, w0, w2]
    hub.delete_window(w0);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Float(id=WindowId(2), x=100.00, y=5.00, w=30.00, h=10.00)
        Float(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00)
      )
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_fullscreen(WindowRestrictions::None);
    let fs2 = hub.insert_fullscreen(WindowRestrictions::None);

    hub.delete_window(fs2);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Fullscreen(id=WindowId(1), x=0.00, y=0.00, w=0.00, h=0.00)
      )
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    let fs = hub.insert_fullscreen(WindowRestrictions::None);

    hub.delete_window(fs);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Float(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00)
      )
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
    let mut hub = setup();
    let t = hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });

    hub.set_focus(t);
    hub.delete_window(t);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Float(id=WindowId(1), x=10.00, y=5.00, w=30.00, h=10.00)
      )
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
fn detach_non_topmost_float_keeps_topmost_focused() {
    let mut hub = setup();
    let a = hub.insert_float(Dimension {
        x: 10.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });
    hub.insert_float(Dimension {
        x: 50.0,
        y: 5.0,
        width: 30.0,
        height: 10.0,
    });

    hub.delete_window(a);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Float(id=WindowId(1), x=50.00, y=5.00, w=30.00, h=10.00)
      )
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
fn detach_non_topmost_fullscreen_no_focus_change() {
    let mut hub = setup();
    let fs1 = hub.insert_fullscreen(WindowRestrictions::None);
    hub.insert_fullscreen(WindowRestrictions::None);

    hub.delete_window(fs1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Fullscreen(id=WindowId(1), x=0.00, y=0.00, w=0.00, h=0.00)
      )
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
