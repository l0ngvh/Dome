use std::collections::BTreeSet;

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
fn open_with_overlapping_matchers_fills_eligible_workspaces_and_reuses_closed_slots() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"local workspaces = {}
        for index = 1, 8 do
            workspaces["slot" .. index] = {
                layout = "partition_tree", tree = { { title = "/^term/" } },
            }
        end
        return { primary = workspaces }"#,
    );
    let mut available: BTreeSet<_> = (1..=8).map(|index| format!("slot{index}")).collect();
    let mut windows = Vec::new();
    for index in 1..=8 {
        let label = format!("original-{index}");
        let window = dome.open(window("term").label(&label));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert!(available.remove(&focused.name), "{focused:?}");
        assert!(dome.snapshot().contains(&format!("{label} 0,0 100x20")));
        windows.push((window, focused.name));
    }
    for index in [5, 1] {
        dome.close(windows[index].0);
        available.insert(windows[index].1.clone());
    }
    for index in 1..=2 {
        let label = format!("replacement-{index}");
        dome.open(window("term-next").label(&label));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert!(available.remove(&focused.name), "{focused:?}");
        assert!(dome.snapshot().contains(&format!("{label} 0,0 100x20")));
    }
    assert!(available.is_empty());
}

#[test]
fn apply_layout_with_remote_holds_consumes_local_overlapping_slots() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { source = { layout = "partition_tree", tree = {
            { title = "term" }, { title = "term" }, { title = "term" }, { title = "term" },
        } } } }"#,
    );
    for label in ["first", "second", "third"] {
        dome.open(window("term").label(label));
        dome.run("move workspace 0");
        dome.run("focus workspace 0");
    }
    dome.open(window("float-only"));
    dome.open(window("fullscreen-only"));
    let replacement = r#"return { primary = {
        ["0"] = { layout = "partition_tree", tree = {
            { title = "/^term$/" }, { title = "term" }, { title = "term" },
            { title = "float-only" }, { title = "fullscreen-only" },
        }, float = { { title = "unused-float" } }, fullscreen = { { title = "unused-fullscreen" } } },
        source = { layout = "partition_tree", tree = { { title = "/.*/" } } },
    } }"#;
    for _ in 0..2 {
        dome.apply_layout(replacement);
        let snapshot = dome.snapshot();
        let mut available = BTreeSet::from(["0,0", "20,0", "40,0"]);
        for label in ["first", "second", "third"] {
            let placement = snapshot
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{label} ")))
                .unwrap_or_else(|| panic!("missing {label}: {snapshot}"));
            let mut fields = placement.split_whitespace();
            assert_eq!(fields.next(), Some(label));
            assert!(available.remove(fields.next().unwrap()), "{snapshot}");
            assert_eq!(fields.next(), Some("20x20"), "{snapshot}");
            assert!(!fields.any(|field| field == "float"), "{snapshot}");
        }
        for placement in ["float-only 60,0 20x20", "fullscreen-only 80,0 20x20"] {
            assert!(snapshot.contains(placement), "{snapshot}");
        }
    }
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
fn apply_layout_changing_float_matchers_tiles_old_matches_and_floats_new_matches() {
    let mut dome = TestDome::with_layout(
        "return {}",
        r#"return { primary = { ["0"] = { layout = "partition_tree", float = { { title = "chat" } } } } }"#,
    );
    let chat = dome.open(window("chat").frame([10, 2, 30, 8]));
    dome.open(window("w1"));
    dome.apply_layout(
        r#"return { primary = { ["0"] = { layout = "partition_tree", float = { { title = "music" } } } } }"#,
    );
    let snapshot = dome.snapshot();
    assert!(snapshot.contains("chat 50,0 50x20"), "{snapshot}");
    assert!(snapshot.contains("w1 0,0 50x20"), "{snapshot}");
    let chat_line = snapshot
        .lines()
        .find(|line| line.trim_start().starts_with("chat "))
        .unwrap();
    assert!(
        !chat_line.split_whitespace().any(|field| field == "float"),
        "{snapshot}"
    );
    dome.open(window("music").frame([50, 4, 30, 8]));
    let snapshot = dome.snapshot();
    assert!(
        snapshot.contains("music 50,4 30x8 float highlighted"),
        "{snapshot}"
    );
    assert!(snapshot.contains("chat 50,0 50x20"), "{snapshot}");
    let chat_line = snapshot
        .lines()
        .find(|line| line.trim_start().starts_with("chat "))
        .unwrap();
    assert!(
        !chat_line.split_whitespace().any(|field| field == "float"),
        "{snapshot}"
    );
    dome.close(chat);
    dome.open(window("chat").label("chat-reopened").frame([10, 2, 30, 8]));
    let snapshot = dome.snapshot();
    assert!(
        snapshot.contains("chat-reopened 50,0 50x20 highlighted"),
        "{snapshot}"
    );
    assert!(snapshot.contains("w1 0,0 50x20"), "{snapshot}");
    assert!(snapshot.contains("music 50,4 30x8 float"), "{snapshot}");
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
fn apply_layout_replaces_slots_on_every_workspace() {
    let initial = r#"return { primary = {
        a = { layout = "partition_tree", tree = { { title = "old-tiled-a" } },
              float = { { title = "old-float-a" } }, fullscreen = { { title = "old-fullscreen-a" } } },
        b = { layout = "partition_tree", tree = { { title = "old-tiled-b" } },
              float = { { title = "old-float-b" } }, fullscreen = { { title = "old-fullscreen-b" } } },
    } }"#;
    let mut dome = TestDome::with_layout("return {}", initial);
    dome.apply_layout(&initial.replace("old-", "new-"));
    for workspace in ["a", "b"] {
        for mode in ["tiled", "float", "fullscreen"] {
            dome.run("focus workspace parking");
            dome.open(window(&format!("old-{mode}-{workspace}")));
            let focused = dome
                .query_workspaces()
                .into_iter()
                .find(|workspace| workspace.is_focused)
                .unwrap();
            assert_eq!(focused.name, "parking");

            dome.open(window(&format!("new-{mode}-{workspace}")));
            let focused = dome
                .query_workspaces()
                .into_iter()
                .find(|workspace| workspace.is_focused)
                .unwrap();
            assert_eq!(focused.name, workspace);
        }
    }
}

