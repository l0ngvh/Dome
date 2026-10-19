use insta::assert_snapshot;

use crate::core::{FractionalLength, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::tests::fixtures::{TestDome, window};

const FLOAT_AND_FULLSCREEN_RULES: &str = r#"return { primary = { ["0"] = {
    layout = "partition_tree",
    float = { { title = "f0" }, { title = "f1" } },
    fullscreen = { { title = "fs0" }, { title = "fs1" } },
} } }"#;

#[test]
fn open_on_empty_workspace_fills_work_area() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
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
}

#[test]
fn open_on_occupied_workspace_takes_even_share_right_of_focus() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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

    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x20
      w2 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              ||               w1               |*               w2              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");

    dome.focus(w1);
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 25x20
      w1 25,0 25x20
      w2 75,0 25x20
      w3 50,0 25x20 highlighted spawn=right
    +-----------------------++-----------------------+*************************+-----------------------+
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |           w0          ||           w1          |*           w3          *|           w2          |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    +-----------------------++-----------------------+*************************+-----------------------+
    ");
}

#[test]
fn open_after_toggle_spawn_wraps_focused_window_in_new_container() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
    dome.run("toggle spawn");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 50x10
      w2 50,10 50x10 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.run("toggle spawn");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 50x10
      w2 50,10 50x5
      w3 50,15 50x5 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                       w1                       |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn open_with_container_highlighted_appends_along_its_split() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted spawn=bottom
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("focus parent");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x20
      w2 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              ||               w1               |*               w2              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
}

#[test]
fn open_with_container_highlighted_across_its_split_goes_beside_it() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("toggle spawn");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x10
      w2 33,10 34x10
      w3 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w1               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              |+--------------------------------+*               w3              *
    |                               |+--------------------------------+*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w2               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus parent");
    dome.run("toggle spawn");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 50,0 50x10
      w2 0,10 100x10 highlighted spawn=bottom
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn open_with_tabbed_container_highlighted_splits_beside_it() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("focus parent");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 50x18
      w2 50,0 50x20 highlighted spawn=right
      container 0,0 50x20 tabbed active=1 titles=[w0, w1]
    +------------------------------------------------+**************************************************
    |          w0            |        [w1]           |*                                                *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                       w1                       |*                                                *
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

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("focus parent");
    dome.run("toggle spawn");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x8
      w2 0,10 100x10 highlighted spawn=bottom
      container 0,0 100x10 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                     [w1]                       |
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn open_inside_tab_splits_tab_with_its_window() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 50x18
      w2 50,2 50x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                  [Container]                   |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w2                       *
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
    dome.run("focus parent");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 33x18
      w2 33,2 34x18
      w3 67,2 33x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                  [Container]                   |
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w1              ||               w2               |*               w3              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
}

#[test]
fn open_with_automatic_tiling_spawns_along_longer_side() {
    let mut dome = TestDome::new("return { partition_tree = { automatic_tiling = true } }");
    for title in ["w0", "w1", "w2", "w3", "w4", "w5"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w5 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 17x20
      w1 17,0 16x20
      w2 33,0 17x20
      w3 50,0 17x20
      w4 67,0 16x20
      w5 83,0 17x20 highlighted spawn=bottom
    +---------------++--------------++---------------++---------------++--------------+*****************
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |       w0      ||      w1      ||       w2      ||       w3      ||      w4      |*       w5      *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    +---------------++--------------++---------------++---------------++--------------+*****************
    ");
    dome.open(window("w6"));
    assert_snapshot!(dome.snapshot(), @"
    focused w6 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 17x20
      w1 17,0 16x20
      w2 33,0 17x20
      w3 50,0 17x20
      w4 67,0 16x20
      w5 83,0 17x10
      w6 83,10 17x10 highlighted spawn=right
    +---------------++--------------++---------------++---------------++--------------++---------------+
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||       w5      |
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||               |
    |               ||              ||               ||               ||              ||               |
    |       w0      ||      w1      ||       w2      ||       w3      ||      w4      |+---------------+
    |               ||              ||               ||               ||              |*****************
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*       w6      *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    |               ||              ||               ||               ||              |*               *
    +---------------++--------------++---------------++---------------++--------------+*****************
    ");
}

#[test]
fn open_after_rotating_with_automatic_tiling_uses_new_longer_side() {
    let mut dome = TestDome::new("return { partition_tree = { automatic_tiling = true } }");
    for title in ["w0", "w1", "w2", "w3", "w4", "w5"] {
        dome.open(window(title));
    }
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused w5 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x3
      w1 0,3 100x4
      w2 0,7 100x3
      w3 0,10 100x3
      w4 0,13 100x4
      w5 0,17 100x3 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                w0                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w1                                                |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w2                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w3                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w4                                                |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                w5                                                *
    ****************************************************************************************************
    ");
    dome.open(window("w6"));
    assert_snapshot!(dome.snapshot(), @"
    focused w6 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x3
      w1 0,3 100x4
      w2 0,7 100x3
      w3 0,10 100x3
      w4 0,13 100x4
      w5 0,17 50x3
      w6 50,17 50x3 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                w0                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w1                                                |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w2                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w3                                                |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                w4                                                |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                       w5                       |*                       w6                       *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn run_ignored_or_targetless_verb_changes_nothing() {
    let mut dome = TestDome::new("return {}");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
    ");
    for verb in [
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "focus tab next",
        "move left",
    ] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb} on empty workspace");
    }

    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("f0").frame([10, 3, 40, 10]));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
              ****************************************                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                  f0                  *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              ****************************************
    ");
    for verb in [
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "focus tab next",
        "move left",
    ] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb} on float-only workspace");
    }

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
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
    for verb in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "focus tab next",
        "focus tab prev",
    ] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb} on single-window workspace");
    }

    let float_over_tabs = || {
        let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
        dome.open(window("w0"));
        dome.open(window("w1"));
        dome.run("toggle spawn");
        dome.open(window("w2"));
        dome.run("toggle layout");
        let float = dome.open(window("f0").frame([10, 3, 40, 10]));
        (dome, float)
    };
    let (mut dome, float) = float_over_tabs();
    dome.close(float);
    let tiling_focus = dome.snapshot();
    assert_snapshot!(tiling_focus, @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x18 highlighted spawn=bottom
      container 50,0 50x20 tabbed active=1 titles=[w1, w2]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w1            |        [w2]           |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                       w2                       *
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
    for verb in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "focus tab next",
    ] {
        let (mut dome, float) = float_over_tabs();
        let before = dome.snapshot();
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb} under focused float");
        dome.close(float);
        assert_eq!(
            dome.snapshot(),
            tiling_focus,
            "{verb} moved hidden tiling focus"
        );
    }

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    for verb in [
        "grow",
        "shrink",
        "master more",
        "master fewer",
        "focus tab next",
        "focus tab prev",
    ] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb} on two-window split");
    }
}

