use insta::assert_snapshot;

use crate::core::{Length, LimitObservation, LimitUpdate, WindowRestrictions};

use crate::tests::test_dome::{TestDome, container, monitor, window};

const MASTER: &str = "return { layout = 'master' }";

fn layout(entry: &str) -> String {
    format!("return {{ primary = {{ ['0'] = {{ layout = 'master', {entry} }} }} }}")
}

#[test]
fn open_first_window_fills_work_area() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted
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
fn open_second_window_splits_panes_at_master_ratio() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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

    let mut dome = TestDome::new("return { layout = 'master', master = { master_ratio = 0.7 } }");
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 70x20
      w1 70,0 30x20 highlighted
    +--------------------------------------------------------------------+******************************
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                 w0                                 |*             w1             *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*                            *
    +--------------------------------------------------------------------+******************************
    ");
}

#[test]
fn open_third_window_stacks_secondary_pane_evenly() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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
fn master_more_past_window_count_stacks_all_in_master() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    for _ in 0..4 {
        dome.run("master more");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x7
      w1 0,7 100x6
      w2 0,13 100x7 highlighted
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn grow_and_shrink_resize_focused_pane_and_collapse_at_bounds() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 45x20
      w1 45,0 55x20 highlighted
    +-------------------------------------------+*******************************************************
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                     w0                    |*                          w1                         *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    |                                           |*                                                     *
    +-------------------------------------------+*******************************************************
    ");
    dome.run("shrink");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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
    dome.focus(first);
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 55x20 highlighted
      w1 55,0 45x20
    *******************************************************+-------------------------------------------+
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                          w0                         *|                     w1                    |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *******************************************************+-------------------------------------------+
    ");
    for _ in 0..12 {
        dome.run("shrink");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 100x20
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
    for _ in 0..21 {
        dome.run("grow");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted
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
fn master_more_promotes_top_secondary_window() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 50x10
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                       w2                       *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn master_more_skips_secondary_slot_holders() {
    let mut dome = TestDome::with_layout(MASTER, &layout("secondary = { { title = 'w1' } }"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 50,0 50x20
      w2 0,10 50x10 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                       w1                       |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w2                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("secondary = { { title = 'w1' }, { title = 'w2' } }"),
    );
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    let before = dome.snapshot();
    dome.run("master more");
    assert_eq!(dome.snapshot(), before);

    let mut dome = TestDome::with_layout(MASTER, &layout("master = { { title = 'w0' } }"));
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.focus(first);
    dome.run("move right");
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,10 100x10 highlighted
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
fn master_fewer_moves_last_master_window_to_top_of_secondary() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("master more");
    dome.run("master fewer");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = 'w0' }, { title = 'w1' } }, master_count = 2"),
    );
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("master fewer");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    let before = dome.snapshot();
    dome.run("master fewer");
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn master_more_changes_only_focused_workspace() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus workspace 1");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,0 100x10
      w3 0,10 100x10 highlighted
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w2                                                |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w3                                                *
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
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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
}

#[test]
fn reload_reserved_area_change_fits_panes_to_new_work_area() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.reload("return { layout = 'master', reserved_area = function() return { top = 2 } end }");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,2 100x18 border 1
      w0 0,2 50x18
      w1 50,2 50x9
      w2 50,11 50x9 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       |+------------------------------------------------+
    |                                                |**************************************************
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
fn load_layout_master_ratio_and_count_size_panes_unless_out_of_range() {
    let mut dome = TestDome::with_layout(MASTER, &layout("master_ratio = 0.6, master_count = 2"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 60x10
      w1 0,10 60x10
      w2 60,0 40x20 highlighted
    +----------------------------------------------------------+****************************************
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                            w0                            |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    +----------------------------------------------------------+*                  w2                  *
    +----------------------------------------------------------+*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                            w1                            |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    +----------------------------------------------------------+****************************************
    ");

    let mut dome = TestDome::with_layout(MASTER, &layout("master_ratio = 1.5, master_count = 0"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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

    let mut dome = TestDome::with_layout(MASTER, &layout("master_ratio = 0"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 100x10
      w2 0,10 100x10 highlighted
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
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::with_layout(MASTER, &layout("master_ratio = 1"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
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
fn open_master_match_into_full_master_evicts_last_window_without_tiling_slot() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = 'w2' } }, master_count = 2"),
    );
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 50,0 50x20
      w2 0,10 50x10 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w0                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                       w1                       |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w2                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = 'w0' } }, master = { { title = 'w1' } }"),
    );
    dome.open(window("w0"));
    dome.run("toggle float");
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x20
      w1 0,0 50x20 highlighted
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w1                       *|                       w0                       |
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
fn open_master_match_into_master_of_slot_holders_raises_master_count() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout(
            "master = { { title = 'w0' }, { title = 'w1' } }, secondary = { { title = 'w3' } }",
        ),
    );
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 50x10
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                       w2                       *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = 'w0' }, { title = 'w1' } }"),
    );
    dome.open(window("w0"));
    dome.run("toggle float");
    dome.run("toggle float");
    dome.open(window("w1"));
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 50x10
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                       w2                       *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn open_unmatched_window_with_master_room_joins_master() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = 'w0' } }, master_count = 2"),
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted
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
fn open_matched_windows_keep_matcher_order_within_each_pane() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout(
            "master = { { title = 'w1' }, { title = 'w0' } }, secondary = { { title = 'w3' }, { title = 'w2' } }, master_count = 2",
        ),
    );
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,10 50x10
      w1 0,0 50x10
      w2 50,10 50x10
      w3 50,0 50x10 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
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

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = 'w1' } }, master_count = 2"),
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted
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

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout(
            "master = { { title = '/^e/' } }, secondary = { { title = 't1' }, { title = '/^e/' }, { title = 'm2' } }",
        ),
    );
    for title in ["e0", "t1", "m2", "e3"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused e3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      e0 0,0 50x20
      e3 50,7 50x6 highlighted
      m2 50,13 50x7
      t1 50,0 50x7
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       t1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                       e0                       |*                       e3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       m2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn open_window_whose_slots_are_held_takes_unmatched_placement() {
    let mut dome = TestDome::with_layout(MASTER, &layout("secondary = { { title = '/^t/' } }"));
    dome.open(window("t0"));
    dome.open(window("t1"));
    assert_snapshot!(dome.snapshot(), @"
    focused t1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t0 50,0 50x20
      t1 0,0 50x20 highlighted
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       t1                       *|                       t0                       |
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

    let mut dome = TestDome::with_layout(
        MASTER,
        "return { primary = { ['1'] = { layout = 'master', master = { { title = '/^t/' } }, secondary = { { title = '/^t/' } }, master_count = 2 } } }",
    );
    dome.open(window("t0"));
    let second = dome.open(window("t1"));
    dome.open(window("t2"));
    dome.run("master fewer");
    dome.close(second);
    dome.run("focus workspace 0");
    dome.open(window("t3"));
    assert_snapshot!(dome.snapshot(), @"
    focused t3 on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      t0 0,0 50x20
      t2 50,0 50x10
      t3 50,10 50x10 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       t2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       t0                       |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       t3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn open_match_after_move_lands_before_first_later_slot_holder() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("secondary = { { title = 'w1' }, { title = 'w2' }, { title = 'w3' } }"),
    );
    for title in ["w0", "w1", "w3"] {
        dome.open(window(title));
    }
    dome.run("move up");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,13 50x7
      w2 50,0 50x7 highlighted
      w3 50,7 50x6
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout(
            "master = { { title = 'w0' }, { title = 'w1' }, { title = 'w2' } }, master_count = 3",
        ),
    );
    dome.open(window("w0"));
    dome.open(window("w2"));
    dome.run("move up");
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,13 100x7
      w1 0,0 100x7 highlighted
      w2 0,7 100x6
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                w2                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn open_match_before_reordered_later_holders_uses_pane_order() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("secondary = { { title = 'w1' }, { title = 'w2' }, { title = 'w3' } }"),
    );
    for title in ["w0", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("move up");
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7 highlighted
      w2 50,13 50x7
      w3 50,7 50x6
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn move_slot_holders_across_panes_keeps_both_slots_held() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { { title = '/^a/' } }, secondary = { { title = '/^b/' } }"),
    );
    let first = dome.open(window("a0"));
    dome.open(window("b1"));
    dome.focus(first);
    dome.run("move right");
    dome.open(window("a2"));
    dome.open(window("b3"));
    assert_snapshot!(dome.snapshot(), @"
    focused b3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a0 50,0 50x7
      a2 50,7 50x6
      b1 0,0 50x20
      b3 50,13 50x7 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       a0                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       b1                       ||                       a2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       b3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn open_windows_into_layout_tabbed_pane_shows_tab_bar() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { display = 'tabbed' }, master_count = 2"),
    );
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 50x18
      w2 50,0 50x20 highlighted
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

    let mut dome = TestDome::with_layout(MASTER, &layout("secondary = { display = 'tabbed' }"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x18 highlighted
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

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout(
            "master = { display = 'tabbed' }, secondary = { display = 'tabbed' }, master_count = 2",
        ),
    );
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 50x18
      w3 50,2 50x18 highlighted
      container 0,0 50x20 tabbed active=1 titles=[w0, w1]
      container 50,0 50x20 tabbed active=1 titles=[w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |          w0            |        [w1]           ||          w2            |        [w3]           |
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w3                       *
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
fn apply_layout_resorts_windows_and_drops_keyboard_master_count() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("master more");
    dome.focus(first);
    dome.apply_layout(&layout("master = { { title = 'w2' } }"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x10 highlighted
      w1 50,10 50x10
      w2 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w0                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.apply_layout(&layout("master_count = 3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,7 100x6
      w1 0,13 100x7
      w2 0,0 100x7 highlighted
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn save_layout_writes_each_window_as_own_matcher_in_its_pane() {
    let mut dome = TestDome::new(MASTER);
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "master",
        },
      },
    }
    "#);

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "master",
          master = {
            { title = "w0" },
          },
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout(MASTER, &layout("secondary = { { title = 'w1' } }"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "master",
          master = {
            { title = "w0" },
          },
          secondary = {
            { title = "w1" },
            { title = "w2" },
          },
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout(MASTER, &layout("master = { { title = '/^w/' } }"));
    dome.open(window("w0"));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "master",
          master = {
            { title = "w0" },
          },
        },
      },
    }
    "#);
}

#[test]
fn save_and_apply_layout_restores_tabbed_pane() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 2 } }");
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.focus(first);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 100x18 highlighted
      container 0,0 100x20 tabbed active=0 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                      [w0]                       |                      w1                        |
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
    let saved = dome.save_layout();
    dome.run("toggle layout");
    assert_ne!(dome.snapshot(), before);
    dome.apply_layout(&saved);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn save_layout_writes_floats_and_fullscreens_from_live_metadata() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = '/^f/' } }, fullscreen = { { title = '/^g/' } }"),
    );
    dome.open(window("w0"));
    dome.run("toggle float");
    dome.open(window("w1"));
    dome.run("toggle float");
    dome.run("toggle float");
    dome.open(window("f2"));
    dome.open(window("w3"));
    dome.run("toggle fullscreen");
    dome.open(window("g4"));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "master",
          master = {
            { title = "w1" },
          },
          float = {
            { title = "w0" },
            { title = "f2" },
          },
          fullscreen = {
            { title = "w3" },
            { title = "g4" },
          },
        },
      },
    }
    "#);
}