#[test]
fn apply_layout_after_cross_monitor_move_rematches_holds() {
    for (source, target) in [("primary", "second"), ("second", "primary")] {
        let initial = r#"return { SOURCE = { ["0"] = {
            layout = "partition_tree", tree = { { title = "moving" } },
        } } }"#
            .replace("SOURCE", source);
        let mut dome = TestDome::with_layout("return {}", &initial);
        dome.add_monitor(monitor("second", [100, 0, 100, 20]));
        let moving = dome.open(window("moving").label("original"));
        dome.run(&format!("move workspace 0 --monitor {target}"));
        let replacement = r#"return {
            primary = { ["0"] = {
                layout = "partition_tree", tree = { { title = "moving" } },
            } },
            second = { ["0"] = {
                layout = "partition_tree", tree = { { title = "moving" } },
            } },
        }"#;
        dome.apply_layout(replacement);
        dome.run("focus workspace parking --monitor primary");
        dome.open(window("moving").label("source-next"));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert_eq!(
            (focused.name.as_str(), focused.monitor.as_str()),
            ("0", source)
        );

        dome.close(moving);
        let target_next = dome.open(window("moving").label("target-next"));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert_eq!(
            (focused.name.as_str(), focused.monitor.as_str()),
            ("0", target)
        );

        dome.minimize(target_next);
        dome.reload("return {}");
        dome.apply_layout(replacement);
        dome.open(window("moving").label("target-after-reset"));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert_eq!(
            (focused.name.as_str(), focused.monitor.as_str()),
            ("0", target)
        );
        assert!(dome.snapshot().contains("minimized: target-next"));

        dome.unminimize(target_next);
        dome.close(target_next);
        dome.run("focus workspace parking --monitor primary");
        dome.open(window("moving").label("unmatched"));
        let focused = dome
            .query_workspaces()
            .into_iter()
            .find(|workspace| workspace.is_focused)
            .unwrap();
        assert_eq!(focused.name, "parking");
    }
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
fn apply_layout_rematches_all_modes_and_holds_slots_in_current_workspace() {
    for source in ["partition_tree", "master", "scrolling"] {
        for target in ["partition_tree", "master", "scrolling"] {
            let initial = format!(
                r#"return {{ primary = {{ ["0"] = {{ layout = "{source}",
                    float = {{ {{ title = "to-tiling" }}, {{ title = "to-fullscreen" }},
                              {{ title = "fallback-float" }}, {{ title = "stay-float" }} }},
                    fullscreen = {{ {{ title = "fullscreen-to-float" }},
                                   {{ title = "fallback-fullscreen" }} }},
                }} }} }}"#
            );
            let mut dome = TestDome::with_layout("return {}", &initial);
            let titles = [
                "to-float",
                "to-tiling",
                "to-fullscreen",
                "fullscreen-to-float",
                "fallback-float",
                "fallback-fullscreen",
                "stay-float",
            ];
            let windows: Vec<_> = titles
                .iter()
                .map(|title| dome.open(window(title).frame([10, 2, 30, 8])))
                .collect();
            let tiling = match target {
                "partition_tree" => r#"tree = { { title = "to-tiling" } },"#,
                "master" => r#"master = { { title = "to-tiling" } },"#,
                "scrolling" => r#"columns = { { children = { { title = "to-tiling" } } } },"#,
                _ => unreachable!(),
            };
            let replacement = format!(
                r#"return {{ primary = {{ ["0"] = {{ layout = "{target}", {tiling}
                    float = {{ {{ title = "to-float" }}, {{ title = "fullscreen-to-float" }},
                              {{ title = "stay-float" }} }},
                    fullscreen = {{ {{ title = "to-fullscreen" }} }},
                }}, remote = {{ layout = "{target}",
                    fullscreen = {{ {{ title = "fallback-float" }}, {{ title = "fallback-fullscreen" }} }},
                }} }} }}"#
            );
            dome.apply_layout(&replacement);
            let live = dome.capture_live_layouts();
            let workspace = live.workspace("primary", "0").unwrap();
            let floats: BTreeSet<_> = workspace
                .float
                .iter()
                .map(|matcher| matcher.title.as_deref().unwrap())
                .collect();
            assert_eq!(
                floats,
                BTreeSet::from(["to-float", "fullscreen-to-float", "stay-float"]),
                "{source} -> {target}"
            );
            assert_eq!(workspace.fullscreen.len(), 1);
            assert_eq!(
                workspace.fullscreen[0].title.as_deref(),
                Some("to-fullscreen")
            );
            assert!(dome.snapshot().contains("fullscreen to-fullscreen"));
            assert!(
                live.workspace("primary", "remote")
                    .unwrap()
                    .fullscreen
                    .is_empty()
            );

            dome.run("focus workspace parking");
            dome.open(window("to-fullscreen").label("fullscreen-duplicate"));
            assert_eq!(
                dome.query_workspaces()
                    .into_iter()
                    .find(|workspace| workspace.is_focused)
                    .unwrap()
                    .name,
                "parking"
            );
            dome.close(windows[2]);
            dome.run("focus workspace 0");
            for index in [1, 4, 5] {
                dome.focus(windows[index]);
                let snapshot = dome.snapshot();
                let title = titles[index];
                let line = snapshot
                    .lines()
                    .find(|line| line.trim_start().starts_with(&format!("{title} ")))
                    .unwrap();
                assert!(
                    !line.split_whitespace().any(|field| field == "float"),
                    "{snapshot}"
                );
            }
            let snapshot = dome.snapshot();
            assert!(
                snapshot.contains("stay-float 10,2 30x8 float"),
                "{snapshot}"
            );
            for index in [0, 1, 3, 6] {
                dome.run("focus workspace parking");
                dome.open(window(titles[index]).label(&format!("duplicate-{index}")));
                assert_eq!(
                    dome.query_workspaces()
                        .into_iter()
                        .find(|workspace| workspace.is_focused)
                        .unwrap()
                        .name,
                    "parking"
                );
                dome.set_title(windows[index], &format!("retained-{index}"));
                dome.close(windows[index]);
                dome.open(window(titles[index]).label(&format!("replacement-{index}")));
                assert_eq!(
                    dome.query_workspaces()
                        .into_iter()
                        .find(|workspace| workspace.is_focused)
                        .unwrap()
                        .name,
                    "0"
                );
            }
        }
    }
}

