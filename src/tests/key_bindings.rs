use std::collections::BTreeSet;

use crate::config::bootstrap::META_LUA;
use crate::config::{Keystroke, Modifiers};
use crate::core::Pixels;
use crate::tests::configuration::load_config;
use crate::tests::test_dome::{TestDome, monitor, window};
use insta::assert_snapshot;

#[test]
fn reload_rebound_keymaps_publishes_and_runs_only_new_bindings() {
    let mut dome = TestDome::new(
        "return { keymaps = { main = {
            ['meta+c'] = function(a) a.execute('old c') end,
            ['meta+v'] = function(a) a.execute('old v') end,
        } } }",
    );
    dome.open(window("w0"));
    assert!(
        dome.reload(
            "return { keymaps = { main = {
                ['meta+c'] = function(a) a.execute('new c') end,
                ['meta+x'] = function(a) a.execute('new x') end,
            } } }",
        )
        .is_some()
    );
    assert_eq!(dome.published_keymaps(), ["main meta+c", "main meta+x"]);
    for keystroke in ["meta+c", "meta+v", "meta+x"] {
        dome.press("main", keystroke);
    }
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: execute "new c", execute "new x"
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
}

#[test]
fn press_same_key_in_main_and_resize_runs_each_modes_binding() {
    let mut dome = TestDome::new(
        "return { keymaps = {
            main = { x = function(a) a.execute('main command') end },
            resize = { x = function(a) a.execute('resize command') end },
        } }",
    );
    dome.open(window("w0"));
    dome.press("main", "x");
    dome.press("resize", "x");
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: execute "main command", execute "resize command"
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
}

#[test]
fn boot_with_edited_defaults_changes_only_edited_bindings() {
    let mut dome = TestDome::new(
        "local config = dome.defaults()
        config.keymaps.main[Meta + 'x'] = function(a) a.execute('added') end
        config.keymaps.main[Alt + 'h'] = function(a) a.execute('replaced') end
        config.keymaps.main[Alt + 'l'] = nil
        return { keymaps = config.keymaps }",
    );
    dome.open(window("w0"));
    let mut expected = load_config("return dome.defaults()").keymaps;
    assert!(expected.iter().any(|binding| binding == "main alt+l"));
    expected.retain(|binding| binding != "main alt+l");
    expected.push("main meta+x".to_owned());
    expected.sort();
    assert_eq!(dome.published_keymaps(), expected);
    dome.press("main", "alt+h");
    dome.press("main", "meta+x");
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: execute "replaced", execute "added"
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
}

#[test]
fn press_binding_reads_value_captured_from_returned_table() {
    let mut dome = TestDome::new(
        "local config = { terminal = 'wt' }
        config.keymaps = { main = { x = function(a) a.execute(config.terminal) end } }
        return config",
    );
    dome.open(window("w0"));
    dome.press("main", "x");
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: execute "wt"
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
}

#[test]
fn press_binding_execute_exit_and_mode_reach_platform() {
    let mut dome = TestDome::new(
        "return { keymaps = { main = { x = function(a)
            a.execute('wt') a.exit() a.mode('resize')
        end } } }",
    );
    dome.open(window("w0"));
    dome.press("main", "x");
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    effects: execute "wt", mode resize, exit
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
}

#[test]
fn press_close_binding_closes_focused_window() {
    let config = "return { keymaps = { main = { x = function(a) a.close() end } } }";
    let mut dome = TestDome::new(config);
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.press("main", "x");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    effects: close w1
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

    let mut empty = TestDome::new(config);
    let before = empty.snapshot();
    empty.press("main", "x");
    assert_eq!(empty.snapshot(), before);
}

#[test]
fn press_action_handle_kept_past_its_binding_does_nothing() {
    let mut dome = TestDome::new(
        "local held
        return { keymaps = { main = {
            x = function(a) held = a.focus_left end,
            y = function() held() end,
        } } }",
    );
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
    dome.press("main", "x");
    dome.press("main", "y");
    assert_eq!(dome.snapshot(), before);
}

#[derive(Clone, Copy)]
enum Fixture {
    Cross,
    Tabbed,
    Monitors,
    Master,
}

