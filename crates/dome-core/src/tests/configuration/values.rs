use crate::core::{Length, SizeConstraint, SizeConstraints, Strategy};
use crate::tests::fixtures::configuration::load_config;
use crate::theme::Flavor;

#[test]
fn load_percentage_near_half_pixel_preserves_resolved_rounding() {
    let loaded = load_config("return { minimum_width = '10.49999999%' }");
    let width = loaded
        .config
        .tiling
        .size_constraints
        .minimum_width
        .resolve(crate::FractionalLength::new(100.0), 1.0);
    assert_eq!(Length::round(width), Length::new(10));
}

#[test]
fn load_unusable_value_falls_back_while_siblings_load() {
    let loaded = load_config("return { minimum_width = '200', start_at_login = true }");
    assert_eq!(
        loaded.config.tiling.size_constraints.minimum_width,
        SizeConstraint::Percent(5.0)
    );
    assert!(loaded.config.start_at_login);

    let loaded =
        load_config("return { minimum_width = 100, maximum_width = 50, layout = 'master' }");
    assert_eq!(
        loaded.config.tiling.size_constraints.minimum_width,
        SizeConstraint::Percent(5.0)
    );
    assert_eq!(
        loaded.config.tiling.size_constraints.maximum_width,
        SizeConstraint::Pixels(Length::new(0))
    );
    assert_eq!(loaded.config.tiling.layout, Strategy::Master);

    for value in ["9.5", "2147483648"] {
        let loaded = load_config(&format!(
            "return {{ border_size = {value}, start_at_login = true }}"
        ));
        assert_eq!(loaded.config.tiling.border_size, Length::new(4), "{value}");
        assert!(loaded.config.start_at_login, "{value}");
    }

    let loaded = load_config(
        "return setmetatable({ border_size = 7 }, { __index = function(_, key)
            if key == 'start_at_login' then error('unavailable') end
        end })",
    );
    assert!(!loaded.config.start_at_login);
    assert_eq!(loaded.config.tiling.border_size, Length::new(7));
}

#[test]
fn load_removed_float_and_fullscreen_keys_reads_rest_unchanged() {
    let with_lists = load_config(
        "return { layout = 'master', float = { { app = 'Calculator' } }, fullscreen = { { app = 'Steam' } } }",
    );
    let without_lists = load_config("return { layout = 'master' }");
    assert_eq!(with_lists.config.tiling, without_lists.config.tiling);
    assert_eq!(with_lists.config.tiling.layout, Strategy::Master);
}

#[test]
fn load_one_key_of_group_keeps_bundled_other_keys() {
    for source in [
        "return { partition_tree = { tab_bar_height = 30 } }",
        "local config = dome.defaults() config.partition_tree.tab_bar_height = 30 return config",
    ] {
        let loaded = load_config(source);
        assert_eq!(
            loaded.config.tiling.partition_tree.tab_bar_height,
            Length::new(30),
            "{source}"
        );
        assert!(
            loaded.config.tiling.partition_tree.automatic_tiling,
            "{source}"
        );
    }
}

#[test]
fn load_size_constraint_reads_pixels_or_percent_and_falls_back_on_unusable() {
    let bundled = SizeConstraint::Percent(5.0);
    for (value, expected) in [
        ("200", SizeConstraint::Pixels(Length::new(200))),
        ("200.0", SizeConstraint::Pixels(Length::new(200))),
        ("'10%'", SizeConstraint::Percent(10.0)),
        ("'101%'", bundled),
        ("'-5%'", bundled),
        ("-100", bundled),
        ("100.5", bundled),
        ("0/0", bundled),
        ("math.huge", bundled),
        ("-math.huge", bundled),
        ("'200'", bundled),
        ("true", bundled),
    ] {
        let loaded = load_config(&format!("return {{ minimum_width = {value} }}"));
        assert_eq!(
            loaded.config.tiling.size_constraints.minimum_width, expected,
            "{value}"
        );
    }
    let loaded = load_config(
        "return { minimum_width = 200, maximum_width = '50%', minimum_height = 100, maximum_height = 0 }",
    );
    assert_eq!(
        loaded.config.tiling.size_constraints,
        SizeConstraints {
            minimum_width: SizeConstraint::Pixels(Length::new(200)),
            maximum_width: SizeConstraint::Percent(50.0),
            minimum_height: SizeConstraint::Pixels(Length::new(100)),
            maximum_height: SizeConstraint::Pixels(Length::new(0)),
        }
    );
}

