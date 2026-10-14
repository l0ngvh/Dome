use insta::assert_snapshot;

use crate::core::{Length, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::tests::test_dome::{TestDome, window};

#[test]
fn minimize_in_any_display_mode_moves_window_once_to_minimized_list() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.run("toggle float");
    dome.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);

    dome.minimize(w1);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    minimized: w1 "w1"
    primary 0,0 100x20 border 1
      w0 0,0 100x20
      w2 67,0 33x20 float highlighted
    +------------------------------------------------------------------*********************************
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                w0                *               w2              *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    |                                                                  *                               *
    +------------------------------------------------------------------*********************************
    "#);
    dome.minimize(w2);
    let minimized_float = dome.snapshot();
    assert_snapshot!(minimized_float, @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    minimized: w1 "w1", w2 "w2"
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    "#);
    dome.minimize(w2);
    assert_eq!(dome.snapshot(), minimized_float);
    dome.minimize(w0);
    assert_snapshot!(dome.snapshot(), @r#"
    focused none on primary
    workspaces: 0 on primary (focused)
    minimized: w1 "w1", w2 "w2", w0 "w0"
    primary 0,0 100x20 border 1
    "#);
}

#[test]
fn unminimize_restores_window_to_current_workspace_in_display_mode_it_had() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    let w3 = dome.open(window("w3"));
    dome.run("toggle float");
    dome.move_float(w3, [10, 2, 30, 8], dome.primary_monitor());
    dome.set_fullscreen(w2, WindowRestrictions::BlockAll);
    dome.minimize(w1);
    dome.minimize(w2);
    dome.minimize(w3);
    let minimized = dome.snapshot();
    dome.unset_fullscreen(w2);
    assert_eq!(dome.snapshot(), minimized);
    dome.run("focus workspace 1");

    dome.unminimize(w1);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w1 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    minimized: w2 "w2", w3 "w3"
    primary 0,0 100x20 border 1
      w1 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    "#);
    dome.unminimize(w3);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w3 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    minimized: w2 "w2"
    primary 0,0 100x20 border 1
      w1 0,0 100x20
      w3 9,1 32x10 float highlighted
    +--------------------------------------------------------------------------------------------------+
    |        ********************************                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *              w3              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *        w1                                                |
    |        ********************************                                                          |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    "#);
    dome.unminimize(w2);
    let fullscreen = dome.snapshot();
    assert_snapshot!(fullscreen, @"
    focused w2 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen w2
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w2                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
    dome.run("focus workspace 0");
    assert_eq!(dome.snapshot(), fullscreen);
    dome.unminimize(w0);
    assert_eq!(dome.snapshot(), fullscreen);
}

#[test]
fn close_minimized_window_drops_it_from_minimized_list_for_good() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.minimize(w1);

    dome.close(w1);
    let closed = dome.snapshot();
    assert_snapshot!(closed, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
    dome.unminimize(w1);
    assert_eq!(dome.snapshot(), closed);
}

#[test]
fn set_constraint_while_minimized_holds_after_unminimize() {
    let mut dome = TestDome::new("return { layout = 'master' }");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    dome.minimize(w1);
    dome.set_constraint(
        w1,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(10.0)),
            ..Default::default()
        },
    );
    dome.unminimize(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 64,4 22x12 highlighted
    +------------------------------------------------+                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |              **********************              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                       w0                       |              *         w1         *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              *                    *              
    |                                                |              **********************              
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    +------------------------------------------------+
    ");
}

#[test]
fn move_float_while_minimized_leaves_restored_frame_unchanged() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle float");
    dome.move_float(w1, [10, 2, 30, 8], dome.primary_monitor());

    dome.minimize(w1);
    dome.move_float(w1, [50, 5, 40, 10], dome.primary_monitor());
    dome.unminimize(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
      w1 9,1 32x10 float highlighted
    +--------------------------------------------------------------------------------------------------+
    |        ********************************                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *              w1              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *                                                          |
    |        *                              *        w0                                                |
    |        ********************************                                                          |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn minimize_lists_windows_in_minimize_order_under_current_title() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    let w2 = dome.open(window("w2"));

    dome.minimize(w2);
    dome.minimize(w0);
    dome.set_title(w2, "renamed");
    assert_snapshot!(dome.snapshot(), @r#"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    minimized: w2 "renamed", w0 "w0"
    primary 0,0 100x20 border 1
      w1 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    "#);
}
