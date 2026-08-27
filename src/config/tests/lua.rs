use crate::config::Config;
use crate::config::{Keystroke, Modifiers};
use crate::core::Pixels;

use super::{CleanupFile, config_from, temp_lua_path, try_config};

fn config_in_vm(src: &str) -> (mlua::Lua, Config) {
    let lua = crate::config::lua::new_vm().expect("vm should build");
    let config = Config::from_lua(&lua, "test config", src).expect("config should load");
    (lua, config)
}

fn keystroke(key: &str, modifiers: Modifiers) -> Keystroke {
    Keystroke {
        key: key.to_string(),
        modifiers,
    }
}

#[test]
fn plus_key_binding_survives_the_load() {
    let config = config_from(
        r#"return { keymaps = { main = { [Meta + "+"] = function(a) a.focus_left() end } } }"#,
    );
    assert!(
        config.keymaps.modes["main"].contains_key(&keystroke("+", Modifiers::META)),
        "{:?}",
        config.keymaps.modes["main"].keys().collect::<Vec<_>>()
    );
}

fn defaults_with_one_default_key_rebound(assignment: &str) -> Config {
    config_from(&format!(
        r#"local other = dome.defaults()
local bound
for key in pairs(other.keymaps.main) do bound = key break end
local c = dome.defaults()
c.keymaps.main[bound] = {assignment}
return c"#
    ))
}

#[test]
fn override_of_a_default_binding_replaces_it() {
    let default_count = config_from("return dome.defaults()").keymaps.modes["main"].len();
    let overridden = defaults_with_one_default_key_rebound("function(a) a.close() end");
    assert_eq!(overridden.keymaps.modes["main"].len(), default_count);
    let removed = defaults_with_one_default_key_rebound("nil");
    assert_eq!(removed.keymaps.modes["main"].len(), default_count - 1);
}

#[test]
fn override_of_a_default_binding_replaces_its_function() {
    // The probe keymap carries the rebound key back out, so the assertion names
    // no specific default binding.
    let (_lua, config) = config_in_vm(
        r#"local other = dome.defaults()
local bound
for key in pairs(other.keymaps.main) do bound = key break end
local c = dome.defaults()
c.keymaps.main[bound] = function() return 7 end
c.keymaps.probe = { [bound] = function() return 7 end }
return c"#,
    );
    let rebound = config.keymaps.modes["probe"]
        .keys()
        .next()
        .expect("the probe keymap names the rebound key");

    let result: i64 = config.keymaps.modes["main"][rebound]
        .call(())
        .expect("the rebound binding should call");

    assert_eq!(result, 7);
}

#[test]
fn a_config_can_set_one_key_inside_a_default_group() {
    let config =
        config_from("local c = dome.defaults() c.partition_tree.tab_bar_height = 30 return c");
    assert_eq!(config.tiling.partition_tree.tab_bar_height.value(), 30);
    assert_eq!(
        config.tiling.partition_tree.automatic_tiling,
        crate::config::tests::tiling_config()
            .partition_tree
            .automatic_tiling
    );
}

#[test]
fn dome_os_is_available_for_branching() {
    let (present, absent) = if cfg!(target_os = "macos") {
        ("meta+h", "meta+l")
    } else {
        ("meta+l", "meta+h")
    };
    let config = config_from(
        r#"local key = dome.os == "macos" and "meta+h" or "meta+l"
return { keymaps = { main = { [key] = function(a) a.focus_left() end } } }"#,
    );
    assert!(config.keymaps.modes["main"].contains_key(&present.parse::<Keystroke>().unwrap()));
    assert!(!config.keymaps.modes["main"].contains_key(&absent.parse::<Keystroke>().unwrap()));
}

#[test]
fn config_load_recovers_from_an_invalid_value() {
    let path = temp_lua_path("bad_value");
    std::fs::write(
        &path,
        "return { border_size = 9.5, start_at_login = true }\n",
    )
    .unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = Config::load_from_path(path.to_str().unwrap()).unwrap();
    assert_eq!(
        config.tiling.border_size,
        crate::config::tests::tiling_config().border_size
    );
    assert!(config.start_at_login);
}

#[test]
fn config_must_return_a_table() {
    assert!(try_config("return 42").is_err());
    assert!(try_config("local x = 1").is_err());
}

#[test]
fn modal_keymaps_base_mode_only() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+h"] = function(a) a.focus_left() end } } }"#,
    );
    assert_eq!(config.keymaps.modes.len(), 1);
    let keystroke = "meta+h".parse::<Keystroke>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&keystroke));
}

