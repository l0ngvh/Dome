use insta::assert_snapshot;

use crate::core::{Length, LimitObservation, LimitUpdate, WindowRestrictions};
use crate::tests::test_dome::{TestDome, monitor, window};

const HAND_WRITTEN: &str = "-- hand-written\nreturn {}\n";

const SAVED: &str = r#"---@type dome.Layout
return {
  ["desk"] = {
    ["count"] = {
      layout = "master",
      master_count = 3,
    },
    ["dev"] = {
      layout = "partition_tree",
      tree = { split = "horizontal", children = {
        { app = "Ghostty" },
        {
          { app = "Firefox", title = "docs" },
          { process = "slack.exe" },
        },
      } },
      float = {
        { title = "a\"b\\c" },
      },
      fullscreen = {
        { class = "Steam" },
      },
    },
    ["work"] = {
      layout = "master",
      master_ratio = 0.6,
      master_count = 2,
      master = { display = "tabbed", children = {
        { bundle_id = "com.editor" },
        { aumid = "Mail" },
      } },
      secondary = {
        { class = "Notes" },
      },
    },
  },
  ["primary"] = {
    ["0"] = {
      layout = "partition_tree",
    },
  },
}
"#;

#[test]
fn open_matching_free_slot_lands_on_slot_workspace_and_focuses_it() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { dev = { layout = "partition_tree", tree = { { title = "term" } } } } }"#,
    );
    dome.open(window("w0"));
    dome.open(window("term"));
    assert_snapshot!(dome.snapshot(), @"
    focused term on primary
    workspaces: 0 on primary, dev on primary (focused)
    primary 0,0 100x20 border 1
      term 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                               term                                               *
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

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = {
            a = { layout = "partition_tree", tree = { { title = "term" } }, float = { { title = "chat" } } },
            b = { layout = "partition_tree", tree = { { title = "term" } }, fullscreen = { { title = "chat" } } },
        } }"#,
    );
    dome.open(window("term").label("term-first"));
    dome.open(window("term").label("term-second"));
    dome.open(window("chat").label("chat-first").frame([10, 2, 30, 8]));
    dome.open(window("chat").label("chat-second").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["a"] = {
          layout = "partition_tree",
          tree = { title = "term" },
          float = {
            { title = "chat" },
          },
        },
        ["b"] = {
          layout = "partition_tree",
          tree = { title = "term" },
          fullscreen = {
            { title = "chat" },
          },
        },
      },
    }
    "#);
}

