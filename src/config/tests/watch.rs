use crate::config::watch::load_or_else;
use crate::config::{Appearance, Config, PreferredLayouts};
use crate::core::PreferredWorkspace;
use crate::font::FontConfig;

use super::{CleanupFile, temp_lua_path};

// No config file in these tests can parse to this marker, so the assertion
// holds only when the fallback ran.
const FALLBACK_MARKER: &str = "dome_test_fallback";

fn sentinel_config() -> Config {
    let base = super::config();
    Config {
        appearance: Appearance {
            font: FontConfig {
                family: Some(FALLBACK_MARKER.to_string()),
                ..base.appearance.font
            },
            ..base.appearance
        },
        ..base
    }
}

fn sentinel_layout() -> PreferredLayouts {
    let mut layout = PreferredLayouts::default();
    layout.insert(
        FALLBACK_MARKER,
        FALLBACK_MARKER,
        PreferredWorkspace::PartitionTree {
            tree: None,
            float: Vec::new(),
            fullscreen: Vec::new(),
        },
    );
    layout
}

#[test]
fn load_or_else_returns_defaults_when_path_missing() {
    let path = temp_lua_path("does_not_exist");
    let config = load_or_else(
        path.to_str().unwrap(),
        Config::load_from_path,
        sentinel_config,
    );
    assert_eq!(
        config.appearance.font.family.as_deref(),
        Some(FALLBACK_MARKER)
    );
}

#[test]
fn load_or_else_returns_parsed_config_on_valid_lua() {
    let path = temp_lua_path("valid");
    std::fs::write(
        &path,
        r#"return { log_level = "debug", start_at_login = true }"#,
    )
    .unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = load_or_else(
        path.to_str().unwrap(),
        Config::load_from_path,
        super::config,
    );
    assert_eq!(config.log_level.as_str(), "debug");
    assert!(config.start_at_login);
}

#[test]
fn load_or_else_returns_defaults_on_malformed_lua() {
    let path = temp_lua_path("malformed");
    std::fs::write(&path, "this is = = not valid lua\n").unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = load_or_else(
        path.to_str().unwrap(),
        Config::load_from_path,
        sentinel_config,
    );
    assert_eq!(
        config.appearance.font.family.as_deref(),
        Some(FALLBACK_MARKER)
    );
}

#[test]
fn layout_load_or_else_returns_defaults_on_malformed() {
    let path = temp_lua_path("layout_malformed");
    std::fs::write(&path, "this is not valid lua ]]}\n").unwrap();
    let _cleanup = CleanupFile(path.clone());
    let layout = load_or_else(
        path.to_str().unwrap(),
        PreferredLayouts::load,
        sentinel_layout,
    );
    assert_eq!(layout, sentinel_layout());
}
