use crate::core::node::Dimension;
use crate::core::tests::{setup, snapshot};
use insta::assert_snapshot;

#[test]
fn insert_float_window() {
    let mut hub = setup();
    hub.insert_float(
        Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 20.0,
        },
        "Float1".into(),
    );
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=FloatWindowId(0),
        Float(id=FloatWindowId(0), title="Float1", x=10.00, y=5.00, w=30.00, h=20.00)
      )
    )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
             ********************************                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *            Float1            *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             ********************************
    "#);
}

#[test]
fn float_window_with_tiling() {
    let mut hub = setup();
    hub.insert_tiling("W0".into());
    hub.insert_float(
        Dimension {
            x: 50.0,
            y: 5.0,
            width: 40.0,
            height: 15.0,
        },
        "Float1".into(),
    );
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=FloatWindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
        Float(id=FloatWindowId(0), title="Float1", x=50.00, y=5.00, w=40.00, h=15.00)
      )
    )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                ******************************************                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                 Float1                 *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                        W0              *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                *                                        *                                                          |
    |                                                ******************************************                                                          |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    "#);
}

#[test]
fn delete_float_window() {
    let mut hub = setup();
    hub.insert_tiling("W0".into());
    let f0 = hub.insert_float(
        Dimension {
            x: 50.0,
            y: 5.0,
            width: 40.0,
            height: 15.0,
        },
        "Float1".into(),
    );
    hub.delete_float(f0);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
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
fn move_float_to_workspace() {
    let mut hub = setup();
    hub.insert_tiling("W0".into());
    hub.insert_float(
        Dimension {
            x: 50.0,
            y: 5.0,
            width: 40.0,
            height: 15.0,
        },
        "Float1".into(),
    );
    hub.move_focused_to_workspace(1);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
      )
      Workspace(id=WorkspaceId(1), name=1, focused=FloatWindowId(0),
        Float(id=FloatWindowId(0), title="Float1", x=50.00, y=5.00, w=40.00, h=15.00)
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
    "#);
}

#[test]
fn focus_falls_back_to_tiling_after_float_delete() {
    let mut hub = setup();
    hub.insert_tiling("W0".into());
    let f0 = hub.insert_float(
        Dimension {
            x: 50.0,
            y: 5.0,
            width: 40.0,
            height: 15.0,
        },
        "Float1".into(),
    );
    // Float is focused after insert
    hub.delete_float(f0);
    // Focus should fall back to tiling window
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=1.00, y=1.00, w=148.00, h=28.00)
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
fn focus_falls_back_to_last_float() {
    let mut hub = setup();
    hub.insert_float(
        Dimension {
            x: 10.0,
            y: 5.0,
            width: 30.0,
            height: 10.0,
        },
        "Float0".into(),
    );
    let f1 = hub.insert_float(
        Dimension {
            x: 50.0,
            y: 5.0,
            width: 30.0,
            height: 10.0,
        },
        "Float1".into(),
    );
    // f1 is focused
    hub.delete_float(f1);
    // Focus should fall back to f0
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
      Workspace(id=WorkspaceId(0), name=0, focused=FloatWindowId(0),
        Float(id=FloatWindowId(0), title="Float0", x=10.00, y=5.00, w=30.00, h=10.00)
      )
    )

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
             ********************************                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *            Float0            *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             *                              *                                                                                                             
             ********************************
    "#);
}