#[test]
fn toggle_layout_on_split_tabs_it_with_focus_on_active_tab() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w1);
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |              w0                |            [w1]               |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus parent");
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x9
      w2 0,11 100x9
      container 0,0 100x20 tabbed active=1 titles=[w0, Container] highlighted spawn=right
    ****************************************************************************************************
    *                       w0                        |                  [Container]                   *
    *--------------------------------------------------------------------------------------------------*
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *--------------------------------------------------------------------------------------------------*
    *--------------------------------------------------------------------------------------------------*
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn toggle_layout_on_tabbed_container_restores_alternating_split() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("focus tab prev");
    dome.open(window("w4"));
    assert_snapshot!(dome.snapshot(), @"
    focused w4 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 50x18
      w4 50,2 50x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, Container, Container]
    +--------------------------------------------------------------------------------------------------+
    |              w0                |         [Container]           |           Container             |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w4                       *
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
    dome.run("focus parent");
    dome.run("focus parent");
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x10
      w2 67,0 33x10
      w3 67,10 33x10
      w4 33,10 34x10
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||               w1               ||               w2              *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *               w0              |+--------------------------------++-------------------------------*
    *                               |+--------------------------------++-------------------------------*
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||               w4               ||               w3              *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    *                               ||                                ||                               *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w2 0,12 100x8 highlighted spawn=bottom
      container 0,10 100x10 tabbed active=1 titles=[w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                       w1                        |                     [w2]                       |
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 50x10
      w2 50,10 50x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn toggle_layout_on_nested_container_keeps_its_share_of_parent() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w1);
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w2 67,0 33x20
      w3 33,2 34x18 highlighted spawn=bottom
      container 33,0 34x20 tabbed active=1 titles=[w1, w3]
    +-------------------------------++--------------------------------++-------------------------------+
    |                               ||      w1        |    [w3]       ||                               |
    |                               |**********************************|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w0              |*                                *|               w2              |
    |                               |*               w3               *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");
}

#[test]
fn focus_tab_next_and_prev_cycle_active_tab_and_wrap() {
    insta::allow_duplicates! {
        for highlight_container in [false, true] {
            let mut settings = insta::Settings::clone_current();
            settings.set_snapshot_suffix(format!("highlight_container={highlight_container}"));
            let _settings = settings.bind_to_scope();
            let mut dome = TestDome::new("return {}");
            dome.open(window("w0"));
            dome.open(window("w1"));
            dome.open(window("w2"));
            dome.run("toggle layout");
            if highlight_container {
                dome.run("focus parent");
            }
            dome.run("focus tab next");
            assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |             [w0]               |             w1                |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
            dome.run("focus tab next");
            assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |              w0                |            [w1]               |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
            dome.run("focus tab prev");
            assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |             [w0]               |             w1                |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
            dome.run("focus tab prev");
            assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=2 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |              w0                |             w1                |             [w2]                |
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
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
    }
}

#[test]
fn focus_tab_onto_nested_container_focuses_its_last_focused_window() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    let w2 = dome.open(window("w2"));
    dome.open(window("w3"));
    dome.focus(w2);
    dome.run("focus tab prev");
    dome.run("focus tab next");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 33x18
      w2 33,2 34x18 highlighted spawn=right
      w3 67,2 33x18
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                  [Container]                   |
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w1              |*               w2               *|               w3              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");
}

#[test]
fn focus_on_hidden_tab_window_activates_its_tab() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.focus(w0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |             [w0]               |             w1                |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
}

#[test]
fn close_active_tab_window_activates_tab_before_it() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    let w3 = dome.open(window("w3"));
    dome.run("toggle layout");
    dome.focus(w0);
    dome.close(w3);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted spawn=right
      w2 50,2 50x18
      container 50,0 50x20 tabbed active=1 titles=[w1, w2]
    **************************************************+------------------------------------------------+
    *                                                *|          w1            |        [w2]           |
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                                                |
    *                                                *|                       w2                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn close_inactive_tab_window_keeps_active_tab() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    let w3 = dome.open(window("w3"));
    dome.focus(w1);
    dome.run("toggle layout");
    dome.close(w0);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w1, w2, w3]
    +--------------------------------------------------------------------------------------------------+
    |             [w1]               |             w2                |              w3                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
    dome.close(w3);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |                      [w1]                       |                      w2                        |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
}

#[test]
fn click_tab_activates_tab_and_ignores_index_past_last() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.click_tab(dome.tab_container(&["w0", "w1", "w2"]), 0);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |             [w0]               |             w1                |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
    dome.click_tab(dome.tab_container(&["w0", "w1", "w2"]), 3);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn toggle_layout_with_tab_bar_taller_than_container_places_no_window() {
    let mut dome = TestDome::new("return { partition_tree = { tab_bar_height = 25 } }");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                       w0                        |                     [w1]                       |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    ");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                       w0                        |                  [Container]                   |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    ");
}

#[test]
fn open_inside_tab_places_windows_below_tall_tab_bar() {
    let mut dome = TestDome::new("return { partition_tree = { tab_bar_height = 15 } }");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,15 50x5
      w2 50,15 50x5 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                       w0                        |                  [Container]                   |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                       w1                       |*                       w2                       *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn nested_tab_bars_exhaust_window_and_deeper_container_height() {
    let mut dome = TestDome::new("return { partition_tree = { tab_bar_height = 15 } }");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.open(window("w2"));
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
      container 0,15 100x5 tabbed active=1 titles=[w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                       w0                        |                  [Container]                   |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w1                        |                     [w2]                       |
    |                                                 |                                                |
    |                                                 |                                                |
    ");
    dome.open(window("w3"));
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
      container 0,15 100x5 tabbed active=1 titles=[w1, Container]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                       w0                        |                  [Container]                   |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w1                        |                  [Container]                   |
    |                                                 |                                                |
    |                                                 |                                                |
    ");
}

#[test]
fn set_constraint_on_partition_tree_window_keeps_even_split() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    for observed in [
        LimitObservation {
            min_width: LimitUpdate::Set(FractionalLength::new(60.0)),
            ..Default::default()
        },
        LimitObservation {
            max_width: LimitUpdate::Set(FractionalLength::new(20.0)),
            ..Default::default()
        },
        LimitObservation {
            min_height: LimitUpdate::Set(FractionalLength::new(60.0)),
            ..Default::default()
        },
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            max_width: LimitUpdate::Cleared,
            min_height: LimitUpdate::Cleared,
            ..Default::default()
        },
    ] {
        dome.set_constraint(w0, observed);
        assert_eq!(dome.snapshot(), before, "{observed:?}");
    }

    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
    for min_height in [
        LimitUpdate::Set(FractionalLength::new(60.0)),
        LimitUpdate::Cleared,
    ] {
        dome.set_constraint(
            w0,
            LimitObservation {
                min_height,
                ..Default::default()
            },
        );
        assert_eq!(
            dome.snapshot(),
            before,
            "vertical split with {min_height:?}"
        );
    }

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle layout");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                     [w1]                       |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
    dome.set_constraint(
        w1,
        LimitObservation {
            min_height: LimitUpdate::Set(FractionalLength::new(60.0)),
            ..Default::default()
        },
    );
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn reload_with_new_tab_bar_height_relays_every_workspace() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("focus workspace 1");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("toggle layout");
    dome.reload("return { partition_tree = { tab_bar_height = 4 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w3 0,4 100x16 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w2, w3]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w2                        |                     [w3]                       |
    |                                                 |                                                |
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w3                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w1 0,4 100x16 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w0                        |                     [w1]                       |
    |                                                 |                                                |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
}

#[test]
fn focus_direction_inside_split_moves_highlight_to_neighbour() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted spawn=right
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                       w1                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10 highlighted spawn=bottom
      w1 0,10 100x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn focus_direction_past_split_end_climbs_to_ancestor_sibling() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 100x10 highlighted spawn=bottom
      w2 50,0 50x10
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.open(window("w3"));
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted spawn=right
      w2 50,2 50x9
      w3 50,11 50x9
      container 50,0 50x20 tabbed active=1 titles=[w1, Container]
    **************************************************+------------------------------------------------+
    *                                                *|          w1            |     [Container]       |
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w2                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                                                |
    *                                                *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w3                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn focus_direction_into_container_lands_on_its_last_focused_window() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    let w2 = dome.open(window("w2"));
    dome.open(window("w3"));
    dome.focus(w2);
    dome.focus(w0);
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6 highlighted spawn=bottom
      w3 50,13 50x7
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                       w0                       |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn focus_direction_at_workspace_edge_changes_nothing() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    for command in ["focus right", "focus up", "focus down"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command} at right edge");
    }
    dome.focus(w0);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted spawn=right
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                       w1                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    for command in ["focus left", "focus up", "focus down"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command} at left edge");
    }
}

