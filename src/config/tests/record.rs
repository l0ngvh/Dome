use crate::config::Config;
use crate::core::{Pixels, SizeConstraint, Strategy, WindowMatcher};
use crate::theme::Flavor;

use super::{CleanupFile, config_from, temp_lua_path};

#[test]
fn size_constraint_parses_float_as_pixels() {
    let config = config_from("return { minimum_width = 200.0 }");
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
}

#[test]
fn size_constraint_parses_int_as_pixels() {
    let config = config_from("return { minimum_width = 200 }");
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
}

#[test]
fn size_constraint_parses_string_percent() {
    let config = config_from(r#"return { minimum_width = "10%" }"#);
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Percent(10.0)
    );
}

#[test]
fn size_constraint_recovers_from_invalid_percent() {
    let default_min = super::tiling_config().size_constraints.minimum_width;
    for src in [
        r#"return { minimum_width = "101%" }"#,
        r#"return { minimum_width = "-5%" }"#,
    ] {
        assert_eq!(
            config_from(src).tiling.size_constraints.minimum_width,
            default_min,
            "{src}"
        );
    }
}

#[test]
fn size_constraint_recovers_from_unusable_pixels() {
    let default_min = super::tiling_config().size_constraints.minimum_width;
    for expr in ["-100", "100.5", "0/0", "math.huge", "-math.huge"] {
        let config = config_from(&format!("return {{ minimum_width = {expr} }}"));
        assert_eq!(
            config.tiling.size_constraints.minimum_width, default_min,
            "{expr} should fall back to the default"
        );
    }
}

#[test]
fn size_constraint_recovers_from_string_without_percent() {
    assert_eq!(
        config_from(r#"return { minimum_width = "200" }"#)
            .tiling
            .size_constraints
            .minimum_width,
        super::tiling_config().size_constraints.minimum_width
    );
}

#[test]
fn a_rejected_field_leaves_its_siblings_alone() {
    let config = config_from(r#"return { minimum_width = "200", start_at_login = true }"#);
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        super::tiling_config().size_constraints.minimum_width
    );
    assert!(config.start_at_login);
}

#[test]
fn size_constraint_reverts_an_inconsistent_pair() {
    let defaults = super::tiling_config().size_constraints;
    let config = config_from("return { minimum_width = 200, maximum_width = 100 }");
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        defaults.minimum_width
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_width,
        defaults.maximum_width
    );

    let config = config_from("return { minimum_height = 200, maximum_height = 100 }");
    assert_eq!(
        config.tiling.size_constraints.minimum_height,
        defaults.minimum_height
    );

    // A zero maximum means unlimited, so this pair is consistent and stands.
    let config = config_from("return { minimum_width = 200, maximum_width = 0 }");
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );

    // Two percentages order the same way on every screen, so this pair reverts too.
    let config = config_from(r#"return { minimum_width = "80%", maximum_width = "10%" }"#);
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        defaults.minimum_width
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_width,
        defaults.maximum_width
    );

    // A pixel minimum against a percentage maximum orders differently on each
    // screen, so both stand.
    let config = config_from(r#"return { minimum_width = 900, maximum_width = "10%" }"#);
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(900))
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_width,
        SizeConstraint::Percent(10.0)
    );
}

#[test]
fn start_at_login_parses_true() {
    assert!(config_from("return { start_at_login = true }").start_at_login);
}

#[test]
fn theme_reads_every_flavor() {
    for (input, expected) in [
        ("latte", Flavor::Latte),
        ("frappe", Flavor::Frappe),
        ("macchiato", Flavor::Macchiato),
        ("mocha", Flavor::Mocha),
    ] {
        let src = format!("return {{ theme = \"{input}\" }}");
        assert_eq!(config_from(&src).appearance.theme, expected);
    }
}