impl Fixture {
    fn boot(self, call: &str) -> TestDome {
        let settings = match self {
            Self::Master => "layout = 'master', master = { master_count = 2 },",
            _ => "",
        };
        let mut dome = TestDome::new(&format!(
            "return {{ {settings} keymaps = {{ main = {{ x = function(a) a.{call} end }} }} }}"
        ));
        match self {
            Self::Cross | Self::Tabbed => {
                dome.open(window("w0"));
                let middle = dome.open(window("w1"));
                dome.open(window("w2"));
                dome.focus(middle);
                dome.run("toggle spawn");
                let focused = dome.open(window("w3"));
                dome.open(window("w4"));
                dome.focus(focused);
                if let Self::Tabbed = self {
                    dome.run("toggle layout");
                }
            }
            Self::Monitors => {
                dome.open(window("w0"));
                for (name, frame) in [
                    ("west", [-100, 0, 100, 20]),
                    ("east", [100, 0, 100, 20]),
                    ("north", [0, -20, 100, 20]),
                    ("south", [0, 20, 100, 20]),
                ] {
                    dome.add_monitor(monitor(name, frame));
                }
            }
            Self::Master => {
                for title in ["w0", "w1", "w2"] {
                    dome.open(window(title));
                }
            }
        }
        dome
    }
}

const VERBS: [(&str, &str, Fixture); 32] = [
    ("focus_left()", "focus left", Fixture::Cross),
    ("focus_right()", "focus right", Fixture::Cross),
    ("focus_up()", "focus up", Fixture::Cross),
    ("focus_down()", "focus down", Fixture::Cross),
    ("focus_parent()", "focus parent", Fixture::Cross),
    ("focus_tab_next()", "focus tab next", Fixture::Tabbed),
    ("focus_tab_prev()", "focus tab prev", Fixture::Tabbed),
    (
        "focus_monitor_left()",
        "focus monitor left",
        Fixture::Monitors,
    ),
    (
        "focus_monitor_right()",
        "focus monitor right",
        Fixture::Monitors,
    ),
    ("focus_monitor_up()", "focus monitor up", Fixture::Monitors),
    (
        "focus_monitor_down()",
        "focus monitor down",
        Fixture::Monitors,
    ),
    ("move_left()", "move left", Fixture::Cross),
    ("move_right()", "move right", Fixture::Cross),
    ("move_up()", "move up", Fixture::Cross),
    ("move_down()", "move down", Fixture::Cross),
    (
        "move_to_monitor_left()",
        "move monitor left",
        Fixture::Monitors,
    ),
    (
        "move_to_monitor_right()",
        "move monitor right",
        Fixture::Monitors,
    ),
    ("move_to_monitor_up()", "move monitor up", Fixture::Monitors),
    (
        "move_to_monitor_down()",
        "move monitor down",
        Fixture::Monitors,
    ),
    ("toggle_split()", "toggle spawn", Fixture::Cross),
    ("rotate()", "toggle direction", Fixture::Cross),
    ("toggle_tabbed()", "toggle layout", Fixture::Cross),
    ("toggle_float()", "toggle float", Fixture::Cross),
    ("toggle_fullscreen()", "toggle fullscreen", Fixture::Cross),
    ("grow()", "grow", Fixture::Master),
    ("shrink()", "shrink", Fixture::Master),
    ("increase_master_count()", "master more", Fixture::Master),
    ("decrease_master_count()", "master fewer", Fixture::Master),
    ("focus_workspace('1')", "focus workspace 1", Fixture::Cross),
    ("move_to_workspace('1')", "move workspace 1", Fixture::Cross),
    (
        "focus_monitor('east')",
        "focus monitor east",
        Fixture::Monitors,
    ),
    (
        "move_to_monitor('east')",
        "move monitor east",
        Fixture::Monitors,
    ),
];