#[test]
fn open_window_matching_float_matcher_floats_at_reported_rect() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f0' } }"));
    dome.open(window("f0").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             f0             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    dome.open(window("f1").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *             f1             *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         ******************************         w0                                                |
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
fn open_window_matching_fullscreen_matcher_covers_monitor() {
    let mut dome = TestDome::with_layout(MASTER, &layout("fullscreen = { { title = 'g1' } }"));
    dome.open(window("w0"));
    dome.open(window("g1"));
    assert_snapshot!(dome.snapshot(), @"
    focused g1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen g1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                g1                                                |
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
fn open_window_matching_several_matchers_prefers_fullscreen_then_float() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("fullscreen = { { title = 'x0' } }, float = { { title = 'x0' } }"),
    );
    dome.open(window("x0").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused x0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen x0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                x0                                                |
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

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = 'y0' } }, master = { { title = 'y0' } }"),
    );
    dome.open(window("y0").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused y0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      y0 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             y0             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn focus_left_and_right_cross_panes_to_last_focused_window() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    let last = dome.open(window("w3"));
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w1                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                       w0                       *|                       w2                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w3                       |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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
    dome.run("move up");
    dome.run("focus left");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,13 50x7
      w3 50,7 50x6 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                       w0                       |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("focus left");
    dome.close(last);
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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
fn focus_up_and_down_cycle_within_pane_and_wrap() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7 highlighted
      w2 50,7 50x6
      w3 50,13 50x7
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6 highlighted
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
    dome.run("focus left");
    let before = dome.snapshot();
    dome.run("focus up");
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn close_focused_window_focuses_most_recently_focused_survivor() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    let second = dome.open(window("w1"));
    let third = dome.open(window("w2"));
    let last = dome.open(window("w3"));
    dome.focus(second);
    dome.focus(last);
    dome.close(last);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10 highlighted
      w2 50,10 50x10
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.focus(first);
    dome.focus(third);
    dome.close(third);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted
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
fn move_scrolling_column_to_master_workspace_places_its_windows_in_secondary_pane() {
    let mut dome = TestDome::with_layout(
        MASTER,
        "return { primary = { ['1'] = { layout = 'scrolling' } } }",
    );
    dome.open(window("w0"));
    dome.open(window("w4"));
    dome.run("focus workspace 1");
    dome.open(window("w1"));
    dome.run("toggle spawn");
    let w2 = dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.focus(w2);
    dome.run("focus parent");
    dome.run("move workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w3 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w3                       *                         
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

    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,13 50x7
      w2 50,7 50x6 highlighted
      w4 50,0 50x7
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w4                       |
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
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn move_left_and_right_swap_across_panes() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    let second = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(second);
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,10 50x10
      w1 0,0 50x20 highlighted
      w2 50,0 50x10
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w2                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w1                       *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w0                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x10
      w1 50,10 50x10 highlighted
      w2 0,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w0                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w2                       |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_up_and_down_swap_within_pane_and_wrap() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("move up");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,13 50x7
      w3 50,7 50x6 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                       w0                       |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("move down");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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
    dome.run("move down");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,13 50x7
      w2 50,7 50x6
      w3 50,0 50x7 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w0                       ||                       w2                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w1                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("move up");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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
}

#[test]
fn move_left_with_master_room_promotes_without_swap() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("secondary = { { title = 'w1' } }, master_count = 2"),
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x10
      w1 0,10 100x10 highlighted
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
fn close_window_refills_master_from_top_of_secondary() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    let last = dome.open(window("w2"));
    dome.close(last);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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

    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x20
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
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
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::with_layout(MASTER, &layout("secondary = { { title = 'w1' } }"));
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 50,0 50x20
      w2 0,0 50x20 highlighted
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w2                       *|                       w1                       |
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

    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(first);
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w1 0,0 50x20
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
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
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 2 } }");
    let first = dome.open(window("w0"));
    for title in ["w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x10
      w2 0,10 50x10
      w3 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                       w3                       *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn run_ignored_or_targetless_verb_changes_nothing() {
    let directions = ["left", "right", "up", "down"];
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    for verb in ["toggle spawn", "toggle direction", "focus parent"] {
        let before = dome.snapshot();
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }

    let mut dome = TestDome::new(MASTER);
    for action in ["focus", "move"] {
        for direction in directions {
            let before = dome.snapshot();
            dome.run(&format!("{action} {direction}"));
            assert_eq!(dome.snapshot(), before, "{action} {direction}");
        }
    }
    for verb in ["grow", "shrink", "master more", "master fewer"] {
        let before = dome.snapshot();
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    for action in ["focus", "move"] {
        for direction in directions {
            let before = dome.snapshot();
            dome.run(&format!("{action} {direction}"));
            assert_eq!(dome.snapshot(), before, "{action} {direction}");
        }
    }

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f0' } }"));
    dome.open(window("f0"));
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 0,0 100x20 float highlighted
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                f0                                                *
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
    for action in ["focus", "move"] {
        for direction in directions {
            let before = dome.snapshot();
            dome.run(&format!("{action} {direction}"));
            assert_eq!(dome.snapshot(), before, "{action} {direction}");
        }
    }
    for verb in ["grow", "shrink", "master more", "master fewer"] {
        let before = dome.snapshot();
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f3' } }"));
    dome.open(window("w0"));
    let second = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(second);
    dome.run("toggle layout");
    dome.open(window("f3"));
    for action in ["focus", "move"] {
        for direction in directions {
            let before = dome.snapshot();
            dome.run(&format!("{action} {direction}"));
            assert_eq!(dome.snapshot(), before, "{action} {direction}");
        }
    }
    for verb in [
        "toggle layout",
        "focus tab next",
        "focus tab prev",
        "grow",
        "shrink",
    ] {
        let before = dome.snapshot();
        dome.run(verb);
        assert_eq!(dome.snapshot(), before, "{verb}");
    }
}

#[test]
fn master_more_and_fewer_while_float_has_focus_change_master_count() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f3' } }"));
    for title in ["w0", "w1", "w2", "f3"] {
        dome.open(window(title));
    }
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused f3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f3 0,0 100x20 float highlighted
      w0 0,0 50x10
      w1 0,10 50x10
      w2 50,0 50x20
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                f3                                                *
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
    dome.run("master fewer");
    assert_snapshot!(dome.snapshot(), @"
    focused f3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f3 0,0 100x20 float highlighted
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                f3                                                *
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
fn run_layout_verb_while_fullscreen_has_focus_changes_nothing() {
    let setup = || {
        let mut dome = TestDome::with_layout(MASTER, &layout("fullscreen = { { title = 'g3' } }"));
        dome.open(window("w0"));
        let second = dome.open(window("w1"));
        dome.open(window("w2"));
        dome.focus(second);
        dome.run("toggle layout");
        let fullscreen = dome.open(window("g3"));
        (dome, fullscreen)
    };
    let (mut control, fullscreen) = setup();
    control.close(fullscreen);
    let expected = control.snapshot();
    for verb in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
        "toggle layout",
        "focus tab next",
        "focus tab prev",
        "grow",
        "shrink",
        "master more",
        "master fewer",
    ] {
        let (mut dome, fullscreen) = setup();
        dome.run(verb);
        dome.close(fullscreen);
        assert_eq!(dome.snapshot(), expected, "{verb}");
    }
    let (mut dome, fullscreen) = setup();
    dome.click_tab(container(1), 1);
    dome.close(fullscreen);
    assert_eq!(dome.snapshot(), expected);
}

#[test]
fn click_tab_activates_tab_and_focuses_its_window() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 2 } }");
    let first = dome.open(window("first"));
    dome.open(window("second"));
    dome.open(window("secondary"));
    dome.focus(first);
    dome.run("toggle layout");
    dome.click_tab(container(0), 1);
    assert_snapshot!(dome.snapshot(), @"
    focused second on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      second 0,2 50x18 highlighted
      secondary 50,0 50x20
      container 0,0 50x20 tabbed active=1 titles=[first, second]
    +------------------------------------------------++------------------------------------------------+
    |         first          |      [second]         ||                                                |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                    secondary                   |
    *                     second                     *|                                                |
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

    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("toggle layout");
    dome.click_tab(container(1), 0);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=0 titles=[w1, w2]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||         [w1]           |         w2            |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                       w1                       *
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
    let before = dome.snapshot();
    dome.click_tab(container(1), 5);
    assert_eq!(dome.snapshot(), before);

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f3' } }"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("toggle layout");
    dome.open(window("f3"));
    dome.click_tab(container(1), 0);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f3 0,0 100x20 float
      w0 0,0 50x20
      w1 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=0 titles=[w1, w2]
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                f3                                                |
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
fn click_tab_after_pane_loses_tabs_keeps_focus() {
    for singleton in [false, true] {
        let mut dome = TestDome::new(MASTER);
        let master = dome.open(window("master"));
        dome.open(window("first"));
        let second = dome.open(window("second"));
        dome.run("toggle layout");
        if singleton {
            dome.close(second);
        } else {
            dome.run("toggle layout");
        }
        dome.focus(master);
        let before = dome.snapshot();
        dome.click_tab(container(1), 0);
        assert_eq!(dome.snapshot(), before);
    }
}

#[test]
fn set_minimum_height_past_screen_scrolls_pane_to_focused_window() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 3 } }");
    let windows: Vec<_> = (0..6)
        .map(|index| dome.open(window(&format!("w{index}"))))
        .collect();
    for &id in &windows[..3] {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,-8 50x14 off-screen
      w2 0,6 50x14 highlighted
      w3 50,0 50x7
      w4 50,7 50x6
      w5 50,13 50x7
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                       w1                       ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    +------------------------------------------------+|                                                |
    **************************************************+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                       w4                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w2                       *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w5                       |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    for &id in &windows[3..] {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w5 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,-8 50x14 off-screen
      w2 0,6 50x14
      w4 50,-8 50x14 off-screen
      w5 50,6 50x14 highlighted
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w1                       ||                       w4                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                       w5                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.open(window("w6"));
    assert_snapshot!(dome.snapshot(), @"
    focused w6 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,-8 50x14 off-screen
      w2 0,6 50x14
      w4 50,-9 50x14 off-screen
      w5 50,5 50x14
      w6 50,19 50x1 highlighted off-screen
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w1                       ||                       w4                       |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    +------------------------------------------------++------------------------------------------------+
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w5                       |
    |                       w2                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_or_close_window_out_of_scrolled_pane_clamps_its_scroll() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 3 } }");
    let windows: Vec<_> = (0..6)
        .map(|index| dome.open(window(&format!("w{index}"))))
        .collect();
    for &id in &windows {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.run("master more");
    assert_snapshot!(dome.snapshot(), @"
    focused w5 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x14
      w1 0,14 50x14 off-screen
      w4 50,-8 50x14 off-screen
      w5 50,6 50x14 highlighted
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                       w4                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                       w0                       |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w5                       *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    ");

    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 3 } }");
    let windows: Vec<_> = (0..6)
        .map(|index| dome.open(window(&format!("w{index}"))))
        .collect();
    for &id in &windows {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.run("focus left");
    dome.run("master fewer");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,-8 50x14 off-screen
      w1 0,6 50x14
      w2 50,0 50x14 highlighted
      w3 50,14 50x14 off-screen
    |                                                |**************************************************
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                       w2                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |**************************************************
    |                                                |+------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                                                |
    ");

    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 3 } }");
    let windows: Vec<_> = (0..6)
        .map(|index| dome.open(window(&format!("w{index}"))))
        .collect();
    for &id in &windows {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.close(windows[5]);
    assert_snapshot!(dome.snapshot(), @"
    focused w4 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x14
      w1 0,14 50x14 off-screen
      w3 50,-8 50x14 off-screen
      w4 50,6 50x14 highlighted
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                |+------------------------------------------------+
    |                       w0                       |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w4                       *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    ");
}

#[test]
fn reload_smaller_minimum_height_clamps_pane_scroll() {
    let mut dome = TestDome::new(
        "return { layout = 'master', minimum_height = 12, master = { master_count = 3 } }",
    );
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.reload("return { layout = 'master', minimum_height = 1, master = { master_count = 3 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x7
      w1 0,7 100x6
      w2 0,13 100x7 highlighted
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn set_size_limits_center_capped_window_and_ignore_minimum_width() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("master"));
    let secondary = dome.open(window("secondary"));
    dome.set_constraint(
        secondary,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    dome.set_constraint(
        secondary,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );
    dome.set_constraint(
        secondary,
        LimitObservation {
            min_width: LimitUpdate::Cleared,
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused secondary on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      master 0,0 50x20
      secondary 59,0 32x20 highlighted
    +------------------------------------------------+         ********************************         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                     master                     |         *           secondary          *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    +------------------------------------------------+         ********************************
    ");

    let mut dome = TestDome::new(MASTER);
    dome.open(window("master"));
    let first = dome.open(window("first"));
    let second = dome.open(window("second"));
    for window in [first, second] {
        dome.set_constraint(
            window,
            LimitObservation {
                max_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    assert_snapshot!(dome.snapshot(), @"
    focused second on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      first 50,0 50x10
      master 0,0 50x20
      second 50,10 50x10 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                      first                     |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                     master                     |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                     second                     *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new(MASTER);
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.set_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(80.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_constraint(
        w1,
        LimitObservation {
            max_height: LimitUpdate::Set(Length::new(6.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,6 50x8 highlighted
    +------------------------------------------------+                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |**************************************************
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    |                                                |                                                  
    +------------------------------------------------+
    ");

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 59,0 32x20 highlighted
    +------------------------------------------------+         ********************************         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                       w0                       |         *              w1              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    |                                                |         *                              *         
    +------------------------------------------------+         ********************************
    ");

    let mut dome = TestDome::new(MASTER);
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.set_constraint(
        w0,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(30.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 9,0 32x20
      w1 50,0 50x20 highlighted
             +------------------------------+         **************************************************
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |              w0              |         *                       w1                       *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             |                              |         *                                                *
             +------------------------------+         **************************************************
    ");

    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 2 } }");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w0);
    dome.run("toggle layout");
    dome.set_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(80.0)),
            ..Default::default()
        },
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 50x18 highlighted
      w2 50,0 50x20
      container 0,0 50x20 tabbed active=0 titles=[w0, w1]
    +------------------------------------------------++------------------------------------------------+
    |         [w0]           |         w1            ||                                                |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w2                       |
    *                       w0                       *|                                                |
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

    let mut dome = TestDome::new("return { layout = 'master', maximum_width = '50%' }");
    dome.open(window("w0"));
    dome.reload("return { layout = 'master', maximum_width = '50%', reserved_area = function() return { left = 40 } end }");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 40,0 60x20 border 1
      w0 55,0 30x20 highlighted
                   ******************************               
                   *                            *               
                   *                            *               
                   *                            *               
                   *                            *               
                   *                            *               
                   *                            *               
                   *                            *               
                   *                            *               
                   *             w0             *               
                   *                            *               
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
    #[cfg(target_os = "windows")]
    {
        let mut dome = TestDome::new("return { layout = 'master', maximum_width = 30 }");
        dome.open(window("w0"));
        dome.update_monitor(
            dome.primary_monitor(),
            monitor("primary", [0, 0, 100, 20]).scale(2.0),
            None,
        );
        assert_snapshot!(dome.snapshot(), @"
        focused w0 on primary
        workspaces: 0 on primary (focused)
        primary 0,0 100x20 border 2
          w0 20,0 60x20 highlighted
                            ************************************************************                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                            w0                            *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            *                                                          *                    
                            ************************************************************                    
        ");
    }
}

#[test]
fn toggle_float_on_scrolled_tile_floats_at_its_shown_rect() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 4 } }");
    let windows: Vec<_> = (0..4)
        .map(|index| dome.open(window(&format!("w{index}"))))
        .collect();
    for id in windows {
        dome.set_constraint(
            id,
            LimitObservation {
                min_height: LimitUpdate::Set(Length::new(12.0)),
                ..Default::default()
            },
        );
    }
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,-8 100x14 off-screen
      w2 0,6 100x14
      w3 0,6 100x14 float highlighted
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ****************************************************************************************************
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
    ****************************************************************************************************
    ");
}

#[test]
fn toggle_layout_tabs_focused_pane_and_back() {
    let mut dome = TestDome::new("return { layout = 'master', master = { master_count = 2 } }");
    let w0 = dome.open(window("w0"));
    for title in ["w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 50x10
      w3 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=1 titles=[w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||          w2            |        [w3]           |
    |                                                |**************************************************
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.focus(w0);
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,2 50x18 highlighted
      w3 50,2 50x18
      container 0,0 50x20 tabbed active=0 titles=[w0, w1]
      container 50,0 50x20 tabbed active=1 titles=[w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |         [w0]           |         w1            ||          w2            |        [w3]           |
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                       w3                       |
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
    dome.run("toggle layout");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10 highlighted
      w1 0,10 50x10
      w3 50,2 50x18
      container 50,0 50x20 tabbed active=1 titles=[w2, w3]
    **************************************************+------------------------------------------------+
    *                                                *|          w2            |        [w3]           |
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                       w0                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************|                                                |
    +------------------------------------------------+|                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w1                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");

    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x18 highlighted
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
}

#[test]
fn focus_tab_next_and_prev_cycle_active_tab_and_wrap() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.run("toggle layout");
    dome.run("focus tab next");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=0 titles=[w1, w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||     [w1]       |     w2        |     w3        |
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                       w1                       *
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
    dome.run("focus tab prev");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w3 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=2 titles=[w1, w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||      w1        |     w2        |    [w3]       |
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
    dome.run("focus tab prev");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w2 50,2 50x18 highlighted
      container 50,0 50x20 tabbed active=1 titles=[w1, w2, w3]
    +------------------------------------------------++------------------------------------------------+
    |                                                ||      w1        |    [w2]       |     w3        |
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
    dome.run("focus left");
    let before = dome.snapshot();
    dome.run("focus tab next");
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn open_second_tab_with_tab_bar_taller_than_screen_cuts_bar_to_screen() {
    let mut dome = TestDome::with_layout(
        "return { layout = 'master', partition_tree = { tab_bar_height = 30 } }",
        &layout("master = { display = 'tabbed' }, master_count = 2"),
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
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
}

#[cfg(target_os = "windows")]
#[test]
fn update_monitor_scale_scales_tab_bar_height() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("master = { display = 'tabbed' }, master_count = 2"),
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,2 100x18 highlighted
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
      w1 0,4 100x16 highlighted
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
fn reload_master_count_and_ratio_reach_untuned_workspace() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.reload("return { layout = 'master', master = { master_count = 3 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x7
      w1 0,7 50x6
      w2 0,13 50x7
      w3 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w3                       *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                                                *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.reload(MASTER);
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x7
      w2 50,7 50x6
      w3 50,13 50x7 highlighted
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
    dome.reload("return { layout = 'master', master = { master_ratio = 0.7 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 70x20
      w1 70,0 30x7
      w2 70,7 30x6
      w3 70,13 30x7 highlighted
    +--------------------------------------------------------------------++----------------------------+
    |                                                                    ||                            |
    |                                                                    ||                            |
    |                                                                    ||             w1             |
    |                                                                    ||                            |
    |                                                                    ||                            |
    |                                                                    |+----------------------------+
    |                                                                    |+----------------------------+
    |                                                                    ||                            |
    |                                 w0                                 ||             w2             |
    |                                                                    ||                            |
    |                                                                    ||                            |
    |                                                                    |+----------------------------+
    |                                                                    |******************************
    |                                                                    |*                            *
    |                                                                    |*                            *
    |                                                                    |*             w3             *
    |                                                                    |*                            *
    |                                                                    |*                            *
    +--------------------------------------------------------------------+******************************
    ");
}

#[test]
fn reload_keeps_keyboard_tuned_ratio_and_count() {
    let mut dome = TestDome::new(MASTER);
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("master more");
    dome.reload("return { layout = 'master', master = { master_count = 3 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x10
      w1 0,10 50x10
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+*                       w2                       *
    +------------------------------------------------+*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.focus(w0);
    dome.run("grow");
    dome.reload("return { layout = 'master', master = { master_ratio = 0.7 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 55x10 highlighted
      w1 0,10 55x10
      w2 55,0 45x20
    *******************************************************+-------------------------------------------+
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                          w0                         *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *                                                     *|                                           |
    *******************************************************|                     w2                    |
    +-----------------------------------------------------+|                                           |
    |                                                     ||                                           |
    |                                                     ||                                           |
    |                                                     ||                                           |
    |                          w1                         ||                                           |
    |                                                     ||                                           |
    |                                                     ||                                           |
    |                                                     ||                                           |
    |                                                     ||                                           |
    +-----------------------------------------------------++-------------------------------------------+
    ");
}

#[test]
fn reload_keeps_layout_master_count_and_ratio_overrides() {
    let mut dome = TestDome::with_layout(MASTER, &layout("master_ratio = 0.6, master_count = 2"));
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.reload("return { layout = 'master', master = { master_count = 3, master_ratio = 0.7 } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 60x10
      w1 0,10 60x10
      w2 60,0 40x20 highlighted
    +----------------------------------------------------------+****************************************
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                            w0                            |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    +----------------------------------------------------------+*                  w2                  *
    +----------------------------------------------------------+*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                            w1                            |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    |                                                          |*                                      *
    +----------------------------------------------------------+****************************************
    ");
}

#[test]
fn toggle_float_floats_tile_at_its_rect_and_back_keeps_focus() {
    let mut dome = TestDome::new(MASTER);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20
      w2 50,10 50x10 float highlighted
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
    |                                                |**************************************************
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
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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
    let mut dome = TestDome::new(MASTER);
    dome.add_monitor(monitor("external", [150, 0, 100, 20]));
    dome.run("focus monitor external");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on external
    workspaces: 0 on primary (visible), 0 on external (focused)
    primary 0,0 100x20 border 1
    external 150,0 100x20 border 1
      w0 150,0 100x20
      w1 200,0 50x20 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
    
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
fn move_float_takes_reported_rect_and_keeps_stacking_and_focus() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = '/^f/' }, { title = '/^f/' } }"),
    );
    dome.open(window("w0"));
    let first = dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.open(window("f2").frame([40, 4, 30, 8]));
    dome.move_float(first, [20, 6, 40, 10], dome.primary_monitor());
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 19,5 42x12 float
      f2 40,4 30x8 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                       ******************************                             |
    |                  +--------------------*                            *                             |
    |                  |                    *                            *                             |
    |                  |                    *             f2             *                             |
    |                  |                    *                            *                             |
    |                  |                    *                            *                             |
    |                  |                   f*                            *                             |
    |                  |                    ******************************                             |
    |                  |                                        |                                      |
    |                  |                                        |                                      |
    |                  |                                        |                                      |
    |                  |                                        |                                      |
    |                  +----------------------------------------+                                      |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn move_float_for_window_no_longer_floating_changes_nothing() {
    for fullscreen in [false, true] {
        let mut dome = TestDome::new(MASTER);
        let second_monitor = dome.add_monitor(monitor("DELL", [100, 0, 60, 20]));
        let tile = dome.open(window("w0"));
        if fullscreen {
            dome.set_fullscreen(tile, WindowRestrictions::None);
        } else {
            dome.run("toggle float");
            dome.run("toggle float");
        }
        let before = dome.snapshot();
        dome.move_float(tile, [110, 2, 30, 8], second_monitor);
        assert_eq!(dome.snapshot(), before, "fullscreen={fullscreen}");
    }
}

#[test]
fn open_float_narrower_than_its_borders_gets_no_placement() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    dome.open(window("f1").frame([10, 2, 2, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
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
fn toggle_fullscreen_covers_monitor_and_toggling_back_tiles() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
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
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
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

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                f1                                                |
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
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 50,0 50x20 highlighted
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
    |                       w0                       |*                       f1                       *
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
fn set_fullscreen_on_tile_or_float_covers_monitor() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    let tile = dome.open(window("w1"));
    dome.set_fullscreen(tile, WindowRestrictions::None);
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

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    let float = dome.open(window("f1"));
    dome.set_fullscreen(float, WindowRestrictions::None);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                f1                                                |
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
fn set_fullscreen_on_fullscreen_window_keeps_its_place_in_stack() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    let second = dome.open(window("w1"));
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
    let before = dome.snapshot();
    dome.set_fullscreen(first, WindowRestrictions::ProtectFullscreen);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn unset_fullscreen_tiles_window_and_ignores_non_fullscreen() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f3' } }"));
    dome.open(window("w0"));
    let tile = dome.open(window("w1"));
    let fullscreen = dome.open(window("w2").restricted(WindowRestrictions::ProtectFullscreen));
    dome.unset_fullscreen(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x10
      w2 50,10 50x10 highlighted
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
    |                                                |**************************************************
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
    let before = dome.snapshot();
    dome.unset_fullscreen(tile);
    assert_eq!(dome.snapshot(), before);
    let float = dome.open(window("f3"));
    let before = dome.snapshot();
    dome.unset_fullscreen(float);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn set_fullscreen_on_two_windows_shows_only_topmost() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    let second = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
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
fn toggle_float_on_fullscreen_window_changes_nothing() {
    let mut dome = TestDome::new(MASTER);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle fullscreen");
    let before = dome.snapshot();
    dome.run("toggle float");
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn open_window_under_fullscreen_keeps_fullscreen_covering_and_focused() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f2' } }"));
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
    dome.open(window("f2").frame([10, 2, 30, 8]));
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
fn focus_window_moves_highlight_between_tiles_and_floats() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f2' } }"));
    let left = dome.open(window("w0"));
    let right = dome.open(window("w1"));
    let float = dome.open(window("f2").frame([10, 2, 30, 8]));
    dome.focus(left);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 10,2 30x8 float
      w0 0,0 50x20 highlighted
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *         +----------------------------+         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         |             f2             |         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         +----------------------------+         *|                       w1                       |
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
    dome.focus(float);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 10,2 30x8 float highlighted
      w0 0,0 50x20
      w1 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         ******************************         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *             f2             *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         ******************************         ||                       w1                       |
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
    dome.focus(right);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 10,2 30x8 float
      w0 0,0 50x20
      w1 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |             f2             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*                       w1                       *
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
fn focus_float_raises_it_above_other_floats() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = '/^f/' }, { title = '/^f/' } }"),
    );
    dome.open(window("w0"));
    let first = dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.open(window("f2").frame([20, 4, 30, 8]));
    dome.focus(first);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      f2 20,4 30x8 float
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *---------+                                                 |
    |         *             f1             *         |                                                 |
    |         *                            *         |                                                 |
    |         *                            *         |                                                 |
    |         *                            *         |                                                 |
    |         ******************************         |0                                                |
    |                   |                            |                                                 |
    |                   +----------------------------+                                                 |
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
fn focus_covered_fullscreen_window_raises_it_over_monitor() {
    let mut dome = TestDome::new(MASTER);
    let first = dome.open(window("w0"));
    let second = dome.open(window("w1"));
    dome.set_fullscreen(first, WindowRestrictions::None);
    dome.set_fullscreen(second, WindowRestrictions::None);
    dome.focus(first);
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
fn focus_tile_under_fullscreen_takes_focus_once_fullscreen_closes() {
    let mut dome = TestDome::with_layout(MASTER, &layout("fullscreen = { { title = 'g3' } }"));
    let first = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    let fullscreen = dome.open(window("g3"));
    let before = dome.snapshot();
    dome.focus(first);
    assert_eq!(dome.snapshot(), before);
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20 highlighted
      w1 50,0 50x10
      w2 50,10 50x10
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w1                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *+------------------------------------------------+
    *                                                *+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                       w2                       |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn toggle_or_close_topmost_fullscreen_reveals_fullscreen_below() {
    for close in [false, true] {
        let mut dome = TestDome::new(MASTER);
        dome.open(window("w0"));
        let covered = dome.open(window("w1"));
        let topmost = dome.open(window("w2"));
        dome.set_fullscreen(covered, WindowRestrictions::None);
        dome.set_fullscreen(topmost, WindowRestrictions::None);
        if close {
            dome.close(topmost);
        } else {
            dome.run("toggle fullscreen");
        }
        insta::allow_duplicates! {
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
    }
}

#[test]
fn unset_or_close_covered_window_keeps_topmost_focused() {
    for close in [false, true] {
        let mut dome = TestDome::new(MASTER);
        dome.open(window("w0"));
        let covered = dome.open(window("w1"));
        let topmost = dome.open(window("w2"));
        dome.set_fullscreen(covered, WindowRestrictions::None);
        dome.set_fullscreen(topmost, WindowRestrictions::None);
        if close {
            dome.close(covered);
        } else {
            dome.unset_fullscreen(covered);
        }
        insta::allow_duplicates! {
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
        }
    }

    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = '/^f/' }, { title = '/^f/' } }"),
    );
    dome.open(window("w0"));
    let covered = dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.open(window("f2").frame([20, 4, 30, 8]));
    dome.close(covered);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 20,4 30x8 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                   ******************************                                                 |
    |                   *                            *                                                 |
    |                   *                            *                                                 |
    |                   *             f2             *                                                 |
    |                   *                            *                                                 |
    |                   *                            *0                                                |
    |                   *                            *                                                 |
    |                   ******************************                                                 |
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
fn remove_last_fullscreen_without_tiles_focuses_float() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f0' } }"));
    dome.open(window("f0").frame([10, 2, 30, 8]));
    let fullscreen = dome.open(window("w1"));
    dome.run("toggle fullscreen");
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f0 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             f0             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f0' } }"));
    dome.open(window("f0").frame([10, 2, 30, 8]));
    let fullscreen = dome.open(window("w1"));
    dome.run("toggle fullscreen");
    dome.minimize(fullscreen);
    assert_snapshot!(dome.snapshot(), @r#"
    focused f0 on primary
    workspaces: 0 on primary (focused)
    minimized: w1 "w1"
    primary 0,0 100x20 border 1
      f0 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             f0             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    "#);

    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f0' } }"));
    dome.open(window("f0").frame([10, 2, 30, 8]));
    dome.open(window("w1"));
    dome.run("toggle fullscreen");
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused f0 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      f0 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             f0             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn close_only_fullscreen_focuses_tile_over_float() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    dome.open(window("f1").frame([10, 2, 30, 8]));
    let fullscreen = dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float
      w0 0,0 100x20 highlighted
    ****************************************************************************************************
    *                                                                                                  *
    *         +----------------------------+                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |             f1             |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         +----------------------------+         w0                                                *
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
fn toggle_fullscreen_after_focusing_float_beneath_keeps_focus_on_window() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    dome.open(window("w0"));
    let float = dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.focus(float);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float
      w0 0,0 50x20
      w2 50,0 50x20 highlighted
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |             f1             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*                       w2                       *
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
fn close_app_fullscreen_window_returns_focus_to_float() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f2' } }"));
    dome.open(window("w0"));
    let fullscreen = dome.open(window("w1"));
    dome.open(window("f2").frame([10, 2, 30, 8]));
    dome.set_fullscreen(fullscreen, WindowRestrictions::None);
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 10,2 30x8 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *             f2             *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         ******************************         w0                                                |
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
fn close_focused_float_focuses_next_float_else_tile() {
    let mut dome = TestDome::with_layout(
        MASTER,
        &layout("float = { { title = '/^f/' }, { title = '/^f/' } }"),
    );
    dome.open(window("w0"));
    let first = dome.open(window("f1").frame([10, 2, 30, 8]));
    let second = dome.open(window("f2").frame([20, 4, 30, 8]));
    dome.close(second);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      w0 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *             f1             *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         ******************************         w0                                                |
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
    dome.close(first);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted
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
fn close_last_tile_focuses_float() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
    let tile = dome.open(window("w0"));
    dome.open(window("f1").frame([10, 2, 30, 8]));
    dome.focus(tile);
    dome.close(tile);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *             f1             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn minimize_last_tile_leaves_float_focused() {
    for focus_tile in [true, false] {
        let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f1' } }"));
        let tile = dome.open(window("w0"));
        dome.open(window("f1").frame([10, 2, 30, 8]));
        if focus_tile {
            dome.focus(tile);
        }
        dome.minimize(tile);
        insta::allow_duplicates! {
            assert_snapshot!(dome.snapshot(), @r#"
            focused f1 on primary
            workspaces: 0 on primary (focused)
            minimized: w0 "w0"
            primary 0,0 100x20 border 1
              f1 10,2 30x8 float highlighted
                                                                                                                
                                                                                                                
                      ******************************                                                            
                      *                            *                                                            
                      *                            *                                                            
                      *             f1             *                                                            
                      *                            *                                                            
                      *                            *                                                            
                      *                            *                                                            
                      ******************************
            "#);
        }
    }
}

#[test]
fn move_fullscreen_onto_workspace_with_focused_float_clears_float_focus() {
    let mut dome = TestDome::new(MASTER);
    dome.run("focus workspace 1");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle float");
    dome.run("focus workspace 0");
    let fullscreen = dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted
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
fn unminimize_fullscreen_window_clears_float_focus() {
    let mut dome = TestDome::with_layout(MASTER, &layout("float = { { title = 'f2' } }"));
    dome.open(window("w0"));
    let fullscreen = dome.open(window("w1"));
    dome.run("toggle fullscreen");
    dome.minimize(fullscreen);
    dome.open(window("f2").frame([10, 2, 30, 8]));
    dome.unminimize(fullscreen);
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 10,2 30x8 float
      w0 0,0 100x20 highlighted
    ****************************************************************************************************
    *                                                                                                  *
    *         +----------------------------+                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |             f2             |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         +----------------------------+         w0                                                *
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
fn apply_layout_focuses_topmost_fullscreen_else_topmost_float() {
    let both_modes = layout(
        "float = { { title = '/^f/' }, { title = '/^f/' } },
         fullscreen = { { title = '/^g/' }, { title = '/^g/' } }",
    );
    let mut dome = TestDome::with_layout(MASTER, &both_modes);
    for title in ["w0", "f1", "f2", "g3", "g4"] {
        dome.open(window(title));
    }
    dome.apply_layout(&both_modes);
    assert_snapshot!(dome.snapshot(), @"
    focused g4 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen g4
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                g4                                                |
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

    let floats = layout("float = { { title = '/^f/' }, { title = '/^f/' } }");
    let mut dome = TestDome::with_layout(MASTER, &floats);
    let tile = dome.open(window("w0"));
    dome.open(window("f1"));
    dome.open(window("f2"));
    dome.focus(tile);
    dome.apply_layout(&floats);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 0,0 100x20 float
      f2 0,0 100x20 float highlighted
      w0 0,0 100x20
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                f2                                                *
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