#[test]
fn focus_parent_climbs_one_container_and_stops_at_root() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10
      container 50,0 50x20 highlighted spawn=bottom
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.run("focus parent");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                       w1                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w0                       |+------------------------------------------------*
    *                                                |+------------------------------------------------*
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                       w2                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
    dome.run("focus parent");
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn move_direction_along_split_swaps_with_neighbour() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w0);
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 33,0 34x20 highlighted spawn=right
      w1 0,0 33x20
      w2 67,0 33x20
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w1              |*               w0               *|               w2              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");

    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("move down");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,10 100x10 highlighted spawn=bottom
      w1 0,0 100x10
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn move_direction_out_of_container_lands_beside_it_in_ancestor() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.focus(w1);
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 67,0 33x20 highlighted spawn=right
      w2 33,0 34x20
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              ||               w2               |*               w1              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 67,0 33x20
      w2 33,0 34x20 highlighted spawn=right
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w0              |*               w2               *|               w1              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.open(window("w3"));
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w2 67,2 33x18
      w3 33,0 34x20 highlighted spawn=right
      container 67,0 33x20 tabbed active=1 titles=[w1, w2]
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|      w1       |    [w2]       |
    |                               |*                                *+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w0              |*               w3               *|                               |
    |                               |*                                *|               w2              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x10
      w2 33,10 34x10
      w3 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w1               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              |+--------------------------------+*               w3              *
    |                               |+--------------------------------+*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w2               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
}

#[test]
fn move_direction_without_ancestor_on_axis_wraps_root_in_new_split() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w1);
    dome.run("move down");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 100x10 highlighted spawn=bottom
      w2 50,0 50x10
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let middle = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(middle);
    dome.run("move up");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,10 50x10
      w1 0,0 100x10 highlighted spawn=bottom
      w2 50,10 50x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn move_container_into_parent_of_same_direction_flips_it() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("toggle spawn");
    dome.open(window("w4"));
    dome.run("focus parent");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 25x7
      w4 75,13 25x7
      container 50,13 50x7 highlighted spawn=right
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                       ||                       *
    |                                                |*                       ||                       *
    |                                                |*           w3          ||           w4          *
    |                                                |*                       ||                       *
    |                                                |*                       ||                       *
    +------------------------------------------------+**************************************************
    ");
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 67,0 33x10
      w2 67,10 33x10
      w3 33,0 34x10
      w4 33,10 34x10
      container 33,0 34x20 highlighted spawn=right
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*               w3               *|               w1              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w0              |*--------------------------------*+-------------------------------+
    |                               |*--------------------------------*+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*               w4               *|               w2              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("toggle spawn");
    dome.open(window("w4"));
    dome.run("focus parent");
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 33x10
      w2 33,10 34x10
      w3 67,10 33x5
      w4 67,15 33x5
      container 67,10 33x10 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*               w3              *
    |                               ||                                |*                               *
    |               w1              ||               w2               |*-------------------------------*
    |                               ||                                |*-------------------------------*
    |                               ||                                |*                               *
    |                               ||                                |*               w4              *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
    dome.run("move up");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x7
      w1 0,13 50x7
      w2 50,13 50x7
      w3 0,7 50x6
      w4 50,7 50x6
      container 0,7 100x6 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                ||                                                *
    *                       w3                       ||                       w4                       *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w1                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn toggle_direction_flips_topmost_split_below_tabbed_ancestor() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 50x10
      w2 50,10 50x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    let w2 = dome.open(window("w2"));
    dome.run("toggle layout");
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("toggle spawn");
    dome.open(window("w4"));
    dome.run("toggle layout");
    dome.run("toggle spawn");
    dome.open(window("w5"));
    dome.focus(w2);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 25x18 highlighted spawn=right
      w4 75,4 13x16
      w5 88,4 12x16
      container 50,0 50x20 tabbed active=1 titles=[w1, Container]
      container 75,2 25x18 tabbed active=1 titles=[w3, Container]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w1            |     [Container]       |
    |                                                |*************************+-----------------------+
    |                                                |*                       *|    w3     [Container] |
    |                                                |*                       *+-----------++----------+
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                       w0                       |*                       *|           ||          |
    |                                                |*           w2          *|           ||          |
    |                                                |*                       *|     w4    ||    w5    |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    |                                                |*                       *|           ||          |
    +------------------------------------------------+*************************+-----------++----------+
    ");
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x9 highlighted spawn=right
      w4 50,13 25x7
      w5 75,13 25x7
      container 50,0 50x20 tabbed active=1 titles=[w1, Container]
      container 50,11 50x9 tabbed active=1 titles=[w3, Container]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w1            |     [Container]       |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||          w3            |     [Container]       |
    |                                                |+-----------------------++-----------------------+
    |                                                ||                       ||                       |
    |                                                ||                       ||                       |
    |                                                ||           w4          ||           w5          |
    |                                                ||                       ||                       |
    |                                                ||                       ||                       |
    +------------------------------------------------++-----------------------++-----------------------+
    ");
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus parent");
    dome.run("toggle direction");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *--------------------------------------------------------------------------------------------------*
    *--------------------------------------------------------------------------------------------------*
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn close_window_shares_freed_space_evenly() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w2                       *
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
    dome.close(w0);
    dome.close(w2);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
    ");
}

#[test]
fn close_window_dissolves_container_left_with_one_child() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w1);
    dome.run("toggle spawn");
    let w3 = dome.open(window("w3"));
    dome.close(w3);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x20 highlighted spawn=bottom
      w2 67,0 33x20
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w0              |*               w1               *|               w2              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");

    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    let w1 = dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w2                       *
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
fn close_window_promoting_container_keeps_directions_alternating() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x10
      w3 50,10 50x10 highlighted spawn=right
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,2 50x18
      w3 50,2 50x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, Container]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                  [Container]                   |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                       w3                       *
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

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("toggle layout");
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w3 50,2 50x18 highlighted spawn=right
      container 50,0 50x20 tabbed active=1 titles=[w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w2            |        [w3]           |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                       w3                       *
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
fn close_focused_window_focuses_most_recently_focused_survivor() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.open(window("w3"));
    dome.focus(w0);
    dome.focus(w2);
    dome.close(w2);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20 highlighted spawn=right
      w1 33,0 34x20
      w3 67,0 33x20
    *********************************+--------------------------------++-------------------------------+
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *               w0              *|               w1               ||               w3              |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *                               *|                                ||                               |
    *********************************+--------------------------------++-------------------------------+
    ");
}

#[test]
fn close_focused_window_activates_remembered_survivors_ancestor_tabs() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree",
            tree = { split = "tabbed", children = {
                { split = "tabbed", children = {
                    { title = "w0" }, { title = "w1" }, { title = "w2" },
                } },
                { split = "tabbed", children = {
                    { title = "w3" }, { title = "w4" }, { title = "w5" },
                } },
            } },
        } } }"#,
    );
    dome.open(window("w0"));
    let remembered = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.open(window("w3"));
    let closing = dome.open(window("w4"));
    dome.open(window("w5"));
    dome.focus(remembered);
    dome.focus(closing);
    dome.close(closing);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,4 100x16 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[Container, Container]
      container 0,2 100x18 tabbed active=1 titles=[w0, w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |                   [Container]                   |                   Container                    |
    +--------------------------------------------------------------------------------------------------+
    |              w0                |            [w1]               |              w2                 |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
}

#[test]
fn close_child_of_highlighted_container_highlights_survivor() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x20 highlighted spawn=bottom
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w2                       *
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
fn move_container_to_empty_workspace_keeps_its_windows_together() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w1 0,0 100x20 highlighted spawn=bottom
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
    ");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x20
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w0                       ||                       w2                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
}