#[test]
fn apply_layout_overlapping_modes_fill_each_matching_slot() {
    for (strategy, tiling) in [
        ("partition_tree", r#"tree = { { title = "term" } },"#),
        ("master", r#"master = { { title = "term" } },"#),
        (
            "scrolling",
            r#"columns = { { children = { { title = "term" } } } },"#,
        ),
    ] {
        let mut dome = TestDome::new("return {}");
        for label in ["first", "second", "third"] {
            dome.open(window("term").label(label));
        }
        let replacement = format!(
            r#"return {{ primary = {{ ["0"] = {{
            layout = "{strategy}", {tiling}
            float = {{ {{ title = "term" }} }},
            fullscreen = {{ {{ title = "term" }} }},
        }} }} }}"#
        );
        for _ in 0..2 {
            dome.apply_layout(&replacement);
            let live = dome.capture_live_layouts();
            let workspace = live.workspace("primary", "0").unwrap();
            assert_eq!(workspace.float.len(), 1);
            assert_eq!(workspace.fullscreen.len(), 1);
        }
        dome.run("focus workspace parking");
        dome.open(window("term").label("next"));
        assert_eq!(
            dome.query_workspaces()
                .into_iter()
                .find(|workspace| workspace.is_focused)
                .unwrap()
                .name,
            "parking"
        );
    }
}