#[test]
fn press_tiling_verb_matches_its_cli_command() {
    for (call, command, fixture) in VERBS {
        let mut pressed = fixture.boot(call);
        let before = pressed.snapshot();
        pressed.press("main", "x");
        let after = pressed.snapshot();
        assert_ne!(after, before, "{call} changes nothing in its fixture");
        let mut ran = fixture.boot(call);
        ran.snapshot();
        ran.run(command);
        assert_eq!(after, ran.snapshot(), "{call} against `{command}`");
    }
}

#[test]
fn press_binding_exposes_exactly_names_type_stub_lists() {
    let stub: BTreeSet<String> = META_LUA
        .lines()
        .skip_while(|line| *line != "---@class Actions")
        .skip(1)
        .take_while(|line| line.starts_with("---@field "))
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(str::to_owned)
        .collect();
    let expected = stub.into_iter().collect::<Vec<_>>().join(",");
    let mut dome = TestDome::new(&format!(
        "local expected = {expected:?}
        return {{ keymaps = {{ main = {{ x = function(actions)
            local names = {{}}
            for name in pairs(actions) do
                table.insert(names, name)
            end
            table.sort(names)
            assert(table.concat(names, ',') == expected)
            actions.execute('action names match')
        end }} }} }}"
    ));
    dome.press("main", "x");
    assert_snapshot!(dome.snapshot(), @r#"
    focused none on primary
    workspaces: 0 on primary (focused)
    effects: execute "action names match"
    primary 0,0 100x20 border 1
    "#);
}

#[test]
fn load_keymaps_table_replaces_bundled_and_absent_or_non_table_keeps_bundled() {
    let bundled = load_config("return dome.defaults()").keymaps;
    assert!(!bundled.is_empty());
    assert_eq!(load_config("return {}").keymaps, bundled);
    let loaded = load_config("return { keymaps = 5, border_size = 7 }");
    assert_eq!(loaded.keymaps, bundled);
    assert_eq!(loaded.config.tiling.border_size, Pixels::new(7));
    let loaded = load_config(
        "return { keymaps = { main = { ['meta+h'] = function(a) a.focus_left() end } } }",
    );
    assert_eq!(loaded.keymaps, ["main meta+h"]);
}

#[test]
fn load_keymaps_with_several_modes_publishes_each_mode() {
    let loaded = load_config(
        "return { keymaps = {
            main = {
                ['meta+h'] = function(a) a.focus_left() end,
                ['meta+r'] = function(a) a.mode('resize') end,
            },
            resize = {
                h = function(a) a.focus_left() end,
                escape = function(a) a.mode('main') end,
            },
        } }",
    );
    assert_eq!(
        loaded.keymaps,
        ["main meta+h", "main meta+r", "resize escape", "resize h"]
    );
}

#[test]
fn load_unusable_binding_drops_it_and_keeps_rest() {
    let loaded = load_config(
        "return { keymaps = {
            main = {
                ['meta+a'] = function(a) a.focus_left() end,
                ['unkmod+h'] = function(a) a.focus_left() end,
                ['meta+b'] = 'focus left',
                [''] = function(a) a.close() end,
            },
            [''] = { h = function(a) a.focus_left() end },
            [false] = { h = function(a) a.focus_left() end },
        } }",
    );
    assert_eq!(loaded.keymaps, ["main meta+a"]);
}

const SPELLINGS: [(&str, &str); 35] = [
    ("tostring(Meta + 'h')", "meta+h"),
    ("tostring(Meta) .. '+h'", "meta+h"),
    ("Meta + 'h'", "meta+h"),
    ("Cmd + 'h'", "meta+h"),
    ("Win + 'h'", "meta+h"),
    ("'cmd+h'", "meta+h"),
    ("'Meta+H'", "meta+h"),
    ("Option + 'a'", "alt+a"),
    ("Opt + 'a'", "alt+a"),
    ("'option+a'", "alt+a"),
    ("Control + 'c'", "ctrl+c"),
    ("'control+c'", "ctrl+c"),
    ("(Ctrl + Meta) + 'y'", "meta+ctrl+y"),
    ("(Cmd + Control) + 'y'", "meta+ctrl+y"),
    ("Meta + Alt + Ctrl + Shift + 'x'", "meta+ctrl+alt+shift+x"),
    ("Space", "space"),
    ("Meta + Space", "meta+space"),
    ("Meta + 'space'", "meta+space"),
    ("'meta+space'", "meta+space"),
    ("Enter", "return"),
    ("Return", "return"),
    ("'enter'", "return"),
    ("Meta + 'enter'", "meta+return"),
    ("'Meta+Enter'", "meta+return"),
    ("Meta + Shift + Enter", "meta+shift+return"),
    ("Esc", "escape"),
    ("Escape", "escape"),
    ("Tab", "tab"),
    ("Backspace", "backspace"),
    ("Up", "up"),
    ("Down", "down"),
    ("Left", "left"),
    ("Right", "right"),
    ("Ctrl + Left", "ctrl+left"),
    ("'ctrl+left'", "ctrl+left"),
];

