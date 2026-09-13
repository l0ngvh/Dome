use crate::config::watch::load_or_default;
use crate::config::{Config, PreferredLayouts};
use crate::core::{MasterConfig, PartitionTreeConfig, Strategy};

use super::{CleanupFile, temp_lua_path};

#[test]
fn load_or_default_returns_defaults_when_path_missing() {
    let path = temp_lua_path("does_not_exist");
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(config.log_level.as_str(), "info");
    assert!(!config.start_at_login);
}

#[test]
fn load_or_default_returns_parsed_config_on_valid_lua() {
    let path = temp_lua_path("valid");
    std::fs::write(
        &path,
        r#"return { log_level = "debug", start_at_login = true }"#,
    )
    .unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(config.log_level.as_str(), "debug");
    assert!(config.start_at_login);
}

#[test]
fn load_or_default_returns_defaults_on_malformed_lua() {
    let path = temp_lua_path("malformed");
    std::fs::write(&path, "this is = = not valid lua\n").unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(config.log_level.as_str(), "info");
}

#[test]
fn layout_load_or_default_returns_defaults_when_missing() {
    let path = temp_lua_path("layout_missing");
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(config.layout.strategy, Strategy::default());
    assert_eq!(config.layout.partition_tree, PartitionTreeConfig::default());
    assert_eq!(config.layout.master, MasterConfig::default());
}

#[test]
fn layout_load_or_default_returns_defaults_on_malformed() {
    let path = temp_lua_path("layout_malformed");
    std::fs::write(&path, "this is not valid lua ]]}\n").unwrap();
    let _cleanup = CleanupFile(path.clone());
    let layout = load_or_default(path.to_str().unwrap(), PreferredLayouts::load);
    assert!(layout.workspace.is_empty());
}
