use crate::config::Config;
use crate::config::watch::load_or_default;
use crate::core::{
    MasterConfig, PartitionTreeConfig, Pixels, SizeConstraint, Strategy, WindowMatcher,
};
use crate::theme::Flavor;

use super::{CleanupFile, config_from, temp_lua_path, try_config};

#[test]
fn min_size_default() {
    let config = config_from("return {}");
    assert_eq!(
        config.layout.size_constraints.minimum_width,
        SizeConstraint::Percent(5.0)
    );
    assert_eq!(
        config.layout.size_constraints.minimum_height,
        SizeConstraint::Percent(5.0)
    );
}

#[test]
fn max_size_default() {
    let config = config_from("return {}");
    assert_eq!(
        config.layout.size_constraints.maximum_width,
        SizeConstraint::Pixels(Pixels::new(0))
    );
    assert_eq!(
        config.layout.size_constraints.maximum_height,
        SizeConstraint::Pixels(Pixels::new(0))
    );
}

#[test]
fn size_constraint_parses_float_as_pixels() {
    let config = config_from("return { minimum_width = 200.0 }");
    assert_eq!(
        config.layout.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
}

#[test]
fn size_constraint_parses_int_as_pixels() {
    let config = config_from("return { minimum_width = 200 }");
    assert_eq!(
        config.layout.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
}

#[test]
fn size_constraint_parses_string_percent() {
    let config = config_from(r#"return { minimum_width = "10%" }"#);
    assert_eq!(
        config.layout.size_constraints.minimum_width,
        SizeConstraint::Percent(10.0)
    );
}

#[test]
fn size_constraint_rejects_invalid_percent() {
    assert!(try_config(r#"return { minimum_width = "101%" }"#).is_err());
    assert!(try_config(r#"return { minimum_width = "-5%" }"#).is_err());
}

#[test]
fn size_constraint_rejects_negative_pixels() {
    assert!(try_config("return { minimum_width = -100 }").is_err());
}

#[test]
fn size_constraint_rejects_fractional_pixels() {
    assert!(try_config("return { minimum_width = 100.5 }").is_err());
}

#[test]
fn size_constraint_rejects_non_finite_pixels() {
    for expr in ["0/0", "math.huge", "-math.huge"] {
        assert!(
            try_config(&format!("return {{ minimum_width = {expr} }}")).is_err(),
            "{expr} should be rejected"
        );
    }
}

#[test]
fn size_constraint_rejects_string_without_percent() {
    assert!(try_config(r#"return { minimum_width = "200" }"#).is_err());
}

#[test]
fn layout_validates_min_le_max() {
    assert!(
        config_from("return { minimum_width = 200, maximum_width = 100 }")
            .validate_layout()
            .is_err()
    );
    assert!(
        config_from("return { minimum_height = 200, maximum_height = 100 }")
            .validate_layout()
            .is_err()
    );
    assert!(
        config_from("return { minimum_width = 200, maximum_width = 0 }")
            .validate_layout()
            .is_ok()
    );
}

#[test]
fn start_at_login_defaults_to_false() {
    assert!(!config_from("return {}").start_at_login);
}

#[test]
fn start_at_login_parses_true() {
    assert!(config_from("return { start_at_login = true }").start_at_login);
}

#[test]
fn theme_deserializes() {
    assert_eq!(
        config_from(r#"return { theme = "latte" }"#)
            .appearance
            .theme,
        Flavor::Latte
    );
}

#[test]
fn font_missing_is_default() {
    assert_eq!(
        config_from("return {}").appearance.font,
        crate::font::FontConfig::default()
    );
}

#[test]
fn font_deserializes_via_config() {
    let config = config_from("return { font_size = 18.0 }");
    assert_eq!(config.appearance.font.size, 18.0);
}

#[test]
fn zero_tab_bar_height_falls_back_to_default() {
    let config = config_from("return { partition_tree = { tab_bar_height = 0 } }");
    assert_eq!(
        config.layout.partition_tree.tab_bar_height,
        PartitionTreeConfig::default().tab_bar_height
    );
}

#[test]
fn master_ratio_out_of_range_falls_back_to_default() {
    let config = config_from("return { master = { master_ratio = 1.5, master_count = 3 } }");
    assert_eq!(
        config.layout.master.master_ratio,
        MasterConfig::default().master_ratio
    );
    assert_eq!(config.layout.master.master_count, 3);
}

#[test]
fn font_family_blank_falls_back_to_default() {
    let config = config_from(r#"return { font_family = "   ", font_size = 18.0 }"#);
    assert_eq!(config.appearance.font.family, None);
    assert_eq!(config.appearance.font.size, 18.0);
}

#[test]
fn partition_tree_config_parses_fields() {
    let config = config_from(
        "return { partition_tree = { tab_bar_height = 30.0, automatic_tiling = false } }",
    );
    assert_eq!(config.layout.partition_tree.tab_bar_height.value(), 30);
    assert!(!config.layout.partition_tree.automatic_tiling);
}

#[test]
fn partition_tree_config_defaults() {
    let config = config_from("return {}");
    assert_eq!(config.layout.partition_tree.tab_bar_height.value(), 24);
    assert!(config.layout.partition_tree.automatic_tiling);
}

#[test]
fn layout_defaults_to_partition_tree() {
    let config = config_from("return {}");
    assert_eq!(config.layout.strategy, Strategy::PartitionTree);
    assert_eq!(config.layout.master.master_ratio, 0.5);
    assert_eq!(config.layout.master.master_count, 1);
}

#[test]
fn layout_parses_master_strategy() {
    let config = config_from(r#"return { strategy = "master" }"#);
    assert_eq!(config.layout.strategy, Strategy::Master);
    assert_eq!(config.layout.partition_tree.tab_bar_height.value(), 24);
    assert_eq!(config.layout.master.master_ratio, 0.5);
}

#[test]
fn layout_parses_master_params() {
    let config = config_from("return { master = { master_ratio = 0.3, master_count = 2 } }");
    assert_eq!(config.layout.master.master_ratio, 0.3);
    assert_eq!(config.layout.master.master_count, 2);
}

#[test]
fn config_rejects_unknown_strategy() {
    assert!(try_config(r#"return { strategy = "floating" }"#).is_err());
}

#[test]
fn config_load_parses_root_schema() {
    let config = config_from(
        r#"return {
  strategy = "master",
  partition_tree = { tab_bar_height = 32.0 },
  master = { master_ratio = 0.6, master_count = 2 },
}"#,
    );
    assert_eq!(config.layout.strategy, Strategy::Master);
    assert_eq!(config.layout.partition_tree.tab_bar_height.value(), 32);
    assert_eq!(config.layout.master.master_ratio, 0.6);
    assert_eq!(config.layout.master.master_count, 2);
}

#[test]
fn config_parses_size_constraints() {
    let config = config_from(
        r#"return { minimum_width = 200, maximum_width = "50%", minimum_height = 100, maximum_height = 0 }"#,
    );
    assert_eq!(
        config.layout.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
    assert_eq!(
        config.layout.size_constraints.maximum_width,
        SizeConstraint::Percent(50.0)
    );
    assert_eq!(
        config.layout.size_constraints.minimum_height,
        SizeConstraint::Pixels(Pixels::new(100))
    );
    assert_eq!(
        config.layout.size_constraints.maximum_height,
        SizeConstraint::Pixels(Pixels::new(0))
    );
}

#[test]
fn config_load_falls_back_when_validate_fails() {
    let path = temp_lua_path("validate_fail");
    std::fs::write(
        &path,
        "return { minimum_width = 100, maximum_width = 50 }\n",
    )
    .unwrap();
    let _cleanup = CleanupFile(path.clone());
    assert!(Config::load(path.to_str().unwrap()).is_err());
    let config = load_or_default(path.to_str().unwrap(), Config::load);
    assert_eq!(config.layout.strategy, Config::default().layout.strategy);
    assert_eq!(
        config.layout.partition_tree,
        Config::default().layout.partition_tree
    );
    assert_eq!(config.layout.master, Config::default().layout.master);
}

#[test]
#[cfg(target_os = "macos")]
fn macos_ignore_defaults() {
    let rules = Config::default_ignore();
    assert_eq!(rules.len(), 4);
    assert!(
        rules
            .iter()
            .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
    );
    let config = config_from("return {}");
    assert!(
        config
            .layout
            .ignore
            .iter()
            .any(|r| r.bundle_id.as_deref() == Some("com.apple.dock"))
    );
}

#[test]
#[cfg(target_os = "windows")]
fn windows_ignore_defaults() {
    let rules = Config::default_ignore();
    assert_eq!(rules.len(), 15);
    assert!(
        rules
            .iter()
            .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
    );
    let config = config_from("return {}");
    assert!(
        config
            .layout
            .ignore
            .iter()
            .any(|r| r.class.as_deref() == Some("Shell_TrayWnd"))
    );
    let core_window = config
        .layout
        .ignore
        .iter()
        .find(|r| r.class.as_deref() == Some("Windows.UI.Core.CoreWindow"))
        .expect("CoreWindow ignore rule present in merged config");
    assert!(core_window.title.is_none());
    assert!(core_window.aumid.is_none());
}

/// A dropped field still yields a list of the right length, so a length check
/// would not notice it.
#[test]
fn ignore_rules_parse_every_matcher_field() {
    let config = config_from(
        r#"return { ignore = {
            {
              app = "App",
              bundle_id = "com.example.app",
              title = "Title",
              process = "proc.exe",
              class = "ClassName",
              aumid = "App_pub!Id",
            },
            { class = "/^MessageWindowClass\\+/" },
        } }"#,
    );
    assert_eq!(
        config.layout.ignore[0],
        WindowMatcher {
            app: Some("App".to_string()),
            bundle_id: Some("com.example.app".to_string()),
            title: Some("Title".to_string()),
            process: Some("proc.exe".to_string()),
            class: Some("ClassName".to_string()),
            aumid: Some("App_pub!Id".to_string()),
        }
    );
    // The regex form stays the literal pattern string. `pattern_matches`
    // interprets the delimiters, not the deserializer.
    assert_eq!(
        config.layout.ignore[1],
        WindowMatcher {
            class: Some(r"/^MessageWindowClass\+/".to_string()),
            ..WindowMatcher::default()
        }
    );
}