#[test]
fn open_matching_no_slot_tiles_on_current_workspace() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return {
            primary = { dev = {
                layout = "partition_tree",
                tree = { { title = "term" } },
                float = { { title = "chat" } },
                fullscreen = { { title = "video" } },
            } },
            desktop = { desk = { layout = "partition_tree", float = { { title = "w0" } } } },
        }"#,
    );
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
fn open_restricted_matching_float_slot_goes_fullscreen_and_holds_slot() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { dev = { layout = "partition_tree", float = { { title = "chat" } } } } }"#,
    );
    dome.open(
        window("chat")
            .label("restricted-chat")
            .frame([10, 2, 30, 8])
            .restricted(WindowRestrictions::ProtectFullscreen),
    );
    assert_snapshot!(dome.snapshot(), @"
    focused restricted-chat on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1 fullscreen restricted-chat
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                          restricted-chat                                         |
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
    dome.open(
        window("chat")
            .label("unrestricted-chat")
            .frame([10, 2, 30, 8]),
    );
    assert_snapshot!(dome.snapshot(), @"
    focused unrestricted-chat on primary
    workspaces: 0 on primary (focused), dev on primary
    primary 0,0 100x20 border 1
      unrestricted-chat 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                         unrestricted-chat                                        *
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
fn open_matching_regex_matcher_routes_like_exact_match() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { dev = { layout = "partition_tree", float = { { title = "/^chat/" } } } } }"#,
    );
    dome.open(window("chat-1").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused chat-1 on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      chat-1 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *           chat-1           *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
    dome.open(window("my-chat").frame([50, 4, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused my-chat on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      chat-1 10,2 30x8 float
      my-chat 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *         +----------------------------+                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |           chat-1           |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         |                            |                                                           *
    *         +----------------------------+       my-chat                                             *
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
fn launch_with_layout_creates_named_workspaces_from_each_monitor_entry() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = {
            c = { layout = "partition_tree", tree = { { title = "wc" } } },
            b = { layout = "partition_tree", tree = { { title = "wb" } } },
            unknown = { layout = "unknown_strategy" },
            a = { layout = "partition_tree", tree = { { title = "wa" } } },
        } }"#,
    );
    dome.open(window("wc"));
    assert_snapshot!(dome.snapshot(), @"
    focused wc on primary
    workspaces: c on primary (focused)
    primary 0,0 100x20 border 1
      wc 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                wc                                                *
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
    dome.open(window("wb"));
    assert_snapshot!(dome.snapshot(), @"
    focused wb on primary
    workspaces: b on primary (focused), c on primary
    primary 0,0 100x20 border 1
      wb 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                wb                                                *
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
    dome.open(window("wa"));
    assert_snapshot!(dome.snapshot(), @"
    focused wa on primary
    workspaces: a on primary (focused), b on primary, c on primary
    primary 0,0 100x20 border 1
      wa 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                wa                                                *
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

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = {
            m = { layout = "master" },
            p = { layout = "partition_tree" },
            s = { layout = "scrolling" },
            x = { layout = "grid" },
            missing = {},
            empty = { layout = "" },
        } }"#,
    );
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["m"] = {
          layout = "master",
        },
        ["p"] = {
          layout = "partition_tree",
        },
        ["s"] = {
          layout = "scrolling",
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return {
            primary = { ["1"] = { layout = "master" } },
            ["monitor-1"] = { ["1"] = { layout = "scrolling" } },
        }"#,
    );
    dome.add_monitor(monitor("monitor-1", [100, 0, 100, 20]));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["monitor-1"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["1"] = {
          layout = "scrolling",
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["1"] = {
          layout = "master",
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"local shared = { ["1"] = { layout = "master" } }
        return { primary = shared, ["monitor-1"] = shared }"#,
    );
    dome.add_monitor(monitor("monitor-1", [100, 0, 100, 20]));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["monitor-1"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["1"] = {
          layout = "master",
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["1"] = {
          layout = "master",
        },
      },
    }
    "#);
}

#[test]
fn apply_layout_resets_parked_workspace_from_origin_monitor_entry() {
    let mut dome = TestDome::new("return {}");
    let secondary = dome.add_monitor(monitor("monitor-1", [100, 0, 100, 20]));
    dome.run("focus monitor monitor-1");
    dome.run("focus workspace work");
    dome.open(window("w0"));
    dome.remove_monitor(secondary);
    dome.apply_layout(r#"return { ["monitor-1"] = { work = { layout = "master" } } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), work parked from monitor-1
    primary 0,0 100x20 border 1
    ");
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["monitor-1"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["work"] = {
          layout = "master",
          master = {
            { title = "w0" },
          },
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
      },
    }
    "#);
}

#[test]
fn apply_layout_naming_new_workspace_creates_it_with_working_matchers() {
    let mut dome = TestDome::new("return {}");
    dome.apply_layout(
        r#"return { primary = { dev = { layout = "partition_tree", float = { { title = "chat" } } } } }"#,
    );
    dome.open(window("chat").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused chat on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      chat 10,2 30x8 float highlighted
                                                                                                        
                                                                                                        
              ******************************                                                            
              *                            *                                                            
              *                            *                                                            
              *            chat            *                                                            
              *                            *                                                            
              *                            *                                                            
              *                            *                                                            
              ******************************
    ");
}

#[test]
fn apply_layout_changing_float_matchers_keeps_open_floats_and_floats_new_matches() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = { layout = "partition_tree", float = { { title = "chat" } } } } }"#,
    );
    let chat = dome.open(window("chat").frame([10, 2, 30, 8]));
    dome.open(window("w1"));
    dome.apply_layout(
        r#"return { primary = { ["0"] = { layout = "partition_tree", float = { { title = "music" } } } } }"#,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused chat on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      chat 10,2 30x8 float highlighted
      w1 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *            chat            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         ******************************         w1                                                |
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
    dome.open(window("music").frame([50, 4, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused music on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      chat 10,2 30x8 float
      music 50,4 30x8 float highlighted
      w1 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         +----------------------------+                                                           |
    |         |                            |                                                           |
    |         |                            |          ******************************                   |
    |         |            chat            |          *                            *                   |
    |         |                            |          *                            *                   |
    |         |                            |          *            music           *                   |
    |         |                            |          *                            *                   |
    |         +----------------------------+         w*                            *                   |
    |                                                 *                            *                   |
    |                                                 ******************************                   |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    +--------------------------------------------------------------------------------------------------+
    ");
    dome.close(chat);
    dome.open(window("chat").label("chat-reopened").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused chat-reopened on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      chat-reopened 50,0 50x20 highlighted spawn=right
      music 50,4 30x8 float
      w1 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |+----------------------------+                   *
    |                                                ||                            |                   *
    |                                                ||                            |                   *
    |                                                ||            music           |                   *
    |                                                ||                            |                   *
    |                       w1                       ||                            |ed                 *
    |                                                ||                            |                   *
    |                                                |+----------------------------+                   *
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
fn close_slot_holder_frees_slot_for_next_matching_window() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { dev = { layout = "partition_tree", tree = { { title = "term" } }, float = { { title = "chat" } } } } }"#,
    );
    let term = dome.open(window("term").label("term-first"));
    let chat = dome.open(window("chat").label("chat-first").frame([10, 2, 30, 8]));
    dome.run("focus workspace 0");
    dome.close(term);
    dome.close(chat);
    dome.open(window("term").label("term-next"));
    dome.open(window("chat").label("chat-next").frame([10, 2, 30, 8]));
    assert_snapshot!(dome.snapshot(), @"
    focused chat-next on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      chat-next 10,2 30x8 float highlighted
      term-next 0,0 100x20
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |         ******************************                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *          chat-next         *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         *                            *                                                           |
    |         ******************************      term-next                                            |
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
fn open_matching_slot_held_by_live_window_tiles_on_current_workspace() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = {
            ["0"] = { layout = "partition_tree", float = { { title = "minimized" } }, fullscreen = { { title = "tiled" } } },
            dev = { layout = "partition_tree", tree = { { title = "moved" } }, float = { { title = "fullscreened" } } },
        } }"#,
    );
    let moved = dome.open(window("moved").label("moved-first"));
    let fullscreened = dome.open(
        window("fullscreened")
            .label("fullscreened-first")
            .frame([10, 2, 30, 8]),
    );
    let minimized = dome.open(
        window("minimized")
            .label("minimized-first")
            .frame([10, 2, 30, 8]),
    );
    let tiled = dome.open(window("tiled").label("tiled-first"));
    dome.focus(moved);
    dome.run("move workspace 2");
    dome.focus(fullscreened);
    dome.run("toggle fullscreen");
    dome.minimize(minimized);
    dome.focus(tiled);
    dome.run("toggle fullscreen");
    dome.reload("return {}");
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.remove_monitor(dell);
    dome.run("focus workspace 0");
    dome.open(window("moved").label("moved-next"));
    dome.open(window("fullscreened").label("fullscreened-next"));
    dome.open(window("minimized").label("minimized-next"));
    dome.open(window("tiled").label("tiled-next"));
    assert_snapshot!(dome.snapshot(), @r#"
    focused tiled-next on primary
    workspaces: 0 on primary (focused), dev on primary, 2 on primary
    minimized: minimized-first "minimized"
    primary 0,0 100x20 border 1
      fullscreened-next 40,0 20x20
      minimized-next 60,0 20x20
      moved-next 20,0 20x20
      tiled-first 0,0 20x20
      tiled-next 80,0 20x20 highlighted spawn=right
    +------------------++------------------++------------------++------------------+********************
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |    tiled-first   ||    moved-next    || fullscreened-next||  minimized-next  |*    tiled-next    *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    |                  ||                  ||                  ||                  |*                  *
    +------------------++------------------++------------------++------------------+********************
    "#);

    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { dev = { layout = "partition_tree", tree = {
            split = "horizontal", children = {
                { { title = "fullscreen" } },
                { { title = "minimized" } },
            },
        } } } }"#,
    );
    dome.open(window("anchor"));
    let fullscreen = dome.open(window("fullscreen").label("fullscreen-holder"));
    let minimized = dome.open(window("minimized").label("minimized-holder"));
    dome.focus(fullscreen);
    dome.run("toggle fullscreen");
    dome.minimize(minimized);
    dome.run("focus workspace 0");
    dome.open(window("fullscreen").label("fullscreen-next"));
    dome.open(window("minimized").label("minimized-next"));
    assert_snapshot!(dome.snapshot(), @r#"
    focused minimized-next on primary
    workspaces: 0 on primary (focused), dev on primary
    minimized: minimized-holder "minimized"
    primary 0,0 100x20 border 1
      anchor 0,0 33x20
      fullscreen-next 33,0 34x20
      minimized-next 67,0 33x20 highlighted spawn=right
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |             anchor            ||         fullscreen-next        |*         minimized-next        *
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
    "#);
}

#[test]
fn move_to_workspace_with_free_matching_slot_takes_no_slot() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("term").label("term-moved"));
    dome.apply_layout(
        r#"return { primary = { dev = { layout = "partition_tree", tree = { { title = "term" } } } } }"#,
    );
    dome.run("move workspace dev");
    dome.open(window("term").label("term-next"));
    assert_snapshot!(dome.snapshot(), @"
    focused term-next on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      term-moved 0,0 50x20
      term-next 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                   term-moved                   |*                    term-next                   *
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
fn apply_layout_rematches_moved_window_holding_slot_elsewhere() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { a = { layout = "partition_tree", tree = { { title = "w" } } } } }"#,
    );
    dome.open(window("w"));
    dome.run("move workspace 0");
    dome.run("focus workspace 0");
    dome.open(window("v"));
    dome.open(window("u"));
    dome.apply_layout(r#"return { primary = {
        ["0"] = { layout = "partition_tree", tree = { { title = "u" }, { title = "w" }, { title = "v" } } },
        a = { layout = "partition_tree", tree = { { title = "w" } } },
    } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused w on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      u 0,0 33x20
      v 67,0 33x20
      w 33,0 34x20 highlighted spawn=right
    +-------------------------------+**********************************+-------------------------------+
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                               |*                                *|                               |
    |                u              |*                w               *|                v              |
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
}

#[test]
fn save_layout_writes_every_workspace_in_name_order_and_backs_up_previous_file() {
    let mut dome = TestDome::with_layout("return {}", HAND_WRITTEN);
    dome.run("focus workspace c");
    dome.run("focus workspace b");
    dome.run("focus workspace a");
    dome.open(window("w0"));
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["a"] = {
          layout = "partition_tree",
          tree = { title = "w0" },
        },
        ["b"] = {
          layout = "partition_tree",
        },
        ["c"] = {
          layout = "partition_tree",
        },
      },
    }
    "#);
    assert_eq!(dome.layout_backup(), HAND_WRITTEN);
}