#[test]
fn apply_layout_new_floats_use_previous_workspace_geometry() {
    for strategy in ["partition_tree", "master", "scrolling"] {
        let mut dome = TestDome::new("return {}");
        dome.add_monitor(monitor("second", [100, 30, 100, 20]));
        dome.run("focus workspace 0 --monitor second");
        dome.apply_layout(&format!(
            r#"return {{ second = {{ ["0"] = {{ layout = "{strategy}",
            fullscreen = {{ {{ title = "fullscreen" }} }},
        }} }} }}"#
        ));
        let tiled = dome.open(window("tiled"));
        dome.run("toggle float");
        let before = dome.snapshot();
        let expected = before
            .lines()
            .find(|line| line.trim_start().starts_with("tiled "))
            .unwrap()
            .split(" float")
            .next()
            .unwrap()
            .to_owned();
        dome.run("toggle float");
        dome.open(window("fullscreen"));
        dome.run("focus workspace elsewhere --monitor primary");
        dome.apply_layout(&format!(
            r#"return {{ second = {{ ["0"] = {{ layout = "{strategy}",
            float = {{ {{ title = "tiled" }}, {{ title = "fullscreen" }} }},
        }} }} }}"#
        ));
        dome.focus(tiled);
        let snapshot = dome.snapshot();
        assert!(
            snapshot.contains(&format!("{expected} float")),
            "{snapshot}"
        );
        assert!(
            snapshot.contains("fullscreen 100,30 100x20 float"),
            "{snapshot}"
        );
    }
}

#[test]
fn apply_layout_restricted_windows_remain_fullscreen_and_hold_matched_slots() {
    for strategy in ["partition_tree", "master", "scrolling"] {
        for restrictions in [
            WindowRestrictions::ProtectFullscreen,
            WindowRestrictions::BlockAll,
        ] {
            let mut dome = TestDome::new("return {}");
            let restricted = dome.open(window("restricted").restricted(restrictions));
            let fallback = dome.open(window("fallback").restricted(restrictions));
            dome.apply_layout(&format!(
                r#"return {{ primary = {{ ["0"] = {{ layout = "{strategy}",
                float = {{ {{ title = "restricted" }} }},
            }} }} }}"#
            ));
            dome.open(window("restricted").label("duplicate"));
            let live = dome.capture_live_layouts();
            let workspace = live.workspace("primary", "0").unwrap();
            assert!(workspace.float.is_empty());
            let fullscreen: BTreeSet<_> = workspace
                .fullscreen
                .iter()
                .map(|matcher| matcher.title.as_deref().unwrap())
                .collect();
            assert_eq!(fullscreen, BTreeSet::from(["restricted", "fallback"]));
            dome.close(fallback);
            dome.close(restricted);
            let snapshot = dome.snapshot();
            let duplicate = snapshot
                .lines()
                .find(|line| line.trim_start().starts_with("duplicate "))
                .unwrap();
            assert!(
                !duplicate.split_whitespace().any(|field| field == "float"),
                "{snapshot}"
            );
            dome.open(
                window("restricted")
                    .label("replacement")
                    .frame([10, 2, 30, 8]),
            );
            assert!(dome.snapshot().contains("replacement 10,2 30x8 float"));
        }
    }
}

#[test]
fn apply_layout_switching_strategy_preserves_window_constraints() {
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
    dome.apply_layout(
        r#"return { primary = { ["0"] = { layout = "master",
        float = { { title = "f" } }, fullscreen = { { title = "fs" } },
    } } }"#,
    );
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
    dome.apply_layout(
        r#"return { primary = { ["0"] = { layout = "partition_tree",
        float = { { title = "f" } },
    } } }"#,
    );
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