#[test]
fn theme_recovers_from_unknown_flavor() {
    assert_eq!(
        config_from(r#"return { theme = "dracula" }"#)
            .appearance
            .theme,
        super::config().appearance.theme
    );
}

#[test]
fn font_missing_is_default() {
    assert_eq!(
        config_from("return {}").appearance.font,
        super::config().appearance.font
    );
}

#[test]
fn font_reads_via_config() {
    let config = config_from("return { font_size = 18.0 }");
    assert_eq!(config.appearance.font.size, 18.0);
}

#[test]
fn zero_tab_bar_height_falls_back_to_default() {
    let config = config_from("return { partition_tree = { tab_bar_height = 0 } }");
    assert_eq!(
        config.tiling.partition_tree.tab_bar_height,
        super::tiling_config().partition_tree.tab_bar_height
    );
}

#[test]
fn master_ratio_out_of_range_falls_back_to_default() {
    let config = config_from("return { master = { master_ratio = 1.5, master_count = 3 } }");
    assert_eq!(
        config.tiling.master.master_ratio,
        super::tiling_config().master.master_ratio
    );
    assert_eq!(config.tiling.master.master_count, 3);
}

#[test]
fn master_count_beyond_exact_f64_range_falls_back_to_default() {
    let config = config_from("return { master = { master_count = 1e18 } }");
    assert_eq!(
        config.tiling.master.master_count,
        super::tiling_config().master.master_count
    );
}

#[test]
fn non_finite_master_ratio_falls_back_to_default() {
    let config = config_from("return { master = { master_ratio = 0/0 } }");
    assert_eq!(
        config.tiling.master.master_ratio,
        super::tiling_config().master.master_ratio
    );
}

#[test]
fn font_family_blank_falls_back_to_default() {
    let config = config_from(r#"return { font_family = "   ", font_size = 18.0 }"#);
    assert_eq!(config.appearance.font.family, None);
    assert_eq!(config.appearance.font.size, 18.0);
}

#[test]
fn font_size_out_of_range_falls_back_to_default() {
    for src in ["return { font_size = 2.0 }", "return { font_size = 200.0 }"] {
        assert_eq!(
            config_from(src).appearance.font.size,
            super::config().appearance.font.size,
            "{src}"
        );
    }
}

#[test]
fn a_group_set_in_part_keeps_the_other_defaults() {
    let config = config_from("return { partition_tree = { tab_bar_height = 30 } }");
    assert_eq!(config.tiling.partition_tree.tab_bar_height.value(), 30);
    assert_eq!(
        config.tiling.partition_tree.automatic_tiling,
        super::tiling_config().partition_tree.automatic_tiling
    );
}

#[test]
fn partition_tree_config_parses_fields() {
    let config = config_from(
        "return { partition_tree = { tab_bar_height = 30.0, automatic_tiling = false } }",
    );
    assert_eq!(config.tiling.partition_tree.tab_bar_height.value(), 30);
    assert!(!config.tiling.partition_tree.automatic_tiling);
}

#[test]
fn layout_parses_master_strategy() {
    let defaults = super::tiling_config();
    let config = config_from(r#"return { layout = "master" }"#);
    assert_eq!(config.tiling.layout, Strategy::Master);
    assert_eq!(
        config.tiling.partition_tree.tab_bar_height,
        defaults.partition_tree.tab_bar_height
    );
    assert_eq!(
        config.tiling.master.master_ratio,
        defaults.master.master_ratio
    );
}

#[test]
fn tiling_parses_master_params() {
    let config = config_from("return { master = { master_ratio = 0.3, master_count = 2 } }");
    assert_eq!(config.tiling.master.master_ratio, 0.3);
    assert_eq!(config.tiling.master.master_count, 2);
}

#[test]
fn a_config_without_keymaps_keeps_the_bundled_bindings() {
    let bundled = config_from("return dome.defaults()").keymaps.modes["main"].len();
    assert!(bundled > 0, "the bundled config should bind something");
    let config = config_from(r#"return { layout = "master" }"#);
    assert_eq!(config.keymaps.modes["main"].len(), bundled);
}

#[test]
fn config_recovers_from_unknown_strategy() {
    assert_eq!(
        config_from(r#"return { layout = "floating" }"#)
            .tiling
            .layout,
        super::tiling_config().layout
    );
}

#[test]
fn config_load_parses_root_schema() {
    let config = config_from(
        r#"return {
  layout = "master",
  partition_tree = { tab_bar_height = 32.0 },
  master = { master_ratio = 0.6, master_count = 2 },
}"#,
    );
    assert_eq!(config.tiling.layout, Strategy::Master);
    assert_eq!(config.tiling.partition_tree.tab_bar_height.value(), 32);
    assert_eq!(config.tiling.master.master_ratio, 0.6);
    assert_eq!(config.tiling.master.master_count, 2);
}

#[test]
fn config_parses_size_constraints() {
    let config = config_from(
        r#"return { minimum_width = 200, maximum_width = "50%", minimum_height = 100, maximum_height = 0 }"#,
    );
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        SizeConstraint::Pixels(Pixels::new(200))
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_width,
        SizeConstraint::Percent(50.0)
    );
    assert_eq!(
        config.tiling.size_constraints.minimum_height,
        SizeConstraint::Pixels(Pixels::new(100))
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_height,
        SizeConstraint::Pixels(Pixels::new(0))
    );
}

#[test]
fn config_load_reverts_an_inconsistent_pair_and_keeps_the_rest() {
    let path = temp_lua_path("validate_fail");
    std::fs::write(
        &path,
        "return { minimum_width = 100, maximum_width = 50, layout = \"master\" }\n",
    )
    .unwrap();
    let _cleanup = CleanupFile(path.clone());
    let config = Config::load_from_path(path.to_str().unwrap()).unwrap();
    let defaults = super::tiling_config().size_constraints;
    assert_eq!(
        config.tiling.size_constraints.minimum_width,
        defaults.minimum_width
    );
    assert_eq!(
        config.tiling.size_constraints.maximum_width,
        defaults.maximum_width
    );
    assert_eq!(config.tiling.layout, Strategy::Master);
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
        config.tiling.ignore[0],
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
    // interprets the delimiters, not the config reader.
    assert_eq!(
        config.tiling.ignore[1],
        WindowMatcher {
            class: Some(r"/^MessageWindowClass\+/".to_string()),
            ..WindowMatcher::default()
        }
    );
}