#[test]
fn save_layout_writes_disconnected_monitor_entries_under_their_monitor() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { desktop = {
            ["9"] = { layout = "master", master_count = 3, float = { {} } },
            reading = { layout = "scrolling", columns = {
                { title = "single" },
                { children = { { title = "top" }, { title = "bottom" } } },
            } },
        } }"#,
    );
    let other = dome.add_monitor(monitor("monitor-1", [100, 0, 100, 20]));
    dome.run("focus monitor monitor-1");
    dome.run("focus workspace work");
    dome.open(window("w0"));
    dome.remove_monitor(other);
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["desktop"] = {
        ["9"] = {
          layout = "master",
          master_count = 3,
          float = {
            {},
          },
        },
        ["reading"] = {
          layout = "scrolling",
          columns = {
            { title = "single" },
            { children = {
              { title = "top" },
              { title = "bottom" },
            } },
          },
        },
      },
      ["monitor-1"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["work"] = {
          layout = "partition_tree",
          tree = { title = "w0" },
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
      },
    }
    "#);
}

#[test]
fn add_monitor_after_repeated_remove_creates_no_extra_workspace_zero() {
    let mut dome = TestDome::new("return {}");
    let mut dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.open(window("w0"));
    for _ in 0..3 {
        dome.remove_monitor(dell);
        dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    }
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["DELL"] = {
        ["0"] = {
          layout = "partition_tree",
          tree = { title = "w0" },
        },
      },
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
      },
    }
    "#);
}

