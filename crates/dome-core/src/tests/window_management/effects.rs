use insta::assert_snapshot;

use crate::tests::fixtures::{TestDome, window};

#[test]
fn run_close_asks_platform_to_close_focused_window_only() {
    let mut dome = TestDome::new("return {}");
    dome.run("close");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
    ");

    let w0 = dome.open(window("w0"));
    dome.open(window("w1"));
    dome.focus(w0);
    dome.run("close");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: close w0
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