#[test]
fn load_size_pairs_with_ordered_or_inverted_bounds_preserves_or_reverts_pair() {
    for (axis, minimum, maximum, expected_minimum, expected_maximum) in [
        (
            "width",
            "100",
            "200",
            SizeConstraint::Pixels(Length::new(100)),
            SizeConstraint::Pixels(Length::new(200)),
        ),
        (
            "width",
            "200",
            "100",
            SizeConstraint::Percent(5.0),
            SizeConstraint::Pixels(Length::new(0)),
        ),
        (
            "height",
            "200",
            "100",
            SizeConstraint::Percent(5.0),
            SizeConstraint::Pixels(Length::new(0)),
        ),
        (
            "width",
            "'80%'",
            "'10%'",
            SizeConstraint::Percent(5.0),
            SizeConstraint::Pixels(Length::new(0)),
        ),
        (
            "width",
            "200",
            "0",
            SizeConstraint::Pixels(Length::new(200)),
            SizeConstraint::Pixels(Length::new(0)),
        ),
        (
            "width",
            "900",
            "'10%'",
            SizeConstraint::Pixels(Length::new(900)),
            SizeConstraint::Percent(10.0),
        ),
    ] {
        let source = format!("return {{ minimum_{axis} = {minimum}, maximum_{axis} = {maximum} }}");
        let loaded = load_config(&source);
        let constraints = loaded.config.tiling.size_constraints;
        let actual = match axis {
            "width" => (constraints.minimum_width, constraints.maximum_width),
            _ => (constraints.minimum_height, constraints.maximum_height),
        };
        assert_eq!(actual, (expected_minimum, expected_maximum), "{source}");
    }
}

#[test]
fn load_layout_with_valid_or_unusable_values_reads_strategy_or_falls_back() {
    for (value, expected) in [
        ("'master'", Strategy::Master),
        ("'scrolling'", Strategy::Scrolling),
        ("'floating'", Strategy::PartitionTree),
        ("true", Strategy::PartitionTree),
    ] {
        let loaded = load_config(&format!("return {{ layout = {value} }}"));
        assert_eq!(loaded.config.tiling.layout, expected, "{value}");
    }
}

#[test]
fn load_partition_tree_reads_values_and_zero_tab_bar_falls_back() {
    let loaded = load_config(
        "return { partition_tree = { tab_bar_height = 30, automatic_tiling = false } }",
    );
    assert_eq!(
        loaded.config.tiling.partition_tree.tab_bar_height,
        Length::new(30)
    );
    assert!(!loaded.config.tiling.partition_tree.automatic_tiling);
    let loaded = load_config("return { partition_tree = { tab_bar_height = 0 } }");
    assert_eq!(
        loaded.config.tiling.partition_tree.tab_bar_height,
        Length::new(24)
    );
}

#[test]
fn load_master_reads_ratio_and_count_and_falls_back_out_of_range() {
    for (fields, ratio, count) in [
        ("master_ratio = 0.3, master_count = 2", 0.3, 2),
        ("master_ratio = 0", 0.0, 1),
        ("master_ratio = 1", 1.0, 1),
        ("master_ratio = 1.5, master_count = 3", 0.5, 3),
        ("master_ratio = 0/0", 0.5, 1),
        ("master_count = 1e18", 0.5, 1),
        ("master_count = 0", 0.5, 1),
    ] {
        let loaded = load_config(&format!("return {{ master = {{ {fields} }} }}"));
        assert_eq!(loaded.config.tiling.master.master_ratio, ratio, "{fields}");
        assert_eq!(loaded.config.tiling.master.master_count, count, "{fields}");
    }
}

#[test]
fn load_scrolling_column_width_reads_size_constraint() {
    let loaded = load_config("return { scrolling = { column_width = '60%' } }");
    assert_eq!(
        loaded.config.tiling.scrolling.column_width,
        SizeConstraint::Percent(60.0)
    );
}