#[test]
fn move_container_to_workspace_lands_right_of_target_focus() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus workspace 1");
    dome.open(window("w3"));
    dome.run("focus workspace 0");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x10
      w2 50,10 50x10
      w3 0,0 50x20
      container 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w0                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w3                       |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_container_to_workspace_lands_below_target_focus() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus workspace 1");
    dome.open(window("w3"));
    dome.run("toggle spawn");
    dome.run("focus workspace 0");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,10 50x10
      w2 50,10 50x10
      w3 0,0 100x10
      container 0,10 100x10 highlighted spawn=bottom
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w3                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w0                       ||                       w2                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
}

#[test]
fn move_container_to_workspace_lands_inside_active_target_tab() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus workspace 1");
    dome.open(window("w3"));
    dome.open(window("w4"));
    dome.run("toggle layout");
    dome.run("focus workspace 0");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,2 50x9
      w2 50,11 50x9
      w4 0,2 50x18
      container 0,0 100x20 tabbed active=1 titles=[w3, Container]
      container 50,2 50x18 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                       w3                        |                  [Container]                   |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w0                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w4                       |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_container_to_workspace_lands_beside_retained_tiling_focus() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus workspace 1");
    let w3 = dome.open(window("w3"));
    dome.open(window("w4"));
    let w5 = dome.open(window("w5"));
    dome.focus(w3);
    dome.focus(w5);
    dome.run("toggle float");
    dome.run("focus workspace 0");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 33,0 34x10
      w2 33,10 34x10
      w3 0,0 33x20
      w4 67,0 33x20
      w5 67,0 33x20 float
      container 33,0 34x20 highlighted spawn=right
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*               w0               *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |               w3              |*--------------------------------*|               w5              |
    |                               |*--------------------------------*|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*               w2               *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");
}

#[test]
fn move_window_to_workspace_with_automatic_tiling_splits_with_next_window() {
    let mut dome = TestDome::new("return { partition_tree = { automatic_tiling = true } }");
    dome.open(window("w17"));
    dome.run("toggle spawn");
    dome.open(window("w18"));
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    dome.open(window("w19"));
    assert_snapshot!(dome.snapshot(), @"
    focused w19 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w18 0,0 50x20
      w19 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w18                      |*                       w19                      *
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
fn move_preferred_root_to_workspace_releases_container_for_free_source_leaves() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree",
            tree = { split = "horizontal", children = {
                { title = "a" }, { title = "b" }, { title = "c" }, { title = "d" },
            } },
        } } }"#,
    );
    dome.open(window("a"));
    dome.open(window("b"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.open(window("d"));
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      c 0,0 50x20 highlighted spawn=right
      d 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        c                       *|                        d                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_preferred_nested_container_to_workspace_releases_it_for_free_source_leaves() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree",
            tree = { split = "horizontal", children = {
                { split = "vertical", children = {
                    { title = "a" }, { title = "b" }, { title = "c" }, { title = "d" },
                } },
                { title = "e" },
            } },
        } } }"#,
    );
    let a = dome.open(window("a"));
    dome.open(window("b"));
    dome.open(window("e"));
    dome.focus(a);
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.open(window("d"));
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      c 0,0 50x10 highlighted spawn=bottom
      d 0,10 50x10
      e 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        c                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************|                        e                       |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        d                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn move_nested_container_to_workspace_keeps_directions_alternating() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("focus parent");
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,0 50x10
      w3 50,10 50x10
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                       w2                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w0                       |+------------------------------------------------*
    *                                                |+------------------------------------------------*
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                       w3                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
}

#[test]
fn move_scrolling_column_to_tree_workspace_keeps_its_windows_together() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["1"] = { layout = "scrolling" } } }"#,
    );
    dome.open(window("w0"));
    dome.run("focus workspace 1");
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("move workspace 0");
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10
      container 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    dome.run("focus workspace 1");
    dome.open(window("w3"));
    dome.run("focus parent");
    dome.run("move workspace 0");
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 33x20
      w1 33,0 34x10
      w2 33,10 34x10
      w3 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w1               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              |+--------------------------------+*               w3              *
    |                               |+--------------------------------+*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||               w2               |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
}

#[test]
fn open_matched_window_takes_its_layout_position() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            { { title = "a" }, { title = "b" } },
            { title = "c" },
            { { title = "d" }, { title = "e" } },
        },
    } } }"#,
    );
    dome.open(window("d"));
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x20 highlighted spawn=right
      d 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                        d                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 33x20
      c 33,0 34x20 highlighted spawn=right
      d 67,0 33x20
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                a              |*                c               *|                d              |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    +-------------------------------+**********************************+-------------------------------+
    ");
    dome.open(window("b"));
    dome.open(window("e"));
    assert_snapshot!(dome.snapshot(), @"
    focused e on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 33x10
      b 0,10 33x10
      c 33,0 34x20
      d 67,0 33x10
      e 67,10 33x10 highlighted spawn=right
    +-------------------------------++--------------------------------++-------------------------------+
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                a              ||                                ||                d              |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    +-------------------------------+|                c               |+-------------------------------+
    +-------------------------------+|                                |*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                b              ||                                |*                e              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree", tree = {
                { title = "a" }, { title = "b" }, { title = "c" },
            },
        } } }"#,
    );
    let first = dome.open(window("a"));
    dome.open(window("c"));
    dome.focus(first);
    dome.open(window("unslotted"));
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 25x20
      b 50,0 25x20 highlighted spawn=right
      c 75,0 25x20
      unslotted 25,0 25x20
    +-----------------------++-----------------------+*************************+-----------------------+
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |            a          ||        unslotted      |*            b          *|            c          |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    |                       ||                       |*                       *|                       |
    +-----------------------++-----------------------+*************************+-----------------------+
    ");

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree", tree = {
                { { { title = "a" }, { title = "b" } }, { title = "c" } },
                { title = "d" },
            },
        } } }"#,
    );
    dome.open(window("a"));
    dome.open(window("d"));
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      b 0,10 50x10 highlighted spawn=right
      d 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        a                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                        d                       |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        b                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn open_matched_window_after_manual_move_joins_nearest_held_leaf() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            { { title = "a" }, { title = "b" } },
            { { title = "c" }, { title = "d" } },
        },
    } } }"#,
    );
    dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.open(window("c"));
    dome.run("move down");
    dome.close(b);
    dome.open(window("d"));
    assert_snapshot!(dome.snapshot(), @"
    focused d on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      c 0,10 100x10
      d 50,0 50x10 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        a                       |*                        d                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 c                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            { { title = "a" }, { title = "b" } },
            { title = "c" }, { title = "d" },
        },
    } } }"#,
    );
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.open(window("x"));
    let y = dome.open(window("y"));
    dome.open(window("c"));
    dome.run("move down");
    dome.close(a);
    dome.close(b);
    dome.close(y);
    dome.open(window("d"));
    assert_snapshot!(dome.snapshot(), @"
    focused d on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      c 0,10 50x10
      d 50,10 50x10 highlighted spawn=right
      x 0,0 100x10
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 x                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        c                       |*                        d                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn open_window_without_free_matching_leaf_follows_spawn_direction() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = { title = "a" },
    } } }"#,
    );
    dome.open(window("a").label("a1"));
    dome.open(window("a").label("a2"));
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a1 0,0 33x20
      a2 33,0 34x20
      b 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               a1              ||               a2               |*                b              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            split = "vertical", children = { { title = "a" } },
        },
    } } }"#,
    );
    dome.open(window("a").label("a1"));
    dome.open(window("a").label("a2"));
    assert_snapshot!(dome.snapshot(), @"
    focused a2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a1 0,0 50x20
      a2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       a1                       |*                       a2                       *
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
fn open_matched_window_skips_holder_no_longer_tiling_on_its_leaf() {
    let setup = || {
        let mut dome = TestDome::with_layout(
            "return {}",
            r#"return { primary = { ["0"] = {
            layout = "partition_tree", tree = { { title = "a" }, { title = "b" }, { title = "c" } },
        } } }"#,
        );
        let b = dome.open(window("b"));
        dome.open(window("c"));
        dome.focus(b);
        dome
    };
    let mut dome = setup();
    dome.run("move workspace 1");
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      a 0,0 50x20 highlighted spawn=right
      c 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                        c                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");

    let mut dome = setup();
    dome.run("toggle float");
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x20 highlighted spawn=right
      b 0,0 50x20 float
      c 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        b                       ||                        c                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn save_layout_writes_live_tree_with_window_titles() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            split = "tabbed", children = { { title = "term" }, { title = "editor" } },
        },
    } } }"#,
    );
    let term = dome.open(window("term"));
    dome.set_title(term, "shell");
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("toggle layout");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
          tree = { split = "horizontal", children = {
            { title = "shell" },
            { split = "vertical", children = {
              { title = "w1" },
              { split = "tabbed", children = {
                { title = "w2" },
                { title = "w3" },
              } },
            } },
          } },
        },
      },
    }
    "#);
}

