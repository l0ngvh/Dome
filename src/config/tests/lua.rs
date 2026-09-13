use crate::config::Config;
use crate::config::watch::load_or_default;
use crate::core::{LayoutOptions, Pixels};
use crate::keybinding::Keymap;

use super::{CleanupFile, config_from, temp_lua_path, try_config};

fn config_and_registry_from(src: &str) -> (Config, Vec<mlua::Function>) {
    let lua = crate::config::lua::new_vm().expect("vm should build");
    let mut registry = Vec::new();
    let config =
        Config::from_lua(&lua, "test config", src, &mut registry).expect("config should load");
    (config, registry)
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
return { keymaps = { main = { [key] = function(a) a.focus.left() end } } }"#,
    );
    assert!(config.keymaps.modes["main"].contains_key(&present.parse::<Keymap>().unwrap()));
    assert!(!config.keymaps.modes["main"].contains_key(&absent.parse::<Keymap>().unwrap()));
}

#[test]
fn config_load_errors_on_invalid_value_then_falls_back() {
    let path = temp_lua_path("bad_value");
    std::fs::write(&path, "return { border_size = 9.5 }\n").unwrap();
    let _cleanup = CleanupFile(path.clone());
    assert!(Config::load(path.to_str().unwrap()).is_err());
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(
        config.layout.border_size,
        LayoutOptions::default_border_size()
    );
}

#[test]
fn config_must_return_a_table() {
    assert!(try_config("return 42").is_err());
    assert!(try_config("local x = 1").is_err());
}

#[test]
fn modal_keymaps_base_mode_only() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+h"] = function(a) a.focus.left() end } } }"#,
    );
    assert_eq!(config.keymaps.modes.len(), 1);
    let keymap = "meta+h".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&keymap));
}

#[test]
fn keymap_function_value_becomes_callback() {
    let (config, registry) = config_and_registry_from(
        r#"return { keymaps = { main = { ["meta+h"] = function() end } } }"#,
    );
    let keymap = "meta+h".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&keymap));
    assert_eq!(registry.len(), 1);
}

/// Loading must leave the table the config returned intact, because a binding
/// closure can still read it. The binding here reports what it sees.
#[test]
fn load_leaves_the_returned_table_intact() {
    let lua = crate::config::lua::new_vm().expect("vm should build");
    let mut registry = Vec::new();
    let src = r#"local config = {}
config.keymaps = { main = { ["meta+h"] = function() return config.keymaps ~= nil end } }
return config"#;
    Config::from_lua(&lua, "test config", src, &mut registry).expect("config should load");
    let still_there: bool = registry[0].call(()).expect("the binding should call");
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
    let mut callbacks = Vec::new();
    let config = Config::load_default_into(&lua, &mut callbacks).unwrap();
    assert!(!config.keymaps.modes["main"].is_empty());
    assert!(!callbacks.is_empty());
}

#[test]
fn dome_defaults_override_keeps_defaults_and_adds_a_binding() {
    let default_count = config_from("return dome.defaults()").keymaps.modes["main"].len();
    let config = config_from(
        r#"local c = dome.defaults()
c.keymaps.main["meta+x"] = function(a) a.close() end
return c"#,
    );
    let meta_x = "meta+x".parse::<Keymap>().unwrap();
    assert_eq!(config.keymaps.modes["main"].len(), default_count + 1);
    assert!(config.keymaps.modes["main"].contains_key(&meta_x));
}

#[test]
fn modal_keymaps_with_mode() {
    let config = config_from(
        r#"return {
  keymaps = {
    main = {
      ["meta+h"] = function(a) a.focus.left() end,
      ["meta+r"] = function(a) a.mode("resize") end,
    },
    resize = {
      ["h"] = function(a) a.focus.left() end,
      ["escape"] = function(a) a.mode("main") end,
    },
  },
}"#,
    );
    let meta_h = "meta+h".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&meta_h));
    let resize = config
        .keymaps
        .modes
        .get("resize")
        .expect("resize mode missing");
    let h = "h".parse::<Keymap>().unwrap();
    assert!(resize.contains_key(&h));
    let esc = "escape".parse::<Keymap>().unwrap();
    assert!(resize.contains_key(&esc));
}

#[test]
fn modal_keymaps_drops_empty_mode_name() {
    let config = config_from(
        r#"return {
  keymaps = {
    main = { ["meta+h"] = function(a) a.focus.left() end },
    [""] = { ["h"] = function(a) a.focus.left() end },
  },
}"#,
    );
    let meta_h = "meta+h".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&meta_h));
    assert!(!config.keymaps.modes.contains_key(""));
}

#[test]
fn modal_keymaps_drops_a_non_table_value() {
    let config = config_from("return { keymaps = 5, border_size = 7 }");
    assert!(config.keymaps.modes.is_empty());
    assert_eq!(config.layout.border_size, Pixels::new(7));
}

#[test]
fn load_drops_single_bad_keymap_binding() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+a"] = function(a) a.focus.left() end, ["unkmod+h"] = function(a) a.focus.left() end } } }"#,
    );
    let good = "meta+a".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&good));
    assert_eq!(config.keymaps.modes["main"].len(), 1);
}

#[test]
fn load_drops_non_function_binding() {
    let config = config_from(
        r#"return { keymaps = { main = { ["meta+a"] = "focus left", ["meta+b"] = function(a) a.focus.left() end } } }"#,
    );
    let b = "meta+b".parse::<Keymap>().unwrap();
    assert!(config.keymaps.modes["main"].contains_key(&b));
    let a = "meta+a".parse::<Keymap>().unwrap();
    assert!(!config.keymaps.modes["main"].contains_key(&a));
}
