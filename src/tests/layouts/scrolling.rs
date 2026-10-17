use insta::assert_snapshot;

use crate::core::{Length, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::tests::test_dome::{TestDome, container, monitor, window};

const SCROLLING: &str = "return { layout = 'scrolling' }";
const SCROLLING_20_PERCENT: &str =
    "return { layout = 'scrolling', scrolling = { column_width = '20%' } }";
const SCROLLING_40_PERCENT: &str =
    "return { layout = 'scrolling', scrolling = { column_width = '40%' } }";
const FLOAT_FRAME: [i32; 4] = [10, 2, 30, 8];
const OVERLAPPING_FLOAT_FRAME: [i32; 4] = [20, 6, 30, 8];

fn display_modes_layout() -> String {
    workspace_0_layout(
        "float = { { title = 'f' }, { title = 'f1' }, { title = 'f2' } },
         fullscreen = { { title = 'fs' }, { title = 'fs1' }, { title = 'fs2' } }",
    )
}

fn workspace_0_layout(body: &str) -> String {
    format!("return {{ primary = {{ ['0'] = {{ layout = 'scrolling', {body} }} }} }}")
}

fn limit(value: Option<f32>) -> LimitUpdate {
    value.map_or(LimitUpdate::Cleared, |value| {
        LimitUpdate::Set(Length::new(value))
    })
}

fn minimum_width(width: Option<f32>) -> LimitObservation {
    LimitObservation {
        min_width: limit(width),
        ..LimitObservation::default()
    }
}

fn maximum_width(width: Option<f32>) -> LimitObservation {
    LimitObservation {
        max_width: limit(width),
        ..LimitObservation::default()
    }
}

fn minimum_height(height: Option<f32>) -> LimitObservation {
    LimitObservation {
        min_height: limit(height),
        ..LimitObservation::default()
    }
}

fn maximum_height(height: Option<f32>) -> LimitObservation {
    LimitObservation {
        max_height: limit(height),
        ..LimitObservation::default()
    }
}

#[test]
fn open_on_scrolling_workspace_adds_column_right_of_focus() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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
    dome.open(window("w1"));
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
    dome.run("focus left");
    dome.run("focus left");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w3 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w3                       *
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
fn open_or_close_in_row_narrower_than_work_area_keeps_row_centered() {
    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 30,0 20x20
      w1 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |        w0        |*        w1        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 20,0 20x20
      w1 40,0 20x20
      w2 60,0 20x20 highlighted spawn=right
                        +------------------++------------------+********************                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |        w0        ||        w1        |*        w2        *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        |                  ||                  |*                  *                    
                        +------------------++------------------+********************
    ");
    dome.close(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 30,0 20x20
      w2 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |        w0        |*        w2        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");
}

#[test]
fn open_past_work_area_edge_scrolls_focused_column_into_view() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x20
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

    let mut dome = TestDome::new(SCROLLING_40_PERCENT);
    for title in ["w0", "w1", "w2", "w3"] {
        dome.open(window(title));
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 -20,0 40x20 off-screen
      w2 20,0 40x20
      w3 60,0 40x20 highlighted spawn=right
    -------------------++--------------------------------------+****************************************
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
             w1        ||                  w2                  |*                  w3                  *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
    -------------------++--------------------------------------+****************************************
    ");
}

#[test]
fn toggle_spawn_switches_next_window_between_stack_and_column() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x20 highlighted spawn=bottom
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x10
      w1 25,10 50x10 highlighted spawn=bottom
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                       w0                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w1                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x7
      w1 25,7 50x6
      w2 25,13 50x7 highlighted spawn=bottom
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                       w0                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                       w1                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                       w2                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.run("toggle spawn");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x7
      w1 0,7 50x6
      w2 0,13 50x7
      w3 50,0 50x20 highlighted spawn=right
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
    dome.focus(w0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x7 highlighted spawn=bottom
      w1 0,7 50x6
      w2 0,13 50x7
      w3 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w0                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************|                                                |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                       w1                       ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                                                |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w2                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn open_or_move_in_follows_spawn_direction_unless_column_matcher_is_free() {
    let layout = workspace_0_layout(
        "columns = { { title = 'a' }, { title = 'b' } }, float = { { title = 'f' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    dome.run("toggle spawn");
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x20
      b 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        a                       |*                        b                       *
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
    dome.focus(a);
    dome.open(window("a").label("second-a"));
    assert_snapshot!(dome.snapshot(), @"
    focused second-a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      b 50,0 50x20
      second-a 0,10 50x10 highlighted spawn=bottom
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        a                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                        b                       |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                    second-a                    *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.open(window("f").frame(FLOAT_FRAME));
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      b 50,0 50x20
      f 10,2 30x8 float highlighted
      second-a 0,10 50x10
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         ******************************         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *              f             *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    +---------******************************---------+|                        b                       |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                    second-a                    ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.open(window("u"));
    assert_snapshot!(dome.snapshot(), @"
    focused u on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x7
      b 50,0 50x20
      f 10,2 30x8 float
      second-a 0,7 50x6
      u 0,13 50x7 highlighted spawn=bottom
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         +----------------------------+         ||                                                |
    |         |                            |         ||                                                |
    |         |                            |         ||                                                |
    |         |              f             |         ||                                                |
    +---------|                            |---------+|                                                |
    +---------|                            |---------+|                                                |
    |         |                            |         ||                                                |
    |         +----------------------------+         ||                        b                       |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                                                |
    **************************************************|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        u                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus workspace 1");
    dome.open(window("m"));
    dome.run("move workspace 0");
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused m on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x5
      b 50,0 50x20
      f 10,2 30x8 float
      m 0,15 50x5 highlighted spawn=bottom
      second-a 0,5 50x5
      u 0,10 50x5
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         +----------------------------+         ||                                                |
    |         |                            |         ||                                                |
    +---------|                            |---------+|                                                |
    +---------|              f             |---------+|                                                |
    |         |                            |         ||                                                |
    |         |                            |         ||                                                |
    |         |                            |         ||                                                |
    +---------+----------------------------+---------+|                        b                       |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                        u                       ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                                                |
    **************************************************|                                                |
    *                                                *|                                                |
    *                        m                       *|                                                |
    *                                                *|                                                |
    **************************************************+------------------------------------------------+
    ");
}

#[test]
fn open_into_stack_shares_column_height_in_matcher_order() {
    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' }, { title = 'c' } } } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      b 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        b                       *                         
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
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x10 highlighted spawn=right
      b 25,10 50x10
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7
      b 25,7 50x6
      c 25,13 50x7 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
}

#[test]
fn set_constraint_with_minimum_height_in_stack_keeps_minimum_and_scrolls_to_focus() {
    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.set_constraint(a, minimum_height(Some(12.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x14
      b 25,14 50x6 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.set_constraint(b, minimum_height(Some(12.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,-8 50x14 off-screen
      b 25,6 50x14 highlighted spawn=right
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.focus(a);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x14 highlighted spawn=right
      b 25,14 50x14 off-screen
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |
    ");
}

#[test]
fn set_constraint_with_maximum_height_centers_stack_in_column() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, maximum_height(Some(10.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,4 50x12 highlighted spawn=right
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");

    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    dome.open(window("b"));
    dome.set_constraint(a, maximum_height(Some(5.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,2 50x7
      b 25,9 50x10 highlighted spawn=right
                                                                                                        
                                                                                                        
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
}

#[test]
fn set_constraint_with_minimum_width_widens_column_to_largest_minimum() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(60.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 19,0 62x20 highlighted spawn=right
                       **************************************************************                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                             w0                             *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       **************************************************************
    ");
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 152x20 highlighted spawn=right off-screen
    ****************************************************************************************************
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                w0                                                 
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    ****************************************************************************************************
    ");

    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.set_constraint(b, minimum_width(Some(60.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 19,0 62x10
      b 19,10 62x10 highlighted spawn=right
                       +------------------------------------------------------------+                   
                       |                                                            |                   
                       |                                                            |                   
                       |                                                            |                   
                       |                              a                             |                   
                       |                                                            |                   
                       |                                                            |                   
                       |                                                            |                   
                       |                                                            |                   
                       +------------------------------------------------------------+                   
                       **************************************************************                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                              b                             *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       *                                                            *                   
                       **************************************************************
    ");
}

#[test]
fn set_constraint_with_maximum_width_narrows_column_unless_minimum_is_larger() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.set_constraint(w0, maximum_width(Some(20.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 14,0 22x20
      w1 36,0 50x20 highlighted spawn=right
                  +--------------------+**************************************************              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |         w0         |*                       w1                       *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  |                    |*                                                *              
                  +--------------------+**************************************************
    ");

    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.set_constraint(a, maximum_width(Some(20.0)));
    dome.set_constraint(b, maximum_width(Some(30.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 39,0 22x10
      b 39,10 22x10 highlighted spawn=right
                                           +--------------------+                                       
                                           |                    |                                       
                                           |                    |                                       
                                           |                    |                                       
                                           |          a         |                                       
                                           |                    |                                       
                                           |                    |                                       
                                           |                    |                                       
                                           |                    |                                       
                                           +--------------------+                                       
                                           **********************                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *          b         *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           **********************
    ");
    dome.set_constraint(a, maximum_width(None));
    dome.set_constraint(a, minimum_width(Some(40.0)));
    dome.set_constraint(b, maximum_width(Some(20.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 29,0 42x10
      b 39,10 22x10 highlighted spawn=right
                                 +----------------------------------------+                             
                                 |                                        |                             
                                 |                                        |                             
                                 |                                        |                             
                                 |                    a                   |                             
                                 |                                        |                             
                                 |                                        |                             
                                 |                                        |                             
                                 |                                        |                             
                                 +----------------------------------------+                             
                                           **********************                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *          b         *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           *                    *                                       
                                           **********************
    ");
}

#[test]
fn set_constraint_with_minimum_height_past_work_area_extends_window_below_screen() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_height(Some(30.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x32 highlighted spawn=right off-screen
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *
    ");
}

#[test]
fn update_monitor_with_narrower_work_area_rescales_percentage_columns() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.update_monitor(
        dome.primary_monitor(),
        monitor("primary", [0, 0, 60, 20]),
        None,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 60x20 border 1
      w0 0,0 30x20
      w1 30,0 30x20 highlighted spawn=right
    +----------------------------+******************************
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |             w0             |*             w1             *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    +----------------------------+******************************
    ");
}

#[test]
fn focus_left_or_right_moves_to_neighbor_column_and_stops_at_row_edge() {
    let mut dome = TestDome::new(SCROLLING);
    for title in ["w0", "w1", "w2"] {
        dome.open(window(title));
    }
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x20 highlighted spawn=right
      w2 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       w1                       *|                       w2                       |
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
    dome.run("focus left");
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
    dome.run("focus right");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 0,0 50x20
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

    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("a"));
    let b = dome.open(window("b"));
    let c = dome.open(window("c"));
    dome.focus(b);
    dome.focus(c);
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      b 0,10 50x10 highlighted spawn=right
      c 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        a                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------+|                        c                       |
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
fn focus_up_or_down_moves_within_stack_and_scrolls_window_into_view() {
    let mut dome = TestDome::new(SCROLLING);
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
    for command in ["focus up", "focus down"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }

    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' }, { title = 'c' } } } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    for title in ["a", "b", "c"] {
        dome.open(window(title));
    }
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7
      b 25,7 50x6 highlighted spawn=right
      c 25,13 50x7
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        c                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.run("focus up");
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7 highlighted spawn=right
      b 25,7 50x6
      c 25,13 50x7
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        c                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.run("focus down");
    dome.run("focus down");
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7
      b 25,7 50x6
      c 25,13 50x7 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");

    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.set_constraint(a, minimum_height(Some(12.0)));
    dome.set_constraint(b, minimum_height(Some(12.0)));
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x14 highlighted spawn=right
      b 25,14 50x14 off-screen
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |
    ");
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,-8 50x14 off-screen
      b 25,6 50x14 highlighted spawn=right
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
}

#[test]
fn move_left_or_right_swaps_columns_or_takes_stacked_window_out() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("move left");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x20
      w1 0,0 50x20 highlighted spawn=right
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
    dome.run("move left");
    assert_eq!(dome.snapshot(), before);
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

    let layout =
        workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    dome.open(window("b"));
    dome.run("move right");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 20x20
      b 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         a        |*         b        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");

    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    dome.open(window("b"));
    dome.run("move left");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 20x20
      b 30,0 20x20 highlighted spawn=right
                                  ********************+------------------+                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *         b        *|         a        |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  ********************+------------------+
    ");
}

#[test]
fn move_up_or_down_reorders_stack_and_stops_at_column_edge() {
    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' }, { title = 'c' } } } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    for title in ["a", "b", "c"] {
        dome.open(window(title));
    }
    dome.run("move up");
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7
      b 25,13 50x7
      c 25,7 50x6 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.run("move up");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,7 50x6
      b 25,13 50x7
      c 25,0 50x7 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.run("move up");
    assert_eq!(dome.snapshot(), before);
    dome.run("move down");
    dome.run("move down");
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x7
      b 25,7 50x6
      c 25,13 50x7 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                        b                       |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");

    let mut dome = TestDome::new(SCROLLING);
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
    for command in ["move up", "move down"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
}

#[test]
fn focus_toward_hidden_part_reveals_it_before_moving_focus() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_height(Some(30.0)));
    dome.run("focus down");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,-12 50x32 highlighted spawn=right off-screen
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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
    dome.run("focus down");
    assert_eq!(dome.snapshot(), before);
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x32 highlighted spawn=right off-screen
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *
    ");

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_height(Some(50.0)));
    dome.run("focus down");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,-20 50x52 highlighted spawn=right off-screen
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *
    ");
    dome.run("focus down");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,-32 50x52 highlighted spawn=right off-screen
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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
    dome.run("focus down");
    assert_eq!(dome.snapshot(), before);
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,-12 50x52 highlighted spawn=right off-screen
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *
    ");
    dome.run("focus up");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x52 highlighted spawn=right off-screen
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *
    ");

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    dome.open(window("w1"));
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -102,0 152x20 highlighted spawn=right off-screen
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                            w0                       *|                       w1                       |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -2,0 152x20 highlighted spawn=right off-screen
    ****************************************************************************************************
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                     w0                                                 
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
    ****************************************************************************************************
    ");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -52,0 152x20 highlighted spawn=right off-screen
    ****************************************************************************************************
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                     w0                                                *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
    ****************************************************************************************************
    ");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -102,0 152x20 off-screen
      w1 50,0 50x20 highlighted spawn=right
    -------------------------------------------------+**************************************************
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                            w0                       |*                       w1                       *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
                                                     |*                                                *
    -------------------------------------------------+**************************************************
    ");
}

#[test]
fn focus_on_column_out_of_view_scrolls_least_distance_to_show_it() {
    let mut dome = TestDome::new(SCROLLING_40_PERCENT);
    let w0 = dome.open(window("w0"));
    for title in ["w1", "w2", "w3"] {
        dome.open(window(title));
    }
    dome.focus(w0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 40x20 highlighted spawn=right
      w1 40,0 40x20
      w2 80,0 40x20 off-screen
    ****************************************+--------------------------------------++-------------------
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                  w0                  *|                  w1                  ||        w2         
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    *                                      *|                                      ||                   
    ****************************************+--------------------------------------++-------------------
    ");
    dome.run("focus right");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -20,0 40x20 off-screen
      w1 20,0 40x20
      w2 60,0 40x20 highlighted spawn=right
    -------------------++--------------------------------------+****************************************
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
             w0        ||                  w1                  |*                  w2                  *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
                       ||                                      |*                                      *
    -------------------++--------------------------------------+****************************************
    ");

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    let w1 = dome.open(window("w1"));
    dome.set_constraint(w1, minimum_width(Some(98.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
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
    ");
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -52,0 152x20 highlighted spawn=right off-screen
    ****************************************************************************************************
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                     w0                                                *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
    ****************************************************************************************************
    ");

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    let w1 = dome.open(window("w1"));
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -102,0 152x20 highlighted spawn=right off-screen
      w1 50,0 50x20
    **************************************************+------------------------------------------------+
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                            w0                       *|                       w1                       |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
                                                     *|                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus left");
    dome.run("focus right");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -52,0 152x20 highlighted spawn=right off-screen
    ****************************************************************************************************
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                     w0                                                *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
                                                                                                       *
    ****************************************************************************************************
    ");
    dome.set_constraint(w1, LimitObservation::default());
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn close_or_narrow_column_clamps_scroll_offsets() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.close(w2);
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

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    dome.open(window("w1"));
    dome.set_constraint(w0, minimum_width(None));
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

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(150.0)));
    dome.run("focus right");
    dome.close(w0);
    dome.open(window("w1"));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w1 25,0 50x20 highlighted spawn=right
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

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_height(Some(30.0)));
    dome.run("focus down");
    dome.set_constraint(w0, minimum_height(None));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.set_constraint(w0, minimum_height(Some(30.0)));
    dome.set_constraint(w1, minimum_height(Some(30.0)));
    dome.run("focus down");
    dome.run("focus left");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x32 highlighted spawn=right off-screen
      w1 50,-12 50x32 off-screen
    **************************************************|                                                |
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
    *                                                *+------------------------------------------------+
    ");
}

#[test]
fn close_focused_window_focuses_window_taking_its_place() {
    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w0);
    dome.focus(w1);
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

    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' }, { title = 'c' } } } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    let c = dome.open(window("c"));
    dome.focus(a);
    dome.focus(b);
    dome.close(b);
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x10
      c 25,10 50x10 highlighted spawn=right
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                        a                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        c                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.close(c);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
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
}

#[test]
fn focus_parent_selects_column_and_blocks_window_verbs() {
    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.open(window("c"));
    dome.focus(b);
    dome.run("focus parent");
    let selected = dome.snapshot();
    assert_snapshot!(selected, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10
      b 0,10 50x10
      c 50,0 50x20
      container 0,0 50x20 highlighted spawn=bottom
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *------------------------------------------------*|                        c                       |
    *------------------------------------------------*|                                                |
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
    for command in [
        "focus parent",
        "focus up",
        "focus down",
        "move up",
        "move down",
        "toggle spawn",
        "toggle float",
        "toggle fullscreen",
        "close",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), selected, "{command}");
    }
    dome.focus(a);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 50x10 highlighted spawn=right
      b 0,10 50x10
      c 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************|                        c                       |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        b                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("close");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    effects: close a
    primary 0,0 100x20 border 1
      a 0,0 50x10 highlighted spawn=right
      b 0,10 50x10
      c 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                        a                       *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    **************************************************|                        c                       |
    +------------------------------------------------+|                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                        b                       ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn move_focus_or_resize_on_selected_column_acts_on_whole_column() {
    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.open(window("c"));
    dome.focus(b);
    dome.run("focus parent");
    dome.run("move right");
    let at_right = dome.snapshot();
    assert_snapshot!(at_right, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 20x10
      b 50,10 20x10
      c 30,0 20x20
      container 50,0 20x20 highlighted spawn=bottom
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*         a        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         c        |*------------------*                              
                                  |                  |*------------------*                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*         b        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");
    dome.run("move right");
    assert_eq!(dome.snapshot(), at_right);
    dome.run("move left");
    let at_left = dome.snapshot();
    assert_snapshot!(at_left, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 20x10
      b 30,10 20x10
      c 50,0 20x20
      container 30,0 20x20 highlighted spawn=bottom
                                  ********************+------------------+                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *         a        *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *------------------*|         c        |                              
                                  *------------------*|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *         b        *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  ********************+------------------+
    ");
    dome.run("focus left");
    assert_eq!(dome.snapshot(), at_left);
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 28,0 25x10
      b 28,10 25x10
      c 53,0 20x20
      container 28,0 25x20 highlighted spawn=bottom
                                *************************+------------------+                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *            a          *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *-----------------------*|         c        |                           
                                *-----------------------*|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *            b          *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *                       *|                  |                           
                                *************************+------------------+
    ");
    dome.run("shrink");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 20x10
      b 30,10 20x10
      c 50,0 20x20
      container 30,0 20x20 highlighted spawn=bottom
                                  ********************+------------------+                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *         a        *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *------------------*|         c        |                              
                                  *------------------*|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *         b        *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  ********************+------------------+
    ");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 20x10
      b 30,10 20x10
      c 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         a        |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+*         c        *                              
                                  +------------------+*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         b        |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");

    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.open(window("c"));
    dome.focus(b);
    dome.set_constraint(a, minimum_width(Some(150.0)));
    dome.set_constraint(b, maximum_width(Some(20.0)));
    dome.run("focus parent");
    dome.run("focus left");
    let revealed = dome.snapshot();
    assert_snapshot!(revealed, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 152x10 off-screen
      b 65,10 22x10
      container 0,0 152x20 highlighted spawn=bottom
    ****************************************************************************************************
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                 a                                                 
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *                                                                                                   
    *---------------------------------------------------------------------------------------------------
    *                                                                +--------------------+             
    *                                                                |                    |             
    *                                                                |                    |             
    *                                                                |                    |             
    *                                                                |          b         |             
    *                                                                |                    |             
    *                                                                |                    |             
    *                                                                |                    |             
    *                                                                |                    |             
    ****************************************************************************************************
    ");
    dome.run("focus left");
    assert_eq!(dome.snapshot(), revealed);
}

#[test]
fn open_while_column_is_selected_joins_bottom_of_column() {
    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("focus parent");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x7
      w1 40,7 20x6
      w2 40,13 20x7 highlighted spawn=bottom
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |        w0        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |        w1        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w2        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.focus(w0);
    dome.run("focus parent");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x7
      w1 40,7 20x6
      w2 40,13 20x7 highlighted spawn=right
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |        w0        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |        w1        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w2        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let layout = workspace_0_layout("columns = { { title = 'a' }, { title = 'b' } }");
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("b"));
    dome.open(window("u"));
    dome.run("focus parent");
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 20,0 20x20 highlighted spawn=right
      b 40,0 20x20
      u 60,0 20x20
                        ********************+------------------++------------------+                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *         a        *|         b        ||         u        |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        *                  *|                  ||                  |                    
                        ********************+------------------++------------------+
    ");

    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    let a = dome.open(window("a"));
    dome.open(window("b"));
    let c = dome.open(window("c"));
    dome.run("toggle float");
    dome.focus(a);
    dome.run("focus parent");
    dome.focus(c);
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x7
      b 40,7 20x6
      c 40,13 20x7 highlighted spawn=right
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |         b        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         c        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
}

#[test]
fn focus_float_or_close_window_keeps_column_selection_until_column_empties() {
    let layout = workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    let b = dome.open(window("b"));
    let c = dome.open(window("c"));
    dome.run("toggle float");
    dome.focus(b);
    dome.run("focus parent");
    dome.focus(c);
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10
      c 50,0 20x20 float highlighted
                                            +---------********************                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            +---------*         c        *                              
                                            +---------*                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            |         *                  *                              
                                            +---------********************
    ");
    dome.close(c);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10
      container 40,0 20x20 highlighted spawn=bottom
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         a        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *------------------*                                        
                                            *------------------*                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         b        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
    dome.close(b);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x20
      container 40,0 20x20 highlighted spawn=bottom
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         a        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    dome.open(window("b"));
    let c = dome.open(window("c"));
    dome.focus(c);
    dome.run("focus parent");
    dome.close(c);
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10 highlighted spawn=right
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         b        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
}

#[test]
fn move_tree_container_to_scrolling_workspace_flattens_into_selected_column() {
    let mut dome = TestDome::with_layout(
        "return { scrolling = { column_width = '20%' } }",
        "return { primary = { ['1'] = { layout = 'scrolling' } } }",
    );
    dome.run("focus workspace 1");
    let w4 = dome.open(window("w4"));
    dome.open(window("w5"));
    dome.focus(w4);
    dome.run("focus workspace 0");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.run("focus parent");
    dome.run("focus parent");
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 on primary
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

    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w1 40,0 20x7
      w2 40,7 20x6
      w3 40,13 20x7
      w4 20,0 20x20
      w5 60,0 20x20
      container 40,0 20x20 highlighted spawn=bottom
                        +------------------+********************+------------------+                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*        w1        *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*------------------*|                  |                    
                        |                  |*------------------*|                  |                    
                        |                  |*                  *|                  |                    
                        |        w4        |*        w2        *|        w5        |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*------------------*|                  |                    
                        |                  |*------------------*|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*        w3        *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        +------------------+********************+------------------+
    ");
}

#[test]
fn move_workspace_with_column_selected_takes_column_whole() {
    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.run("focus workspace 1");
    dome.open(window("w0"));
    dome.run("focus workspace 0");
    dome.open(window("w1"));
    dome.run("toggle spawn");
    let w2 = dome.open(window("w2"));
    dome.run("toggle spawn");
    dome.open(window("w3"));
    dome.focus(w2);
    dome.run("grow");
    dome.run("focus parent");
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w3 40,0 20x20 highlighted spawn=right
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w3        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 30,0 20x20
      w1 50,0 20x10
      w2 50,10 20x10
      container 50,0 20x20 highlighted spawn=bottom
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*        w1        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |        w0        |*------------------*                              
                                  |                  |*------------------*                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*        w2        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.focus(w0);
    dome.focus(w1);
    dome.run("focus parent");
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 30,0 20x20
      w2 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |        w0        |*        w2        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.run("focus workspace 1");
    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("focus workspace 0");
    dome.open(window("a"));
    dome.run("toggle spawn");
    dome.open(window("b"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10
      w0 20,0 20x20
      w1 60,0 20x20
      container 40,0 20x20 highlighted spawn=bottom
                        +------------------+********************+------------------+                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*         a        *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |        w0        |*------------------*|        w1        |                    
                        |                  |*------------------*|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*         b        *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        +------------------+********************+------------------+
    ");
    dome.run("focus left");
    dome.run("focus right");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10 highlighted spawn=right
      b 40,10 20x10
      w0 20,0 20x20
      w1 60,0 20x20
                        +------------------+********************+------------------+                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*         a        *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |        w0        |********************|        w1        |                    
                        |                  |+------------------+|                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||         b        ||                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||                  ||                  |                    
                        |                  ||                  ||                  |                    
                        +------------------++------------------++------------------+
    ");

    let mut dome = TestDome::with_layout(
        "return { scrolling = { column_width = '20%' } }",
        "return { primary = { ['1'] = { layout = 'scrolling' } } }",
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle spawn");
    dome.open(window("w2"));
    dome.run("focus parent");
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x7
      w1 40,7 20x6
      w2 40,13 20x7
      container 40,0 20x20 highlighted spawn=bottom
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w0        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *------------------*                                        
                                            *------------------*                                        
                                            *                  *                                        
                                            *        w1        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *------------------*                                        
                                            *------------------*                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w2        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
}

#[test]
fn grow_or_shrink_resizes_focused_column_by_five_percent_within_bounds() {
    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("grow");
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 20x20
      w1 45,0 30x20 highlighted spawn=right
                             +------------------+******************************                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |        w0        |*             w1             *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             +------------------+******************************
    ");
    for _ in 0..4 {
        dome.run("shrink");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 35,0 20x20
      w1 55,0 10x20 highlighted spawn=right
                                       +------------------+**********                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |        w0        |*   w1   *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       +------------------+**********
    ");

    let mut dome =
        TestDome::new("return { layout = 'scrolling', scrolling = { column_width = '90%' } }");
    dome.open(window("w0"));
    for _ in 0..3 {
        dome.run("grow");
    }
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

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, maximum_width(Some(38.0)));
    for _ in 0..5 {
        dome.run("grow");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 30,0 40x20 highlighted spawn=right
                                  ****************************************                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                  w0                  *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  ****************************************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(13.0)));
    for _ in 0..3 {
        dome.run("shrink");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 43,0 15x20 highlighted spawn=right
                                               ***************                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *      w0     *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               *             *                                          
                                               ***************
    ");

    let mut dome = TestDome::new(
        "return { layout = 'scrolling', minimum_width = 0, scrolling = { column_width = '20%' } }",
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    for _ in 0..5 {
        dome.run("shrink");
    }
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x20
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |        w0        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+
    ");
    dome.run("grow");
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 35,0 20x20
      w1 55,0 10x20 highlighted spawn=right
                                       +------------------+**********                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |        w0        |*   w1   *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       |                  |*        *                                   
                                       +------------------+**********
    ");

    let mut dome =
        TestDome::new("return { layout = 'scrolling', scrolling = { column_width = '80%' } }");
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, maximum_width(Some(43.0)));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 28,0 45x20 highlighted spawn=right
                                *********************************************                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                     w0                    *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *                                           *                           
                                *********************************************
    ");
    dome.run("shrink");
    dome.run("shrink");
    dome.set_constraint(w0, maximum_width(None));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 33,0 35x20 highlighted spawn=right
                                     ***********************************                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                w0               *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     *                                 *                                
                                     ***********************************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_width(Some(148.0)));
    dome.run("grow");
    dome.set_constraint(w0, minimum_width(None));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x20 highlighted spawn=right
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w0        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    dome.run("toggle spawn");
    dome.open(window("w1"));
    dome.run("grow");
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 35,0 30x10
      w1 35,10 30x10 highlighted spawn=bottom
                                       +----------------------------+                                   
                                       |                            |                                   
                                       |                            |                                   
                                       |                            |                                   
                                       |             w0             |                                   
                                       |                            |                                   
                                       |                            |                                   
                                       |                            |                                   
                                       |                            |                                   
                                       +----------------------------+                                   
                                       ******************************                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *             w1             *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       ******************************
    ");

    let mut dome = TestDome::new(SCROLLING_40_PERCENT);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("grow");
    dome.run("grow");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 -30,0 40x20 off-screen
      w1 10,0 40x20
      w2 50,0 50x20 highlighted spawn=right
    ---------++--------------------------------------+**************************************************
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
        w0   ||                  w1                  |*                       w2                       *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
             ||                                      |*                                                *
    ---------++--------------------------------------+**************************************************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    dome.run("grow");
    dome.run("grow");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "scrolling",
          columns = {
            { width = "30%", children = {
              { title = "w0" },
            } },
          },
        },
      },
    }
    "#);

    let layout = workspace_0_layout("float = { { title = 'f' } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("w0"));
    dome.open(window("f").frame(FLOAT_FRAME));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      w0 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *              f             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************         w0                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    for command in ["grow", "shrink"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }

    let mut dome =
        TestDome::new("return { layout = 'scrolling', scrolling = { column_width = 60 } }");
    dome.add_monitor(monitor("empty", [100, 0, 0, 20]));
    dome.run("focus monitor empty");
    dome.open(window("w0"));
    dome.run("shrink");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["empty"] = {
        ["0"] = {
          layout = "scrolling",
          columns = {
            { width = 60, children = {
              { title = "w0" },
            } },
          },
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "scrolling",
        },
      },
    }
    "#);
}

#[test]
fn reload_with_new_column_width_resizes_columns_on_workspaces_layout_does_not_name() {
    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("grow");
    dome.run("grow");
    dome.reload(SCROLLING_40_PERCENT);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 10,0 40x20
      w1 50,0 40x20 highlighted spawn=right
              +--------------------------------------+****************************************          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                  w0                  |*                  w1                  *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              +--------------------------------------+****************************************
    ");

    let mut dome = TestDome::new(SCROLLING_20_PERCENT);
    dome.open(window("w0"));
    dome.run("grow");
    dome.run("grow");
    dome.reload(SCROLLING_20_PERCENT);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 40,0 20x20 highlighted spawn=right
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        w0        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let layout = workspace_0_layout("columns = { { title = 'a' } }");
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    dome.open(window("u"));
    dome.run("grow");
    dome.run("grow");
    dome.reload(SCROLLING_40_PERCENT);
    assert_snapshot!(dome.snapshot(), @"
    focused u on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 20x20
      u 45,0 30x20 highlighted spawn=right
                             +------------------+******************************                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |         a        |*              u             *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             |                  |*                            *                         
                             +------------------+******************************
    ");

    let layout =
        workspace_0_layout("columns = { { width = '40%', children = { { title = 'a' } } } }");
    let mut dome = TestDome::with_layout(SCROLLING_20_PERCENT, &layout);
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 40x20 highlighted spawn=right
                                  ****************************************                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                   a                  *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  *                                      *                              
                                  ****************************************
    ");
    dome.apply_layout("return {}");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x20 highlighted spawn=right
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         a        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
    dome.reload("return { layout = 'scrolling', scrolling = { column_width = '30%' } }");
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 35,0 30x20 highlighted spawn=right
                                       ******************************                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *                            *                                   
                                       *              a             *                                   
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
}

#[test]
fn run_verb_without_scrolling_meaning_changes_nothing() {
    let mut dome = TestDome::new(SCROLLING);
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
    for command in [
        "toggle direction",
        "toggle layout",
        "focus tab next",
        "focus tab prev",
        "master more",
        "master fewer",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
    dome.click_tab(container(0), 0);
    assert_eq!(dome.snapshot(), before);

    let mut dome = TestDome::new(SCROLLING);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
    ");
    for command in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
        "toggle spawn",
        "focus parent",
        "grow",
        "shrink",
        "toggle float",
        "toggle fullscreen",
        "toggle direction",
        "toggle layout",
        "focus tab next",
        "master more",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }

    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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
    for command in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }

    let layout = workspace_0_layout("float = { { title = 'f' } }");
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("f").frame(FLOAT_FRAME));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
    for command in [
        "focus left",
        "focus right",
        "focus up",
        "focus down",
        "move left",
        "move right",
        "move up",
        "move down",
        "toggle spawn",
        "focus parent",
        "grow",
        "shrink",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
}

#[test]
fn run_tiling_verb_while_fullscreen_has_focus_changes_nothing() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle fullscreen");
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
    for command in [
        "focus left",
        "move left",
        "toggle spawn",
        "toggle direction",
        "toggle layout",
        "focus parent",
        "toggle float",
        "grow",
        "shrink",
    ] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before, "{command}");
    }
}

#[test]
fn toggle_float_floats_window_at_shown_tile_rect_with_focus() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,0 50x20
      w1 50,0 50x20 float highlighted
                             +------------------------**************************************************
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                       w*                       w1                       *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             +------------------------**************************************************
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

    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20
      w2 50,0 50x20 float highlighted
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

    let mut dome = TestDome::new(SCROLLING);
    let w0 = dome.open(window("w0"));
    dome.set_constraint(w0, minimum_height(Some(30.0)));
    dome.run("focus down");
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      w0 25,-12 50x32 float highlighted
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       w0                       *                         
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

    let mut dome = TestDome::with_layout(
        SCROLLING,
        &workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }"),
    );
    let a = dome.open(window("a"));
    let b = dome.open(window("b"));
    dome.set_constraint(a, minimum_height(Some(12.0)));
    dome.set_constraint(b, minimum_height(Some(12.0)));
    dome.run("toggle float");
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x20
      b 25,6 50x14 float highlighted
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        b                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    let mut dome = TestDome::new(SCROLLING);
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
      w0 175,0 50x20
      w1 200,0 50x20 float highlighted
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
    
                             +------------------------**************************************************
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                       w*                       w1                       *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             |                        *                                                *
                             +------------------------**************************************************
    ");
}

#[test]
fn move_float_takes_reported_frame_and_keeps_stack_order_and_focus() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f1 = dome.open(window("f1").frame(FLOAT_FRAME));
    dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    dome.move_float(f1, [30, 8, 20, 6], dome.primary_monitor());
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 29,7 22x8 float
      f2 20,6 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                        ******************************                        |                         
                        *                            *+                       |                         
                        *                            *|                       |                         
                        *             f2             *|                       |                         
                        *                            *|                       |                         
                        *                            *|                       |                         
                        *                            *|                       |                         
                        ******************************|                       |                         
                             |   +--------------------+                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
}

#[test]
fn move_float_on_window_no_longer_floating_changes_nothing() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.run("toggle float");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 50,0 50x20 highlighted spawn=right
      t 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        t                       |*                        f                       *
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
    dome.move_float(f, [50, 4, 20, 6], dome.primary_monitor());
    assert_eq!(dome.snapshot(), before);

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.set_fullscreen(f, WindowRestrictions::None);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 f                                                |
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
    dome.move_float(f, [50, 4, 20, 6], dome.primary_monitor());
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn open_float_whose_border_fills_frame_places_nothing() {
    let mut dome = TestDome::with_layout(
        "return { layout = 'scrolling', border_size = 4 }",
        &workspace_0_layout("float = { { title = 'f' }, { title = 'g' } }"),
    );
    dome.open(window("f").frame([10, 2, 8, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 4
    ");
    dome.open(window("g").frame([40, 2, 10, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused g on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 4
      g 40,2 10x10 float highlighted
                                                                                                        
                                                                                                        
                                            **********                                                  
                                            *        *                                                  
                                            *        *                                                  
                                            *        *                                                  
                                            *    g   *                                                  
                                            *        *                                                  
                                            *        *                                                  
                                            *        *                                                  
                                            *        *                                                  
                                            **********
    ");
}

#[test]
fn set_fullscreen_on_tiling_or_float_covers_monitor() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t0 = dome.open(window("t0"));
    dome.open(window("t1"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.set_fullscreen(t0, WindowRestrictions::ProtectFullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused t0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen t0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                t0                                                |
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

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t0"));
    dome.open(window("t1"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.set_fullscreen(f, WindowRestrictions::ProtectFullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 f                                                |
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
fn unset_fullscreen_tiles_window_by_spawn_rules_and_ignores_others() {
    let mut dome = TestDome::new(SCROLLING);
    let t0 = dome.open(window("t0"));
    let t1 = dome.open(window("t1"));
    dome.focus(t0);
    dome.run("toggle spawn");
    dome.set_fullscreen(t1, WindowRestrictions::BlockAll);
    assert_snapshot!(dome.snapshot(), @"
    focused t1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen t1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                t1                                                |
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
    dome.unset_fullscreen(t1);
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused t1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t0 25,0 50x10
      t1 25,10 50x10 highlighted spawn=bottom
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                       t0                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+                         
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                       t1                       *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             **************************************************
    ");
    dome.unset_fullscreen(t0);
    assert_eq!(dome.snapshot(), before);

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *              f             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************          t                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.unset_fullscreen(f);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn set_fullscreen_on_two_windows_shows_only_topmost() {
    let mut dome = TestDome::new(SCROLLING);
    let t0 = dome.open(window("t0"));
    let t1 = dome.open(window("t1"));
    dome.open(window("t2"));
    dome.set_fullscreen(t0, WindowRestrictions::None);
    dome.set_fullscreen(t1, WindowRestrictions::None);
    assert_snapshot!(dome.snapshot(), @"
    focused t1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen t1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                t1                                                |
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
fn set_fullscreen_on_fullscreen_window_changes_nothing_visible() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let fs1 = dome.open(window("fs1"));
    let fs2 = dome.open(window("fs2"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused fs2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs2
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs2                                               |
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
    dome.set_fullscreen(fs1, WindowRestrictions::BlockAll);
    assert_eq!(dome.snapshot(), before);
    dome.set_fullscreen(fs2, WindowRestrictions::ProtectFullscreen);
    assert_eq!(dome.snapshot(), before);
    dome.close(fs2);
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs1                                               |
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
fn toggle_fullscreen_covers_monitor_and_toggling_back_tiles_by_spawn_rules() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t0 = dome.open(window("t0"));
    dome.open(window("t1"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.focus(t0);
    dome.run("toggle fullscreen");
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused t0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen t0
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                t0                                                |
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
    dome.focus(f);
    assert_eq!(dome.snapshot(), before);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused t0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t0 50,0 50x20 highlighted spawn=right
      t1 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |              f             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*                       t0                       *
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
    dome.focus(f);
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen f
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 f                                                |
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
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 50,0 50x20 highlighted spawn=right
      t0 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       t0                       |*                        f                       *
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
fn open_while_fullscreen_has_focus_keeps_fullscreen_covering_and_focused() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let fullscreen = dome.open(window("fs"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    dome.open(window("u"));
    assert_eq!(dome.snapshot(), before);
    dome.open(window("f").frame(FLOAT_FRAME));
    assert_eq!(dome.snapshot(), before);
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      t 0,0 50x20
      u 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         ******************************         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *              f             *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         ******************************         ||                        u                       |
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
fn focus_window_moves_highlight_between_tiling_and_float() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t0 = dome.open(window("t0"));
    let t1 = dome.open(window("t1"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.focus(t0);
    assert_snapshot!(dome.snapshot(), @"
    focused t0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t0 0,0 50x20 highlighted spawn=right
      t1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *         +----------------------------+         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         |              f             |         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         |                            |         *|                                                |
    *         +----------------------------+         *|                       t1                       |
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
    dome.focus(f);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      t0 0,0 50x20
      t1 50,0 50x20
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |         ******************************         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *              f             *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         *                            *         ||                                                |
    |         ******************************         ||                       t1                       |
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
    dome.focus(t1);
    assert_snapshot!(dome.snapshot(), @"
    focused t1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t0 0,0 50x20
      t1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |              f             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*                       t1                       *
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
fn focus_covered_float_raises_it_above_other_floats() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f1 = dome.open(window("f1").frame(FLOAT_FRAME));
    dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    dome.focus(f1);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      f2 20,6 30x8 float
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *             f1             *                                  |                         
              *                            *---------+                        |                         
              *                            *         |                        |                         
              *                            *         |                        |                         
              ******************************         |t                       |                         
                        |                            |                        |                         
                        |                            |                        |                         
                        |                            |                        |                         
                        +----------------------------+                        |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
}

#[test]
fn focus_covered_fullscreen_window_brings_it_to_front() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let fs1 = dome.open(window("fs1"));
    dome.open(window("fs2"));
    dome.focus(fs1);
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs1                                               |
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
fn focus_tiling_window_under_fullscreen_window_takes_focus_and_scrolls_into_view_once_it_closes() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t0 = dome.open(window("t0"));
    dome.open(window("t1"));
    dome.open(window("t2"));
    let fs = dome.open(window("fs"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    dome.focus(t0);
    assert_eq!(dome.snapshot(), before);
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused t0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t0 0,0 50x20 highlighted spawn=right
      t1 50,0 50x20
    **************************************************+------------------------------------------------+
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                                                *|                                                |
    *                       t0                       *|                       t1                       |
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
fn toggle_or_close_topmost_fullscreen_window_shows_one_below() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let fs1 = dome.open(window("fs1"));
    dome.open(window("fs2"));
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs1                                               |
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
    dome.close(fs1);
    assert_snapshot!(dome.snapshot(), @"
    focused fs2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      fs2 50,0 50x20 highlighted spawn=right
      t 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        t                       |*                       fs2                      *
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

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    dome.open(window("fs1"));
    let fs2 = dome.open(window("fs2"));
    dome.close(fs2);
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs1                                               |
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
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let fs1 = dome.open(window("fs1"));
    let fs2 = dome.open(window("fs2"));
    dome.unset_fullscreen(fs1);
    assert_snapshot!(dome.snapshot(), @"
    focused fs2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs2
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs2                                               |
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
    dome.close(fs2);
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      fs1 50,0 50x20 highlighted spawn=right
      t 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        t                       |*                       fs1                      *
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

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f1 = dome.open(window("f1").frame(FLOAT_FRAME));
    dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    dome.close(f1);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f2 20,6 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                        ******************************                        |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        *             f2             *t                       |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        ******************************                        |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let fs1 = dome.open(window("fs1"));
    dome.open(window("fs2"));
    dome.close(fs1);
    assert_snapshot!(dome.snapshot(), @"
    focused fs2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs2
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs2                                               |
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
fn close_minimize_or_move_last_fullscreen_without_tiling_window_focuses_float() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("f").frame(FLOAT_FRAME));
    let fs = dome.open(window("fs"));
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("f").frame(FLOAT_FRAME));
    let fs = dome.open(window("fs"));
    dome.minimize(fs);
    assert_snapshot!(dome.snapshot(), @r#"
    focused f on primary
    workspaces: 0 on primary (focused)
    minimized: fs "fs"
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    "#);

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.open(window("fs"));
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn close_only_fullscreen_window_focuses_tiling_window_over_float() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    let fs = dome.open(window("fs"));
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused t on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
              +----------------------------+                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |              f             |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              +----------------------------+          t                       *                         
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
}

#[test]
fn toggle_fullscreen_off_after_float_click_keeps_focus_on_window() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    dome.open(window("fs"));
    dome.focus(f);
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      fs 50,0 50x20 highlighted spawn=right
      t 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |              f             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*                       fs                       *
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
fn close_window_app_made_fullscreen_returns_focus_to_float() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t0"));
    let t1 = dome.open(window("t1"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.set_fullscreen(t1, WindowRestrictions::ProtectFullscreen);
    dome.close(t1);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      t0 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *              f             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************         t0                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
}

#[test]
fn close_focused_float_focuses_next_float_then_tiling_window() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f1 = dome.open(window("f1").frame(FLOAT_FRAME));
    let f2 = dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    dome.close(f2);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *             f1             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************          t                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.close(f1);
    assert_snapshot!(dome.snapshot(), @"
    focused t on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        t                       *                         
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
}

#[test]
fn close_last_tiling_window_focuses_float() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t = dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.focus(t);
    dome.close(t);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn minimize_last_tiling_window_leaves_float_focused() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t = dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.focus(t);
    dome.minimize(t);
    assert_snapshot!(dome.snapshot(), @r#"
    focused f on primary
    workspaces: 0 on primary (focused)
    minimized: t "t"
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    "#);

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    let t = dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.minimize(t);
    assert_snapshot!(dome.snapshot(), @r#"
    focused f on primary
    workspaces: 0 on primary (focused)
    minimized: t "t"
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    "#);
}

#[test]
fn move_fullscreen_window_to_workspace_clears_float_focus_there() {
    let layout = "return { primary = {
        ['0'] = { layout = 'scrolling', fullscreen = { { title = 'fs' } } },
        ['1'] = { layout = 'scrolling', float = { { title = 'f' } } },
    } }";
    let mut dome = TestDome::with_layout(SCROLLING, layout);
    dome.run("focus workspace 1");
    dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.run("focus workspace 0");
    let fs = dome.open(window("fs"));
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused t on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
              +----------------------------+                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |              f             |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              +----------------------------+          t                       *                         
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
}

#[test]
fn move_selected_column_to_workspace_takes_focus_from_float_there() {
    let layout = "return { primary = {
        ['1'] = { layout = 'scrolling', float = { { title = 'f' } } },
    } }";
    let mut dome = TestDome::with_layout(SCROLLING, layout);
    dome.run("focus workspace 1");
    dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.run("focus workspace 0");
    dome.open(window("a"));
    dome.run("toggle spawn");
    dome.open(window("b"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 50x10
      b 50,10 50x10
      f 10,2 30x8 float
      t 0,0 50x20
      container 50,0 50x20 highlighted spawn=bottom
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |         +----------------------------+         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                        a                       *
    |         |              f             |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         |                            |         |*                                                *
    |         +----------------------------+         |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                        b                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn move_selected_column_under_fullscreen_window_takes_focus_once_it_closes() {
    let layout = "return { primary = {
        ['1'] = { layout = 'scrolling', fullscreen = { { title = 'fs' } } },
    } }";
    let mut dome = TestDome::with_layout(SCROLLING, layout);
    dome.run("focus workspace 1");
    dome.open(window("t"));
    let fs = dome.open(window("fs"));
    dome.run("focus workspace 0");
    dome.open(window("a"));
    dome.run("toggle spawn");
    dome.open(window("b"));
    dome.run("focus parent");
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 50x10
      b 50,10 50x10
      t 0,0 50x20
      container 50,0 50x20 highlighted spawn=bottom
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                        a                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                        t                       |*------------------------------------------------*
    |                                                |*------------------------------------------------*
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                        b                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
}

#[test]
fn unminimize_fullscreen_window_clears_float_focus() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    let f = dome.open(window("f").frame(FLOAT_FRAME));
    let fs = dome.open(window("fs"));
    dome.minimize(fs);
    dome.focus(f);
    dome.unminimize(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    dome.close(fs);
    assert_snapshot!(dome.snapshot(), @"
    focused t on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float
      t 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
              +----------------------------+                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |              f             |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              |                            |                                  *                         
              +----------------------------+          t                       *                         
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
}

#[test]
fn apply_layout_focuses_topmost_fullscreen_then_float_then_tiling_window() {
    let layout = display_modes_layout();
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let t = dome.open(window("t"));
    let f1 = dome.open(window("f1").frame(FLOAT_FRAME));
    let f2 = dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    let fs1 = dome.open(window("fs1"));
    let fs2 = dome.open(window("fs2"));
    dome.focus(t);
    dome.apply_layout(&layout);
    assert_snapshot!(dome.snapshot(), @"
    focused fs2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs2
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs2                                               |
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
    dome.close(fs2);
    assert_snapshot!(dome.snapshot(), @"
    focused fs1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs1                                               |
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
    dome.close(fs1);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float
      f2 20,6 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              +----------------------------+                                  |                         
              |                            |                                  |                         
              |                            |                                  |                         
              |             f1             |                                  |                         
              |         ******************************                        |                         
              |         *                            *                        |                         
              |         *                            *                        |                         
              +---------*             f2             *t                       |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        ******************************                        |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.close(f2);
    assert_snapshot!(dome.snapshot(), @"
    focused f1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *             f1             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************          t                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
    dome.close(f1);
    assert_snapshot!(dome.snapshot(), @"
    focused t on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        t                       *                         
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

    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    let t = dome.open(window("t"));
    dome.open(window("f1").frame(FLOAT_FRAME));
    dome.open(window("f2").frame(OVERLAPPING_FLOAT_FRAME));
    dome.focus(t);
    dome.apply_layout(&layout);
    assert_snapshot!(dome.snapshot(), @"
    focused f2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f1 10,2 30x8 float
      f2 20,6 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              +----------------------------+                                  |                         
              |                            |                                  |                         
              |                            |                                  |                         
              |             f1             |                                  |                         
              |         ******************************                        |                         
              |         *                            *                        |                         
              |         *                            *                        |                         
              +---------*             f2             *t                       |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        *                            *                        |                         
                        ******************************                        |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
}

#[test]
fn open_matching_window_keeps_column_slot_order_through_moves_and_closes() {
    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { { children = { { title = 'a' }, { title = 'b' } } } }"),
    );
    let a = dome.open(window("a"));
    dome.open(window("b"));
    dome.run("move right");
    dome.close(a);
    dome.open(window("a").label("new_a"));
    assert_snapshot!(dome.snapshot(), @"
    focused new_a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      b 30,0 20x20
      new_a 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         b        |*       new_a      *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout(
            "columns = { { children = { { title = 'a' }, { title = 'b' }, { title = 'c' } } } }",
        ),
    );
    let a = dome.open(window("a"));
    let c = dome.open(window("c"));
    dome.focus(a);
    dome.run("toggle spawn");
    dome.focus(c);
    dome.run("toggle fullscreen");
    dome.run("toggle fullscreen");
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x7
      b 40,7 20x6 highlighted spawn=right
      c 40,13 20x7
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *         b        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         c        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { { title = 'a' }, { title = 'b' } }"),
    );
    let a = dome.open(window("a"));
    dome.run("toggle spawn");
    dome.open(window("u"));
    dome.close(a);
    dome.open(window("b"));
    dome.open(window("a").label("new_a"));
    assert_snapshot!(dome.snapshot(), @"
    focused new_a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      b 50,0 20x20
      new_a 30,10 20x10 highlighted spawn=right
      u 30,0 20x10
                                  +------------------++------------------+                              
                                  |                  ||                  |                              
                                  |                  ||                  |                              
                                  |                  ||                  |                              
                                  |         u        ||                  |                              
                                  |                  ||                  |                              
                                  |                  ||                  |                              
                                  |                  ||                  |                              
                                  |                  ||                  |                              
                                  +------------------+|         b        |                              
                                  ********************|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *       new_a      *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  *                  *|                  |                              
                                  ********************+------------------+
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout(
            "columns = { { children = { { title = 'a' }, { title = 'c' } } }, { title = 'b' } }",
        ),
    );
    dome.open(window("a"));
    dome.run("toggle spawn");
    let b = dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 20x20
      b 50,0 20x20 highlighted spawn=right
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         a        |*         b        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");
    dome.minimize(b);
    dome.unminimize(b);
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10 highlighted spawn=bottom
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         b        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x7
      b 40,7 20x6
      c 40,13 20x7 highlighted spawn=right
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |         b        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         c        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout(
            "columns = { { width = '40%', children = { { title = 't' }, { title = 't' } } } }",
        ),
    );
    dome.open(window("t").label("t1"));
    dome.open(window("t").label("t2"));
    dome.open(window("t").label("t3"));
    assert_snapshot!(dome.snapshot(), @"
    focused t3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t1 20,0 40x10
      t2 20,10 40x10
      t3 60,0 20x20 highlighted spawn=right
                        +--------------------------------------+********************                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                  t1                  |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        +--------------------------------------+*        t3        *                    
                        +--------------------------------------+*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                  t2                  |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        |                                      |*                  *                    
                        +--------------------------------------+********************
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout(
            "columns = { { width = '40%', children = { { title = 't' } } }, { width = '40%', children = { { title = 't' } } } }",
        ),
    );
    dome.open(window("t").label("t1"));
    dome.open(window("t").label("t2"));
    assert_snapshot!(dome.snapshot(), @"
    focused t2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t1 10,0 40x20
      t2 50,0 40x20 highlighted spawn=right
              +--------------------------------------+****************************************          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                  t1                  |*                  t2                  *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              +--------------------------------------+****************************************
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { { children = { { title = 't' }, { title = 't' } } } }"),
    );
    let first = dome.open(window("t").label("t1"));
    dome.open(window("t").label("t2"));
    dome.close(first);
    dome.open(window("t").label("t3"));
    assert_snapshot!(dome.snapshot(), @"
    focused t3 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t2 40,10 20x10
      t3 40,0 20x10 highlighted spawn=right
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *        t3        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************                                        
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |        t2        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+
    ");
}

#[test]
fn open_float_matched_window_floats_at_reported_frame_on_matcher_workspace() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("f").frame(FLOAT_FRAME));
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");

    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      t 25,0 50x20
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *              f             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************          t                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING,
        "return { primary = { ['1'] = { layout = 'scrolling', float = { { title = 'f' } } } } }",
    );
    dome.open(window("t"));
    dome.open(window("f").frame(FLOAT_FRAME));
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *              f             *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn open_fullscreen_matched_window_covers_matcher_workspace() {
    let mut dome = TestDome::with_layout(SCROLLING, &display_modes_layout());
    dome.open(window("t"));
    dome.open(window("fs"));
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
        SCROLLING,
        "return { primary = { ['1'] = { layout = 'scrolling', fullscreen = { { title = 'fs' } } } } }",
    );
    dome.open(window("t"));
    dome.open(window("fs"));
    assert_snapshot!(dome.snapshot(), @"
    focused fs on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
fn open_window_matching_fullscreen_and_float_goes_fullscreen() {
    let mut dome = TestDome::with_layout(
        SCROLLING,
        &workspace_0_layout("float = { { title = 'x' } }, fullscreen = { { title = 'x' } }"),
    );
    dome.open(window("t"));
    dome.open(window("x"));
    assert_snapshot!(dome.snapshot(), @"
    focused x on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen x
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                 x                                                |
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
fn open_window_matching_float_and_column_floats() {
    let mut dome = TestDome::with_layout(
        SCROLLING,
        &workspace_0_layout("columns = { { title = 'x' } }, float = { { title = 'x' } }"),
    );
    dome.open(window("t"));
    dome.open(window("x").frame(FLOAT_FRAME));
    assert_snapshot!(dome.snapshot(), @"
    focused x on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      t 25,0 50x20
      x 10,2 30x8 float highlighted
                             +------------------------------------------------+                         
                             |                                                |                         
              ******************************                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *              x             *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              *                            *                                  |                         
              ******************************          t                       |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             |                                                |                         
                             +------------------------------------------------+
    ");
}

#[test]
fn save_layout_writes_floats_and_fullscreens_from_live_titles() {
    let mut dome = TestDome::new(SCROLLING);
    dome.open(window("t"));
    dome.open(window("f"));
    dome.run("toggle float");
    dome.open(window("g"));
    dome.run("toggle fullscreen");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "scrolling",
          columns = {
            { width = "50%", children = {
              { title = "t" },
            } },
          },
          float = {
            { title = "f" },
          },
          fullscreen = {
            { title = "g" },
          },
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout(
        SCROLLING,
        &workspace_0_layout("float = { { title = '/^f/' } }, fullscreen = { { title = '/^g/' } }"),
    );
    dome.open(window("t"));
    dome.open(window("f1").frame(FLOAT_FRAME));
    dome.open(window("g1"));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "scrolling",
          columns = {
            { width = "50%", children = {
              { title = "t" },
            } },
          },
          float = {
            { title = "f1" },
          },
          fullscreen = {
            { title = "g1" },
          },
        },
      },
    }
    "#);
}

#[test]
fn open_column_matched_windows_fills_layout_columns_with_their_widths() {
    let layout = workspace_0_layout(
        "columns = { { width = '30%', children = { { title = 'a' } } }, { width = '20%', children = { { title = 'b' } } }, { title = 'c' } }",
    );
    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("a"));
    dome.open(window("b"));
    dome.open(window("c"));
    assert_snapshot!(dome.snapshot(), @"
    focused c on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 30x20
      b 30,0 20x20
      c 50,0 50x20 highlighted spawn=right
    +----------------------------++------------------+**************************************************
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |              a             ||         b        |*                        c                       *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    |                            ||                  |*                                                *
    +----------------------------++------------------+**************************************************
    ");

    let mut dome = TestDome::with_layout(SCROLLING, &layout);
    dome.open(window("c"));
    dome.open(window("a"));
    dome.open(window("b"));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 30x20
      b 30,0 20x20 highlighted spawn=right
      c 50,0 50x20
    +----------------------------+********************+------------------------------------------------+
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |              a             |*         b        *|                        c                       |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    |                            |*                  *|                                                |
    +----------------------------+********************+------------------------------------------------+
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { { title = 'a' }, { title = 'b' } }"),
    );
    let a = dome.open(window("a"));
    dome.open(window("b").label("b1"));
    dome.focus(a);
    dome.open(window("u"));
    assert_snapshot!(dome.snapshot(), @"
    focused u on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 20,0 20x20
      b1 60,0 20x20
      u 40,0 20x20 highlighted spawn=right
                        +------------------+********************+------------------+                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |         a        |*         u        *|        b1        |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        +------------------+********************+------------------+
    ");
    dome.open(window("b").label("b2"));
    assert_snapshot!(dome.snapshot(), @"
    focused b2 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 10,0 20x20
      b1 70,0 20x20
      b2 50,0 20x20 highlighted spawn=right
      u 30,0 20x20
              +------------------++------------------+********************+------------------+          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |         a        ||         u        |*        b2        *|        b1        |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              |                  ||                  |*                  *|                  |          
              +------------------++------------------+********************+------------------+
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING,
        "return { primary = { ['1'] = { layout = 'scrolling', columns = { { title = 'a' } } } } }",
    );
    dome.open(window("t"));
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      a 25,0 50x20 highlighted spawn=right
                             **************************************************                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                                                *                         
                             *                        a                       *                         
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
}

#[test]
fn apply_layout_with_changed_columns_reorders_restacks_and_resizes_open_windows() {
    let reversed = workspace_0_layout("columns = { { title = 'b' }, { title = 'a' } }");
    let mut dome = TestDome::new("return {}");
    dome.open(window("a"));
    dome.open(window("b"));
    dome.apply_layout(&reversed);
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

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { { title = 'a' }, { title = 'b' } }"),
    );
    dome.open(window("a"));
    dome.open(window("b"));
    dome.apply_layout(&reversed);
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 50,0 20x20 highlighted spawn=right
      b 30,0 20x20
                                  +------------------+********************                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |         b        |*         a        *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  |                  |*                  *                              
                                  +------------------+********************
    ");
    dome.apply_layout(&workspace_0_layout(
        "columns = { { children = { { title = 'a' }, { title = 'b' } } } }",
    ));
    assert_snapshot!(dome.snapshot(), @"
    focused b on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x10
      b 40,10 20x10 highlighted spawn=right
                                            +------------------+                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |         a        |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            |                  |                                        
                                            +------------------+                                        
                                            ********************                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *         b        *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            *                  *                                        
                                            ********************
    ");
    dome.apply_layout(&workspace_0_layout("column_width = '30%', columns = { { title = 'a' }, { width = '10%', children = { { title = 'b' } } } }"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 30,0 30x20 highlighted spawn=right
      b 60,0 10x20
                                  ******************************+--------+                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *              a             *|    b   |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  *                            *|        |                              
                                  ******************************+--------+
    ");
    dome.open(window("u"));
    assert_snapshot!(dome.snapshot(), @"
    focused u on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 15,0 30x20
      b 75,0 10x20
      u 45,0 30x20 highlighted spawn=right
                   +----------------------------+******************************+--------+               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |              a             |*              u             *|    b   |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   |                            |*                            *|        |               
                   +----------------------------+******************************+--------+
    ");

    let mut dome = TestDome::new(SCROLLING_40_PERCENT);
    dome.open(window("u0"));
    dome.apply_layout(&workspace_0_layout("column_width = '10%'"));
    dome.open(window("u1"));
    assert_snapshot!(dome.snapshot(), @"
    focused u1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      u0 40,0 10x20
      u1 50,0 10x20 highlighted spawn=right
                                            +--------+**********                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |   u0   |*   u1   *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            |        |*        *                                        
                                            +--------+**********
    ");
}

#[test]
fn save_layout_writes_live_columns_and_reapplies_unchanged() {
    let mut dome = TestDome::with_layout(
        SCROLLING,
        &workspace_0_layout(
            "column_width = '30%', columns = { { title = 'a' }, { width = '20%', children = { { title = 'b' }, { title = 'c' } } } }, float = { { title = 'f' } }, fullscreen = { { title = 'fs' } }",
        ),
    );
    dome.open(window("a"));
    dome.open(window("b"));
    dome.open(window("c"));
    dome.open(window("t").label("t1"));
    dome.open(window("t").label("t2"));
    dome.open(window("f").frame(FLOAT_FRAME));
    dome.open(window("fs"));
    let before = dome.snapshot();
    assert_snapshot!(before, @"
    focused fs on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1 fullscreen fs
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                fs                                                |
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
    let saved = dome.save_layout();
    assert_snapshot!(saved, @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "scrolling",
          column_width = "30%",
          columns = {
            { width = "30%", children = {
              { title = "a" },
            } },
            { width = "20%", children = {
              { title = "b" },
              { title = "c" },
            } },
            { width = "30%", children = {
              { title = "t" },
            } },
            { width = "30%", children = {
              { title = "t" },
            } },
          },
          float = {
            { title = "f" },
          },
          fullscreen = {
            { title = "fs" },
          },
        },
      },
    }
    "#);
    dome.apply_layout(&saved);
    assert_eq!(dome.save_layout(), saved);
    assert_eq!(dome.snapshot(), before);
}

#[test]
fn open_with_columns_layout_skips_unreadable_entries_and_keeps_rest() {
    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout(
            "columns = { { title = 'a' }, { width = '30%', children = { { title = 'b' }, { title = 'c' } } }, { width = 10, children = { { title = 'd' } } }, { width = '40%', title = 'e' }, { width = 'wide', children = { { title = 'g' } } } }",
        ),
    );
    let a = dome.open(window("a"));
    dome.open(window("b"));
    dome.open(window("c"));
    dome.open(window("d"));
    dome.focus(a);
    dome.open(window("e"));
    dome.open(window("g"));
    assert_snapshot!(dome.snapshot(), @"
    focused g on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 0,0 20x20
      b 40,0 30x10
      c 40,10 30x10
      d 70,0 10x20
      e 20,0 20x20
      g 80,0 20x20 highlighted spawn=right
    +------------------++------------------++----------------------------++--------+********************
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||              b             ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |         a        ||         e        |+----------------------------+|    d   |*         g        *
    |                  ||                  |+----------------------------+|        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||              c             ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    |                  ||                  ||                            ||        |*                  *
    +------------------++------------------++----------------------------++--------+********************
    ");

    let mut dome = TestDome::with_layout(
        SCROLLING_20_PERCENT,
        &workspace_0_layout("columns = { title = 'a' }"),
    );
    let u = dome.open(window("u"));
    dome.open(window("v"));
    dome.focus(u);
    dome.open(window("a"));
    assert_snapshot!(dome.snapshot(), @"
    focused a on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      a 40,0 20x20 highlighted spawn=right
      u 20,0 20x20
      v 60,0 20x20
                        +------------------+********************+------------------+                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |         u        |*         a        *|         v        |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        |                  |*                  *|                  |                    
                        +------------------+********************+------------------+
    ");
}