#[test]
fn apply_layout_places_matched_windows_and_splits_unmatched_windows() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("a"));
    dome.open(window("b"));
    dome.open(window("c"));
    dome.apply_layout(
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            split = "vertical", children = { { title = "c" }, { title = "a" }, { title = "b" } },
        },
    } } }"#,
    );
    let snapshot = dome.snapshot();
    for placement in ["  c 0,0 100x7", "  a 0,7 100x6", "  b 0,13 100x7"] {
        assert!(snapshot.contains(placement), "{snapshot}");
    }
    assert!(!snapshot.contains(" float"), "{snapshot}");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.apply_layout("return {}");
    let snapshot = dome.snapshot();
    let mut rectangles = Vec::new();
    for title in ["w0", "w1", "w2"] {
        let line = snapshot
            .lines()
            .find(|line| line.starts_with(&format!("  {title} ")))
            .unwrap();
        let fields: Vec<_> = line.split_whitespace().collect();
        rectangles.push((fields[1], fields[2]));
    }
    rectangles.sort_unstable();
    assert_eq!(
        rectangles,
        [("0,0", "33x20"), ("33,0", "34x20"), ("67,0", "33x20")],
        "{snapshot}"
    );
    assert!(!snapshot.contains(" float"), "{snapshot}");

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus parent");
    dome.apply_layout("return {}");
    let snapshot = dome.snapshot();
    let mut positions = Vec::new();
    for title in ["w0", "w1"] {
        let line = snapshot
            .lines()
            .find(|line| line.starts_with(&format!("  {title} ")))
            .unwrap();
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields[2], "50x20", "{snapshot}");
        positions.push(fields[1]);
    }
    positions.sort_unstable();
    assert_eq!(positions, ["0,0", "50,0"], "{snapshot}");
    assert!(!snapshot.contains(" float"), "{snapshot}");
}

#[test]
fn apply_layout_switching_master_to_tree_places_migrated_windows_in_declared_slots() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = { layout = "master" } } }"#,
    );
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.apply_layout(
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", tree = {
            split = "vertical", children = { { title = "w0" }, { title = "w1" }, { title = "w2" } },
        },
    } } }"#,
    );
    let snapshot = dome.snapshot();
    for placement in ["  w0 0,0 100x7", "  w1 0,7 100x6", "  w2 0,13 100x7"] {
        assert!(snapshot.contains(placement), "{snapshot}");
    }
    assert!(!snapshot.contains(" float"), "{snapshot}");

    dome.close(w0);
    let snapshot = dome.snapshot();
    assert!(snapshot.contains("  w1 0,0 100x10"), "{snapshot}");
    assert!(snapshot.contains("  w2 0,10 100x10"), "{snapshot}");
    assert!(!snapshot.contains("  w0 "), "{snapshot}");
    assert!(!snapshot.contains(" float"), "{snapshot}");

    dome.close(w1);
    let snapshot = dome.snapshot();
    assert!(snapshot.contains("  w2 0,0 100x20"), "{snapshot}");
    assert!(!snapshot.contains("  w1 "), "{snapshot}");
    assert!(!snapshot.contains(" float"), "{snapshot}");
}