#[test]
fn keymap_function_value_becomes_a_callable_binding() {
    // The VM has to outlive the call.
    let (_lua, config) =
        config_in_vm(r#"return { keymaps = { main = { ["meta+h"] = function() return 5 end } } }"#);
    let keystroke = "meta+h".parse::<Keystroke>().unwrap();
    let binding = &config.keymaps.modes["main"][&keystroke];
    let result: i64 = binding.call(()).expect("the binding should call");
    assert_eq!(result, 5);
}

#[test]
fn load_leaves_the_returned_table_intact() {
    let src = r#"local config = {}
config.keymaps = { main = { ["meta+h"] = function() return config.keymaps ~= nil end } }
return config"#;
    let (_lua, config) = config_in_vm(src);
    let keystroke = "meta+h".parse::<Keystroke>().unwrap();
    let still_there: bool = config.keymaps.modes["main"][&keystroke]
        .call(())
        .expect("the binding should call");
    assert!(still_there);
}

#[test]
fn dome_defaults_returns_the_default_keymaps() {
    let config = config_from("return dome.defaults()");
    assert!(!config.keymaps.modes["main"].is_empty());
}

#[test]
fn load_default_config_provides_default_keymaps() {
    let lua = crate::config::lua::new_vm().unwrap();
    let config = Config::load_default(&lua).unwrap();
    assert!(!config.keymaps.modes["main"].is_empty());
}

#[test]
fn dome_defaults_override_keeps_defaults_and_adds_a_binding() {
    let default_count = config_from("return dome.defaults()").keymaps.modes["main"].len();
    let config = config_from(
        r#"local c = dome.defaults()
c.keymaps.main["meta+x"] = function(a) a.close() end
return c"#,
    );
    let meta_x = "meta+x".parse::<Keystroke>().unwrap();
    assert_eq!(config.keymaps.modes["main"].len(), default_count + 1);
    assert!(config.keymaps.modes["main"].contains_key(&meta_x));
}

#[test]
fn a_key_global_binds_the_same_keystroke_as_its_string() {
    let config = config_from(
        r#"return {
  keymaps = {
    main = {
      [Meta + Space] = function(a) a.toggle_float() end,
      [Meta + Shift + Enter] = function(a) a.close() end,
      [Return] = function(a) a.mode("main") end,
      [Ctrl + Left] = function(a) a.focus_monitor_left() end,
      [Esc] = function(a) a.mode("main") end,
    },
  },
}"#,
    );
    let main = &config.keymaps.modes["main"];
    assert_eq!(main.len(), 5);
    for spelled in [
        "meta+space",
        "meta+shift+return",
        "return",
        "ctrl+left",
        "escape",
    ] {
        let keystroke = spelled.parse::<Keystroke>().unwrap();
        assert!(main.contains_key(&keystroke), "{spelled}");
    }
}

#[test]
fn modal_keymaps_with_mode() {
    let config = config_from(
        r#"return {
  keymaps = {
    main = {
      ["meta+h"] = function(a) a.focus_left() end,
      ["meta+r"] = function(a) a.mode("resize") end,
    },
    resize = {
      ["h"] = function(a) a.focus_left() end,
      ["escape"] = function(a) a.mode("main") end,
    },
  },
}"#,
    );
    let meta_h = "meta+h".parse::<Keystroke>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&meta_h));
    let resize = config
        .keymaps
        .modes
        .get("resize")
        .expect("resize mode missing");
    let h = "h".parse::<Keystroke>().unwrap();
    assert!(resize.contains_key(&h));
    let esc = "escape".parse::<Keystroke>().unwrap();
    assert!(resize.contains_key(&esc));
}

#[test]
fn modal_keymaps_drops_empty_mode_name() {
    let config = config_from(
        r#"return {
  keymaps = {
    main = { ["meta+h"] = function(a) a.focus_left() end },
    [""] = { ["h"] = function(a) a.focus_left() end },
  },
}"#,
    );
    let meta_h = "meta+h".parse::<Keystroke>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&meta_h));
    assert!(!config.keymaps.modes.contains_key(""));
}

#[test]
fn modal_keymaps_drops_a_non_table_value() {
    let bundled = config_from("return dome.defaults()").keymaps.modes["main"].len();
    let config = config_from("return { keymaps = 5, border_size = 7 }");
    assert_eq!(config.keymaps.modes["main"].len(), bundled);
    assert_eq!(config.tiling.border_size, Pixels::new(7));
}

#[test]
fn load_drops_single_bad_keymap_binding() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+a"] = function(a) a.focus_left() end, ["unkmod+h"] = function(a) a.focus_left() end } } }"#,
    );
    let good = "meta+a".parse::<Keystroke>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&good));
    assert_eq!(config.keymaps.modes["main"].len(), 1);
}

#[test]
fn load_drops_non_function_binding() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+a"] = "focus left", ["meta+b"] = function(a) a.focus_left() end } } }"#,
    );
    let b = "meta+b".parse::<Keystroke>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&b));
    let a = "meta+a".parse::<Keystroke>().unwrap();
    assert!(!config.keymaps.modes["main"].contains_key(&a));
}
