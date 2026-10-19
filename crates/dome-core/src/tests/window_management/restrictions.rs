use insta::assert_snapshot;

use crate::action::Action;
use crate::core::WindowRestrictions;
use crate::tests::fixtures::{TestDome, monitor, window};

#[test]
fn run_or_click_tab_under_block_all_focus_changes_nothing_but_effects() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor right");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("focus monitor left");
    let w2 = dome.open(window("w2"));
    let w3 = dome.open(window("w3"));
    dome.minimize(w3);
    dome.set_fullscreen(w2, WindowRestrictions::BlockAll);
    let before = dome.snapshot();
    assert_snapshot!(before, @r#"
    focused w2 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    minimized: w3 "w3"
    primary 0,0 100x20 border 1 fullscreen w2
    DELL 100,0 40x10 border 1
      w1 100,2 40x8
      container 100,0 40x10 tabbed active=1 titles=[w0, w1]
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

    +--------------------------------------+
    |        w0         |      [w1]        |
    +--------------------------------------+
    |                                      |
    |                                      |
    |                  w1                  |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    "#);

    for command in [
        "focus workspace 1",
        "move workspace 1",
        "focus monitor right",
        "move monitor right",
        "toggle fullscreen",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
    dome.click_tab(dome.tab_container(&["w0", "w1"]), 0);
    assert_eq!(dome.snapshot(), before, "tab click");

    for command in ["close", "execute open -a Safari", "exit", "mode resize"] {
        dome.run(command);
    }
    dome.run_action(Action::UnminimizeWindow { id: w3 });
    assert_snapshot!(dome.snapshot(), @r#"
    focused w2 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    minimized: w3 "w3"
    effects: close w2, execute "open -a Safari", mode resize, exit, unminimize w3
    primary 0,0 100x20 border 1 fullscreen w2
    DELL 100,0 40x10 border 1
      w1 100,2 40x8
      container 100,0 40x10 tabbed active=1 titles=[w0, w1]
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

    +--------------------------------------+
    |        w0         |      [w1]        |
    +--------------------------------------+
    |                                      |
    |                                      |
    |                  w1                  |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    "#);
}

#[test]
fn open_and_close_under_block_all_focus_still_change_windows_behind_it() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w1, WindowRestrictions::BlockAll);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen w1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
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

    dome.open(window("w2"));
    assert_eq!(dome.snapshot(), before);
    dome.close(w0);
    assert_eq!(dome.snapshot(), before);
    dome.unset_fullscreen(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 50,0 50x20 highlighted spawn=right
      w2 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn unset_fullscreen_of_block_all_window_lets_commands_run() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w1, WindowRestrictions::BlockAll);

    dome.unset_fullscreen(w1);
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
      w1 50,0 50x20 float highlighted
    +-------------------------------------------------**************************************************
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                w*                       w1                       *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    +-------------------------------------------------**************************************************
    ");
}

#[test]
fn close_block_all_window_lets_commands_run() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w1, WindowRestrictions::BlockAll);

    dome.close(w1);
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 float highlighted
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
}

#[test]
fn run_under_protect_fullscreen_focus_blocks_mode_change_and_monitor_move_only() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1 fullscreen w1
    DELL 100,0 40x10 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
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

    for command in ["toggle fullscreen", "move monitor right"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 0 on DELL (visible), 1 on primary (focused)
    primary 0,0 100x20 border 1
    DELL 100,0 40x10 border 1
    ");
    dome.run("focus workspace 0");
    assert_eq!(dome.snapshot(), before);
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
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
}

#[test]
fn set_fullscreen_again_replaces_restriction_of_focused_or_covered_window() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w0, WindowRestrictions::ProtectFullscreen);
    dome.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);

    dome.set_fullscreen(w1, WindowRestrictions::BlockAll);
    let upgraded = dome.snapshot();
    assert_snapshot!(upgraded, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen w1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
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
    dome.run("move workspace 1");
    assert_eq!(dome.snapshot(), upgraded);

    dome.set_fullscreen(w0, WindowRestrictions::BlockAll);
    assert_eq!(dome.snapshot(), upgraded);

    dome.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);
    dome.run("move workspace 1");
    let downgraded = dome.snapshot();
    assert_snapshot!(downgraded, @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1 fullscreen w0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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
    dome.run("move workspace 1");
    assert_eq!(dome.snapshot(), downgraded);
}

#[test]
fn toggle_float_while_block_all_window_lacks_focus_floats_focused_window() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor right");
    let w0 = dome.open(window("w0"));
    dome.run("focus monitor left");
    let w1 = dome.open(window("w1"));
    dome.set_fullscreen(w1, WindowRestrictions::BlockAll);

    dome.focus(w0);
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused)
    primary 0,0 100x20 border 1 fullscreen w1
    DELL 100,0 40x10 border 1
      w0 100,0 40x10 float highlighted
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
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

    ****************************************
    *                                      *
    *                                      *
    *                                      *
    *                  w0                  *
    *                                      *
    *                                      *
    *                                      *
    *                                      *
    ****************************************
    ");
}