#[test]
fn open_on_tree_layout_places_windows_by_declared_split() {
    let setup = |tree: &str| {
        let mut dome = TestDome::with_layout(
            "return {}",
            &format!(
                r#"return {{ primary = {{ ["0"] = {{
            layout = "partition_tree", {tree}
        }} }} }}"#
            ),
        );
        dome.open(window("b"));
        dome.open(window("a"));
        dome
    };
    let mut dome = setup(r#"tree = { { title = "a" }, { title = "b" } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x20 highlighted spawn=right
      b 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                        b                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    let mut dome =
        setup(r#"tree = { split = "vertical", children = { { title = "a" }, { title = "b" } } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 100x10 highlighted spawn=bottom
      b 0,10 100x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                 a                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 b                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
    let mut dome =
        setup(r#"tree = { split = "tabbed", children = { { title = "a" }, { title = "b" } } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=0 titles=[a, b]
    +--------------------------------------------------------------------------------------------------+
    |                       [a]                       |                       b                        |
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                 a                                                *
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
    let mut dome =
        setup(r#"tree = { split = "diagonal", children = { { title = "a" }, { title = "b" } } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x20 highlighted spawn=right
      b 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                        b                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    let mut dome = setup("");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 50x20 highlighted spawn=right
      b 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        b                       |*                        a                       *
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
    insta::allow_duplicates! {
        for malformed in ["42", r#"{ split = "vertical" }"#] {
            let mut dome = setup(&format!(
                r#"tree = {{ {malformed}, {{ title = "a" }}, {{ title = "b" }} }}"#
            ));
            assert_snapshot!(dome.snapshot(), @"
        focused a on primary
        workspaces: 0 on primary (focused)
        primary 0,0 100x20 border 1
          a 0,0 50x20 highlighted spawn=right
          b 50,0 50x20
        **************************************************+------------------------------------------------+
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                        a                       *|                        b                       |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        *                                                *|                                                |
        **************************************************+------------------------------------------------+
        ");
        }
    }
}

#[test]
fn open_window_matching_fullscreen_rule_covers_monitor() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    dome.open(window("fs0"));
    assert_snapshot!(dome.snapshot(), @"
    focused fs0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs0                                               |
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
}

#[test]
fn open_window_matching_float_rule_floats_at_reported_frame() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("f0").frame([10, 3, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
              ****************************************                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                  f0                  *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *                                                  
              ****************************************
    ");

    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    dome.open(window("f0").frame([10, 3, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |         ****************************************                                                 |
    |         *                                      *                                                 |
    |         *                                      *                                                 |
    |         *                                      *                                                 |
    |         *                  f0                  *                                                 |
    |         *                                      *                                                 |
    |         *                                      *0                                                |
    |         *                                      *                                                 |
    |         *                                      *                                                 |
    |         ****************************************                                                 |
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
fn open_overlapping_mode_matches_consumes_each_eligible_slot_before_tiling_fallback() {
    for (prefix, count) in [("both", 2), ("all", 3)] {
        for open_fallback in [false, true] {
            let mut dome = TestDome::with_layout(
                "return {}",
                r#"return { primary = { ["0"] = {
                    layout = "partition_tree", tree = { { title = "/^both/" }, { title = "/^all/" } },
                    float = { { title = "/^both/" }, { title = "/^all/" } },
                    fullscreen = { { title = "/^all/" } },
                } } }"#,
            );
            let titles: Vec<_> = (0..count).map(|index| format!("{prefix}{index}")).collect();
            let windows: Vec<_> = titles
                .iter()
                .map(|title| dome.open(window(title).frame([10, 3, 40, 10])))
                .collect();
            let layouts = dome.capture_live_layouts();
            let workspace = layouts.workspace("primary", "0").unwrap();
            assert_eq!(workspace.float.len(), 1);
            assert_eq!(workspace.fullscreen.len(), count - 2);
            let floating = workspace.float[0].title.as_deref().unwrap();
            let fullscreen = workspace
                .fullscreen
                .first()
                .map(|matcher| matcher.title.as_deref().unwrap());
            let tiled: Vec<_> = titles
                .iter()
                .filter(|title| title.as_str() != floating && Some(title.as_str()) != fullscreen)
                .collect();
            assert_eq!(tiled.len(), 1);
            let mut placed = vec![tiled[0].as_str(), floating];
            placed.extend(fullscreen);
            placed.sort();
            assert_eq!(placed, titles);

            let fallback = format!("{prefix}{count}");
            if open_fallback {
                dome.open(window(&fallback).frame([10, 3, 40, 10]));
                let layouts = dome.capture_live_layouts();
                let after = layouts.workspace("primary", "0").unwrap();
                assert_eq!(after.float, workspace.float);
                assert_eq!(after.fullscreen, workspace.fullscreen);
            }
            if let Some(title) = fullscreen {
                let snapshot = dome.snapshot();
                assert!(
                    snapshot
                        .lines()
                        .any(|line| line
                            == format!("primary 0,0 100x20 border 1 fullscreen {title}")),
                    "{snapshot}"
                );
                let index = titles
                    .iter()
                    .position(|candidate| candidate == title)
                    .unwrap();
                dome.close(windows[index]);
            }
            let snapshot = dome.snapshot();
            let placements: Vec<Vec<_>> = snapshot
                .lines()
                .filter_map(|line| line.strip_prefix("  "))
                .map(|line| line.split_whitespace().collect())
                .collect();
            assert_eq!(
                placements.len(),
                if open_fallback { 3 } else { 2 },
                "{snapshot}"
            );
            let floating_placement = placements
                .iter()
                .find(|fields| fields[0] == floating)
                .unwrap();
            assert_eq!(
                &floating_placement[..4],
                &[floating, "10,3", "40x10", "float"],
                "{snapshot}"
            );
            let mut rectangles = Vec::new();
            for title in
                std::iter::once(tiled[0].as_str()).chain(open_fallback.then_some(fallback.as_str()))
            {
                let placement = placements.iter().find(|fields| fields[0] == title).unwrap();
                assert!(!placement.contains(&"float"), "{snapshot}");
                rectangles.push((placement[1], placement[2]));
            }
            rectangles.sort();
            let expected = if open_fallback {
                vec![("0,0", "50x20"), ("50,0", "50x20")]
            } else {
                vec![("0,0", "100x20")]
            };
            assert_eq!(rectangles, expected, "{snapshot}");
        }
    }
}

#[test]
fn save_layout_writes_floats_and_fullscreens_from_live_titles() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
        layout = "partition_tree", float = { { title = "f0" } }, fullscreen = { { title = "fs0" } },
    } } }"#,
    );
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.focus(w0);
    dome.run("toggle float");
    dome.run("toggle float");
    dome.focus(w2);
    dome.run("toggle float");
    dome.focus(w1);
    dome.run("toggle fullscreen");
    let f0 = dome.open(window("f0").frame([10, 3, 40, 10]));
    let fs0 = dome.open(window("fs0"));
    dome.set_title(f0, "notes");
    dome.set_title(fs0, "video");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
          tree = { title = "w0" },
          float = {
            { title = "w2" },
            { title = "notes" },
          },
          fullscreen = {
            { title = "w1" },
            { title = "video" },
          },
        },
      },
    }
    "#);
}

#[test]
fn toggle_float_floats_window_at_its_tile_then_tiles_it_back() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
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
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.open(window("w3"));
    let nested = dome.snapshot();
    assert_snapshot!(nested, @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted spawn=bottom
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10
      w3 50,13 50x7 float highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.run("toggle float");
    assert_eq!(dome.snapshot(), nested);
}

#[test]
fn move_float_moves_float_keeping_order_and_focus() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    let first = dome.open(window("f0").frame([10, 3, 40, 10]));
    dome.open(window("f1").frame([30, 6, 40, 10]));
    dome.move_float(first, [50, 4, 30, 8], dome.primary_monitor());
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 49,3 32x10 float
      f1 30,6 40x10 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
                                                     +------------------------------+                   
                                                     |                              |                   
                                                     |                              |                   
                                  ****************************************          |                   
                                  *                                      *          |                   
                                  *                                      *          |                   
                                  *                                      *          |                   
                                  *                  f1                  *          |                   
                                  *                                      *          |                   
                                  *                                      *----------+                   
                                  *                                      *                              
                                  *                                      *                              
                                  ****************************************
    ");
}

#[test]
fn open_float_within_twice_border_size_is_not_placed() {
    let mut dome = TestDome::with_layout("return { border_size = 2 }", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("f0").frame([10, 3, 4, 4]));
    dome.open(window("f1").frame([30, 6, 5, 5]));
    let placed = dome.snapshot();
    assert_snapshot!(placed, @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 2
      f1 30,6 5x5 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                  *****                                                                 
                                  *   *                                                                 
                                  * f1*                                                                 
                                  *   *                                                                 
                                  *****
    ");

    for exhausted_frame in [[10, 3, 4, 10], [10, 3, 10, 4]] {
        let mut dome =
            TestDome::with_layout("return { border_size = 2 }", FLOAT_AND_FULLSCREEN_RULES);
        dome.open(window("f0").frame(exhausted_frame));
        dome.open(window("f1").frame([30, 6, 5, 5]));
        assert_eq!(dome.snapshot(), placed, "{exhausted_frame:?}");
    }
}

#[test]
fn toggle_float_or_fullscreen_with_container_highlighted_changes_nothing() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus parent");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w0                       ||                       w1                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
    for verb in ["toggle float", "toggle fullscreen"] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }
}