#[test]
fn load_keystroke_spellings_publish_one_canonical_keystroke() {
    for (expression, expected) in SPELLINGS {
        let loaded = load_config(&format!(
            "return {{ keymaps = {{ main = {{ [{expression}] = function(a) a.close() end }} }} }}"
        ));
        assert_eq!(loaded.keymaps, [format!("main {expected}")], "{expression}");
    }
    let loaded = load_config(
        "return { keymaps = { main = {
            [Meta + Space] = function(a) a.close() end,
            ['meta+space'] = function(a) a.close() end,
        } } }",
    );
    assert_eq!(loaded.keymaps, ["main meta+space"]);
}

#[test]
fn load_second_dome_defaults_call_returns_fresh_table() {
    let bundled = load_config("return dome.defaults()").keymaps;
    assert!(!bundled.iter().any(|binding| binding == "main meta+x"));
    let loaded = load_config(
        "local config = dome.defaults()
        config.keymaps.main[Meta + 'x'] = function(a) a.close() end
        return dome.defaults()",
    );
    assert_eq!(loaded.keymaps, bundled);
}

fn alt_swapped_for_meta(binding: &str) -> String {
    let (mode, keystroke) = binding
        .split_once(' ')
        .expect("published binding has mode and keystroke");
    let mut keystroke: Keystroke = keystroke.parse().expect("published keystroke parses");
    keystroke.modifiers.remove(Modifiers::ALT);
    keystroke.modifiers.insert(Modifiers::META);
    format!("{mode} {keystroke}")
}

#[test]
fn load_with_default_modifier_swaps_modifier_and_rejects_others() {
    let bundled = load_config("return dome.defaults()").keymaps;
    let mut meta: Vec<String> = bundled
        .iter()
        .map(|binding| alt_swapped_for_meta(binding))
        .collect();
    meta.sort();
    for (modifier, expected) in [("Alt", &bundled), ("Meta", &meta), ("Cmd", &meta)] {
        let loaded = load_config(&format!(
            "return dome.with_default_modifier({modifier}).defaults()"
        ));
        assert_eq!(&loaded.keymaps, expected, "{modifier}");
    }
    for modifier in ["Ctrl", "Meta + Ctrl", "'meta'"] {
        let loaded = load_config(&format!(
            "local config = dome.with_default_modifier({modifier}).defaults()
            config.border_size = 7
            return config"
        ));
        assert_eq!(
            loaded.config.tiling.border_size,
            Pixels::new(4),
            "{modifier}"
        );
        assert_eq!(loaded.keymaps, bundled, "{modifier}");
    }
}

#[test]
fn load_invalid_key_expression_fails_load() {
    let bundled = load_config("return dome.defaults()").keymaps;
    for expression in [
        "Meta + 'h' + 'j'",
        "Space + Meta",
        "'h' + Meta",
        "Meta + 1",
        "Meta + newproxy()",
        "Meta + ''",
    ] {
        let loaded = load_config(&format!(
            "return {{ border_size = 7, keymaps = {{ main = {{
                [{expression}] = function(a) a.close() end,
            }} }} }}"
        ));
        assert_eq!(
            loaded.config.tiling.border_size,
            Pixels::new(4),
            "{expression}"
        );
        assert_eq!(loaded.keymaps, bundled, "{expression}");
    }
}