#[test]
fn load_theme_reads_each_flavor_and_falls_back_on_unknown() {
    for (value, expected) in [
        ("latte", Flavor::Latte),
        ("frappe", Flavor::Frappe),
        ("macchiato", Flavor::Macchiato),
        ("mocha", Flavor::Mocha),
        ("dracula", Flavor::Mocha),
    ] {
        let loaded = load_config(&format!("return {{ theme = '{value}' }}"));
        assert_eq!(loaded.config.appearance.theme, expected, "{value}");
    }
}

#[test]
fn load_font_size_reads_value_and_falls_back_out_of_range() {
    for (value, expected) in [(18, 18.0), (2, 14.0), (200, 14.0)] {
        let loaded = load_config(&format!("return {{ font_size = {value} }}"));
        assert_eq!(loaded.config.appearance.font.size, expected, "{value}");
    }
}

#[test]
fn load_font_family_with_nonblank_or_blank_value_preserves_name_or_falls_back() {
    for (value, expected) in [("   ", None), ("  Fira Code  ", Some("  Fira Code  "))] {
        let loaded = load_config(&format!("return {{ font_family = '{value}' }}"));
        assert_eq!(loaded.config.appearance.font.family.as_deref(), expected);
    }
}

#[test]
fn load_start_at_login_with_boolean_or_unusable_value_reads_flag_or_falls_back() {
    for (value, expected) in [("true", true), ("'true'", false)] {
        let loaded = load_config(&format!("return {{ start_at_login = {value} }}"));
        assert_eq!(loaded.config.start_at_login, expected, "{value}");
    }
}

#[test]
fn load_env_keeps_valid_entries_and_drops_unusable_ones() {
    assert!(load_config("return {}").config.env.is_empty());
    let loaded = load_config(
        "return { env = {
            EDITOR = 'nvim', EMPTY = '',
            ['A=B'] = 'x', [''] = 'x',
            ['A' .. string.char(0) .. 'B'] = 'x',
            BAD = 'a' .. string.char(0) .. 'b', COUNT = 42,
            [1] = 'integer', [1.5] = 'fraction', [true] = 'boolean',
            [{}] = 'table', [string.char(255)] = 'non-Unicode',
        } }",
    );
    assert_eq!(
        loaded.config.env,
        std::collections::HashMap::from([
            ("EDITOR".to_owned(), "nvim".to_owned()),
            ("EMPTY".to_owned(), String::new()),
        ])
    );
}

#[cfg(target_os = "windows")]
#[test]
fn load_env_names_differing_only_in_case_keeps_one() {
    let loaded = load_config("return { env = { Path = 'a', PATH = 'b' } }");
    assert_eq!(loaded.config.env.len(), 1);
    assert!(matches!(
        loaded.config.env.get("PATH").map(String::as_str),
        Some("a" | "b")
    ));
}

#[test]
fn load_dome_os_reads_current_platform() {
    let loaded = load_config("return { env = { DOME_OS = dome.os } }");
    assert_eq!(
        loaded.config.env.get("DOME_OS").map(String::as_str),
        Some(std::env::consts::OS)
    );
}

#[test]
fn load_dome_env_reads_launch_environment_and_rejects_assignment() {
    let path = std::env::var("PATH").expect("the test process should have PATH");
    let loaded = load_config(
        "return { env = { COPIED = dome.env.PATH, ABSENT = dome.env.DOME_NAME_THAT_NOTHING_SETS } }",
    );
    assert_eq!(loaded.config.env.get("COPIED"), Some(&path));
    assert!(!loaded.config.env.contains_key("ABSENT"));
    let loaded = load_config("dome.env.PATH = '/tmp' return { border_size = 7 }");
    assert_eq!(loaded.config.tiling.border_size, Length::new(4));
}

#[test]
fn load_assignment_to_dome_table_fails_load() {
    for assignment in ["dome.os = 'x'", "dome.defaults = 1", "dome.new_field = 1"] {
        let loaded = load_config(&format!("{assignment} return {{ border_size = 7 }}"));
        assert_eq!(
            loaded.config.tiling.border_size,
            Length::new(4),
            "{assignment}"
        );
    }
}

#[test]
fn load_log_level_reads_each_level() {
    for level in ["trace", "debug", "info", "warn", "error"] {
        let loaded = load_config(&format!("return {{ log_level = '{level}' }}"));
        assert_eq!(loaded.config.log_level.as_str(), level);
    }
}