#[test]
fn focus_window_on_hidden_workspace_keeps_empty_origin_workspace() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    dome.run("move workspace 2");
    dome.focus(w0);
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["2"] = {
          layout = "partition_tree",
          tree = { title = "w0" },
        },
      },
    }
    "#);
}

#[test]
fn save_layout_writes_file_that_loads_back_unchanged() {
    let mut dome = TestDome::with_layout("return {}", SAVED);
    assert_eq!(dome.save_layout(), SAVED);

    let title = "a\u{1}2b\u{1f}5c\nline\rreturn\ttab";
    let mut dome = TestDome::new("return {}");
    dome.run("focus workspace dev");
    dome.open(window(title).label("control-title"));
    let saved = dome.save_layout();
    assert_snapshot!(saved, @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
        },
        ["dev"] = {
          layout = "partition_tree",
          tree = { title = "a\0012b\0315c\nline\rreturn\ttab" },
        },
      },
    }
    "#);

    let mut dome = TestDome::with_layout("return {}", &saved);
    dome.open(window(title).label("control-title"));
    assert_snapshot!(dome.snapshot(), @"
    focused control-title on primary
    workspaces: dev on primary (focused)
    primary 0,0 100x20 border 1
      control-title 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                           control-title                                          *
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
fn reload_with_new_default_strategy_reaches_only_new_workspaces() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["1"] = { layout = "master" } } }"#,
    );
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    dome.run("focus workspace 1");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.open(window("w4"));
    dome.reload("return { layout = 'scrolling', scrolling = { column_width = '40%' } }");
    assert_snapshot!(dome.snapshot(), @"
    focused w4 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,0 50x20
      w3 50,0 50x10
      w4 50,10 50x10 highlighted
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w2                       |+------------------------------------------------+
    |                                                |**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                       w4                       *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    +------------------------------------------------+**************************************************
    ");
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
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
    dome.run("focus workspace 2");
    dome.open(window("w5"));
    dome.open(window("w6"));
    assert_snapshot!(dome.snapshot(), @"
    focused w6 on primary
    workspaces: 0 on primary, 1 on primary, 2 on primary (focused)
    primary 0,0 100x20 border 1
      w5 10,0 40x20
      w6 50,0 40x20 highlighted spawn=right
              +--------------------------------------+****************************************          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                                      |*                                      *          
              |                  w5                  |*                  w6                  *          
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
    assert_snapshot!(dome.save_layout(), @r#"
    ---@type dome.Layout
    return {
      ["primary"] = {
        ["0"] = {
          layout = "partition_tree",
          tree = { split = "tabbed", children = {
            { title = "w0" },
            { title = "w1" },
          } },
        },
        ["1"] = {
          layout = "master",
          master = {
            { title = "w2" },
          },
          secondary = {
            { title = "w3" },
            { title = "w4" },
          },
        },
        ["2"] = {
          layout = "scrolling",
          columns = {
            { width = "40%", children = {
              { title = "w5" },
            } },
            { width = "40%", children = {
              { title = "w6" },
            } },
          },
        },
      },
    }
    "#);
}

#[test]
fn apply_layout_switching_strategy_keeps_each_window_display_mode() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = {
            layout = "partition_tree",
            float = { { title = "f" } },
            fullscreen = { { title = "fs" } },
        } } }"#,
    );
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.set_constraint(
        w1,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(40.0)),
            ..Default::default()
        },
    );
    dome.open(window("f").frame([10, 2, 30, 8]));
    let fullscreen = dome.open(window("fs"));
    dome.apply_layout(r#"return { primary = { ["0"] = { layout = "master" } } }"#);
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
    dome.close(fullscreen);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      w0 0,0 50x20
      w1 54,0 42x10
      w2 50,10 50x10
    +------------------------------------------------+    +----------------------------------------+    
    |                                                |    |                                        |    
    |         ******************************         |    |                                        |    
    |         *                            *         |    |                                        |    
    |         *                            *         |    |                   w1                   |    
    |         *              f             *         |    |                                        |    
    |         *                            *         |    |                                        |    
    |         *                            *         |    |                                        |    
    |         *                            *         |    |                                        |    
    |         ******************************         |    +----------------------------------------+    
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
    dome.apply_layout(r#"return { primary = { ["0"] = { layout = "partition_tree" } } }"#);
    assert_snapshot!(dome.snapshot(), @"
    focused f on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 1
      f 10,2 30x8 float highlighted
      w0 0,0 33x20
      w1 33,0 34x20
      w2 67,0 33x20
    +-------------------------------++--------------------------------++-------------------------------+
    |                               ||                                ||                               |
    |         ******************************                          ||                               |
    |         *                            *                          ||                               |
    |         *                            *                          ||                               |
    |         *              f             *                          ||                               |
    |         *                            *                          ||                               |
    |         *                            *                          ||                               |
    |         *                            *                          ||                               |
    |         ******************************         w1               ||               w2              |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    |                               ||                                ||                               |
    +-------------------------------++--------------------------------++-------------------------------+
    ");
}