#[test]
fn click_tab_while_float_focused_focuses_tab() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle layout");
    dome.run("toggle float");
    dome.click_tab(dome.tab_container(&["w0", "w1"]), 0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted spawn=right
      w2 0,2 100x18 float
      container 0,0 100x20 tabbed active=0 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                      [w0]                       |                      w1                        |
    +--------------------------------------------------------------------------------------------------+
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
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn focus_moves_highlight_between_tiling_and_float() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    let first = dome.open(window("w0"));
    let second = dome.open(window("w1"));
    let float = dome.open(window("f0").frame([10, 3, 40, 10]));
    dome.focus(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float
      w0 0,0 50x20 highlighted spawn=right
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *         +--------------------------------------+|                                                |
    *         |                                      ||                                                |
    *         |                                      ||                                                |
    *         |                                      ||                                                |
    *         |                  f0                  ||                                                |
    *         |                                      ||                                                |
    *         |                                      ||                       w1                       |
    *         |                                      ||                                                |
    *         |                                      ||                                                |
    *         +--------------------------------------+|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.focus(float);
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
      w0 0,0 50x20
      w1 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |         ****************************************|                                                |
    |         *                                      *|                                                |
    |         *                                      *|                                                |
    |         *                                      *|                                                |
    |         *                  f0                  *|                                                |
    |         *                                      *|                                                |
    |         *                                      *|                       w1                       |
    |         *                                      *|                                                |
    |         *                                      *|                                                |
    |         ****************************************|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.focus(second);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |         +--------------------------------------+*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                  f0                  |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                       w1                       *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         +--------------------------------------+*                                                *
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
fn focus_on_float_raises_it_above_other_floats() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    let first = dome.open(window("f0").frame([10, 3, 40, 10]));
    dome.open(window("f1").frame([30, 6, 40, 10]));
    dome.focus(first);
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
      f1 30,6 40x10 float
                                                                                                        
                                                                                                        
                                                                                                        
              ****************************************                                                  
              *                                      *                                                  
              *                                      *                                                  
              *                                      *-------------------+                              
              *                  f0                  *                   |                              
              *                                      *                   |                              
              *                                      *                   |                              
              *                                      *1                  |                              
              *                                      *                   |                              
              ****************************************                   |                              
                                  |                                      |                              
                                  |                                      |                              
                                  +--------------------------------------+
    ");
}

#[test]
fn close_focused_float_focuses_next_float_then_tiling() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let first = dome.open(window("w1"));
    let second = dome.open(window("w2"));
    dome.focus(first);
    dome.run("toggle float");
    dome.focus(second);
    dome.run("toggle float");
    dome.close(second);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
      w1 33,0 34x20 float highlighted
    +--------------------------------**********************************--------------------------------+
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *               w1               *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    |                                *                                *                                |
    +--------------------------------**********************************--------------------------------+
    ");
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
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

    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    let retained = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(retained);
    let tiling = dome.snapshot();
    let first = dome.open(window("f0").frame([10, 3, 40, 10]));
    let second = dome.open(window("f1").frame([30, 6, 40, 10]));
    dome.close(second);
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float highlighted
      w0 0,0 33x20
      w1 33,0 34x20
      w2 67,0 33x20
    +-------------------------------++--------------------------------++-------------------------------+
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |         ****************************************                ||                               |
    |         *                                      *                ||                               |
    |         *                                      *                ||                               |
    |         *                                      *                ||                               |
    |         *                  f0                  *                ||                               |
    |         *                                      *                ||                               |
    |         *                                      *1               ||               w2              |
    |         *                                      *                ||                               |
    |         *                                      *                ||                               |
    |         ****************************************                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    +-------------------------------++--------------------------------++-------------------------------+
    ");
    dome.close(first);
    assert_eq!(dome.snapshot(), tiling);
}

#[test]
fn close_or_minimize_last_tiling_window_focuses_float() {
    let setup = || {
        let mut dome = TestDome::new("return {}");
        let tiling = dome.open(window("w0"));
        dome.open(window("w1"));
        dome.run("toggle float");
        (dome, tiling)
    };
    let (mut dome, tiling) = setup();
    dome.focus(tiling);
    dome.close(tiling);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 50,0 50x20 float highlighted
                                                      **************************************************
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                       w1                       *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      **************************************************
    ");
    let (mut dome, tiling) = setup();
    dome.focus(tiling);
    dome.minimize(tiling);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    minimized: w0 "w0"
    primary 0,0 100x20 border 1
      w1 50,0 50x20 float highlighted
                                                      **************************************************
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                       w1                       *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      **************************************************
    "#);
    let (mut dome, tiling) = setup();
    dome.minimize(tiling);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    minimized: w0 "w0"
    primary 0,0 100x20 border 1
      w1 50,0 50x20 float highlighted
                                                      **************************************************
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                       w1                       *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      *                                                *
                                                      **************************************************
    "#);
}

#[test]
fn move_container_to_workspace_with_focused_float_takes_focus() {
    let mut dome = TestDome::new("return {}");
    dome.run("focus workspace 1");
    dome.open(window("w0"));
    dome.run("toggle float");
    dome.run("focus workspace 0");
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 float
      w1 0,0 50x20
      w2 50,0 50x20
      container 0,0 100x20 highlighted spawn=right
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
}

#[test]
fn run_tiling_verb_under_fullscreen_changes_nothing() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle layout");
    let tiling = dome.snapshot();
    assert_snapshot!(tiling, @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x18 highlighted spawn=bottom
      container 50,0 50x20 tabbed active=1 titles=[w1, w2]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w1            |        [w2]           |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                       w2                       *
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
    let tabs = dome.tab_container(&["w1", "w2"]);
    let fullscreen = dome.open(window("fs0"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused fs0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs0                                               |
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
    for verb in [
        "focus left",
        "move left",
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "toggle float",
        "focus tab next",
    ] {
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }
    dome.click_tab(tabs, 0);
    assert_eq!(dome.snapshot(), before);
    dome.close(fullscreen);
    assert_eq!(dome.snapshot(), tiling);
}

#[test]
fn set_fullscreen_on_two_windows_shows_topmost_until_other_focused() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let first = dome.open(window("w1"));
    let second = dome.open(window("w2"));
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    dome.focus(first);
    assert_snapshot!(dome.snapshot(), @"
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
}

#[test]
fn close_or_toggle_off_topmost_fullscreen_reveals_next_fullscreen() {
    let setup = || {
        let mut dome = TestDome::new("return {}");
        dome.open(window("w0"));
        let first = dome.open(window("w1"));
        let second = dome.open(window("w2"));
        dome.set_fullscreen(first, WindowRestrictions::None);
        dome.set_fullscreen(second, WindowRestrictions::None);
        (dome, second)
    };
    let (mut dome, _) = setup();
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
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
    let (mut dome, second) = setup();
    dome.close(second);
    assert_snapshot!(dome.snapshot(), @"
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
}

#[test]
fn close_or_unset_covered_window_keeps_topmost_focused() {
    let setup = || {
        let mut dome = TestDome::new("return {}");
        dome.open(window("w0"));
        let first = dome.open(window("w1"));
        let second = dome.open(window("w2"));
        dome.set_fullscreen(first, WindowRestrictions::None);
        dome.set_fullscreen(second, WindowRestrictions::None);
        (dome, first, second)
    };
    let (mut dome, first, second) = setup();
    dome.unset_fullscreen(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    dome.close(second);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    let (mut dome, first, _) = setup();
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    let first = dome.open(window("f0").frame([10, 3, 40, 10]));
    dome.open(window("f1").frame([30, 6, 40, 10]));
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 30,6 40x10 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                             ****************************************                             |
    |                             *                                      *                             |
    |                             *                                      *                             |
    |                             *                                      *                             |
    |                             *                  f1                  *                             |
    |                             *                                      *                             |
    |                             *                                      *                             |
    |                             *                                      *                             |
    |                             *                                      *                             |
    |                             ****************************************                             |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn close_minimize_or_move_last_fullscreen_without_tiling_focuses_float() {
    let setup = || {
        let mut dome = TestDome::new("return {}");
        dome.open(window("w0"));
        dome.run("toggle float");
        let fullscreen = dome.open(window("w1"));
        dome.run("toggle fullscreen");
        (dome, fullscreen)
    };
    let (mut dome, fullscreen) = setup();
    dome.close(fullscreen);
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
    let (mut dome, fullscreen) = setup();
    dome.minimize(fullscreen);
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    minimized: w1 "w1"
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
    "#);
    let (mut dome, _) = setup();
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 on primary
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
fn close_only_fullscreen_focuses_tiling_over_float() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle float");
    let fullscreen = dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
      w1 50,0 50x20 float
    **************************************************+------------------------------------------------+
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                w|                       w1                       |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn toggle_fullscreen_off_with_float_selected_keeps_focus_on_window() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let float = dome.open(window("w1"));
    dome.run("toggle float");
    dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.focus(float);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 float
      w2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn close_app_fullscreen_returns_focus_to_float_focused_before() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let fullscreen = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle float");
    dome.set_fullscreen(fullscreen, WindowRestrictions::ProtectFullscreen);
    assert_snapshot!(dome.snapshot(), @"
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
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    ");
}

#[test]
fn set_fullscreen_again_on_fullscreen_window_changes_nothing_visible() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let first = dome.open(window("w1"));
    let second = dome.open(window("w2"));
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    dome.set_fullscreen(first, WindowRestrictions::ProtectFullscreen);
    assert_eq!(dome.snapshot(), before);
    dome.set_fullscreen(second, WindowRestrictions::ProtectFullscreen);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn toggle_fullscreen_twice_restores_tile_and_tiles_former_float() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    let tiling = dome.open(window("w1"));
    let float = dome.open(window("f0").frame([10, 3, 40, 10]));
    dome.focus(tiling);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
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
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |         +--------------------------------------+*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                  f0                  |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                       w1                       *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         +--------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.focus(float);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                f0                                                |
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
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 67,0 33x20 highlighted spawn=right
      w0 0,0 33x20
      w1 33,0 34x20
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w0              ||               w1               |*               f0              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
}

#[test]
fn move_fullscreen_to_workspace_with_focused_float_takes_focus() {
    let mut dome = TestDome::new("return {}");
    dome.run("focus workspace 1");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle float");
    dome.run("focus workspace 0");
    let fullscreen = dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 1 on primary (focused)
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
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
      w1 50,0 50x20 float
    **************************************************+------------------------------------------------+
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                w|                       w1                       |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_container_to_workspace_under_fullscreen_focuses_it_after_close() {
    let mut dome = TestDome::new("return {}");
    dome.run("focus workspace 1");
    let fullscreen = dome.open(window("w0"));
    dome.run("toggle fullscreen");
    dome.run("focus workspace 0");
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 1 on primary (focused)
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
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x20
      w2 50,0 50x20
      container 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                       w1                       ||                       w2                       *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    *                                                ||                                                *
    ****************************************************************************************************
    ");
}

#[test]
fn focus_window_under_fullscreen_focuses_it_after_close() {
    let mut dome = TestDome::new("return {}");
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    let fullscreen = dome.open(window("w2"));
    dome.run("toggle fullscreen");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    dome.focus(first);
    assert_eq!(dome.snapshot(), before);
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted spawn=right
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                       w1                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn unminimize_fullscreen_window_clears_float_focus_underneath() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let fullscreen = dome.open(window("w1"));
    dome.run("toggle fullscreen");
    dome.minimize(fullscreen);
    dome.open(window("w2"));
    dome.run("toggle float");
    dome.unminimize(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
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
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
      w2 50,0 50x20 float
    **************************************************+------------------------------------------------+
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                w|                       w2                       |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn apply_layout_retains_windows_in_declared_display_modes() {
    let mut dome = TestDome::new("return {}");
    let tiling = dome.open(window("w0"));
    let first = dome.open(window("w1"));
    let second = dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("toggle float");
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
    dome.focus(tiling);
    dome.apply_layout(
        r#"return { primary = { ["0"] = {
        layout = "partition_tree",
        float = { { title = "w3" } },
        fullscreen = { { title = "w1" }, { title = "w2" } },
    } } }"#,
    );
    let snapshot = dome.snapshot();
    let (visible, covered, covered_label) = if snapshot.contains("border 1 fullscreen w1") {
        (first, second, "w2")
    } else {
        assert!(snapshot.contains("border 1 fullscreen w2"), "{snapshot}");
        (second, first, "w1")
    };
    dome.close(visible);
    let snapshot = dome.snapshot();
    assert!(
        snapshot.contains(&format!("border 1 fullscreen {covered_label}")),
        "{snapshot}"
    );
    dome.close(covered);
    let snapshot = dome.snapshot();
    assert!(snapshot.contains("  w3 75,0 25x20 float"), "{snapshot}");
    let tile = snapshot
        .lines()
        .find(|line| line.starts_with("  w0 "))
        .expect("retained tile");
    assert!(tile.starts_with("  w0 0,0 100x20"), "{snapshot}");
    assert!(!tile.contains(" float"), "{snapshot}");
}

#[test]
fn open_window_under_fullscreen_keeps_fullscreen_focused() {
    let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
    dome.open(window("w0"));
    dome.run("toggle fullscreen");
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
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
    dome.open(window("f0").frame([10, 3, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
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
}

#[test]
fn move_float_on_window_no_longer_floating_changes_nothing() {
    let setup = || {
        let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
        dome.open(window("w0"));
        let float = dome.open(window("f0").frame([10, 3, 40, 10]));
        (dome, float)
    };
    let (mut dome, float) = setup();
    dome.run("toggle float");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 50,0 50x20 highlighted spawn=right
      w0 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       f0                       *
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
    dome.move_float(float, [50, 4, 30, 8], dome.primary_monitor());
    assert_eq!(dome.snapshot(), before);
    let (mut dome, float) = setup();
    dome.set_fullscreen(float, WindowRestrictions::ProtectFullscreen);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                f0                                                |
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
    dome.move_float(float, [50, 4, 30, 8], dome.primary_monitor());
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn set_fullscreen_from_tiling_or_float_covers_monitor() {
    let setup = || {
        let mut dome = TestDome::new("return {}");
        dome.open(window("w0"));
        let second = dome.open(window("w1"));
        (dome, second)
    };
    let (mut dome, second) = setup();
    dome.set_fullscreen(second, WindowRestrictions::ProtectFullscreen);
    assert_snapshot!(dome.snapshot(), @"
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
    let (mut dome, second) = setup();
    dome.run("toggle float");
    dome.set_fullscreen(second, WindowRestrictions::ProtectFullscreen);
    assert_snapshot!(dome.snapshot(), @"
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
}

#[test]
fn unset_fullscreen_tiles_window_and_ignores_window_not_fullscreen() {
    let setup = || {
        let mut dome = TestDome::with_layout("return {}", FLOAT_AND_FULLSCREEN_RULES);
        dome.open(window("w0"));
        let float = dome.open(window("f0").frame([10, 3, 40, 10]));
        (dome, float)
    };
    let (mut dome, _) = setup();
    let fullscreen = dome.open(window("w2").restricted(WindowRestrictions::ProtectFullscreen));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
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
    dome.unset_fullscreen(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float
      w0 0,0 50x20
      w2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |         +--------------------------------------+*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                  f0                  |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                       w2                       *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         +--------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    let (mut dome, float) = setup();
    let tiling = dome.open(window("w2"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,3 40x10 float
      w0 0,0 50x20
      w2 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |         +--------------------------------------+*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         |                  f0                  |*                                                *
    |         |                                      |*                                                *
    |         |                                      |*                       w2                       *
    |         |                                      |*                                                *
    |         |                                      |*                                                *
    |         +--------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.unset_fullscreen(tiling);
    assert_eq!(dome.snapshot(), before);
    dome.unset_fullscreen(float);
    assert_eq!(dome.snapshot(), before);
}

#[cfg(target_os = "windows")]
#[test]
fn update_monitor_scale_scales_tab_bar_height() {
    use crate::tests::fixtures::monitor;

    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                       w0                        |                     [w1]                       |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
    dome.update_monitor(
        dome.primary_monitor(),
        monitor("primary", [0, 0, 100, 20]).scale(2.0),
        None,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 2
      w1 0,4 100x16 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w0                        |                     [w1]                       |
    |                                                 |                                                |
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new("return { partition_tree = { tab_bar_height = 1 } }");
    dome.open(window("w0"));
    dome.run("toggle spawn");
    let middle = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(middle);
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("toggle layout");
    dome.update_monitor(
        dome.primary_monitor(),
        monitor("primary", [0, 0, 100, 20]).scale(1.5),
        None,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 2
      w0 0,0 100x7
      w2 0,13 100x7
      w3 0,8 100x5 highlighted spawn=right
      container 0,7 100x6 tabbed active=1 titles=[w1, w3]
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +-----------------------w1----------------------------------------------[w3]-----------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                w3                                                *
    *                                                                                                  *
    ****************************************************************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w2                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}
