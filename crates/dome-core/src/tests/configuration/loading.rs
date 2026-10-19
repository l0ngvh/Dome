use crate::tests::fixtures::configuration::{LoadedConfig, load_config, load_missing_config};
use crate::tests::fixtures::{TestDome, monitor, window};
use insta::assert_snapshot;

#[test]
fn boot_with_missing_empty_or_broken_layout_file_loads_no_layout() {
    let mut missing = TestDome::new("return {}");
    missing.open(window("w0"));
    let expected = missing.snapshot();
    assert_snapshot!(expected, @"
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
    for layout in [
        "return {}",
        "return { primary = { ['0'] = { layout = 'scrolling' } }",
    ] {
        let mut dome = TestDome::with_layout("return {}", layout);
        dome.open(window("w0"));
        assert_eq!(dome.snapshot(), expected, "{layout}");
    }
}

#[test]
fn reload_broken_config_keeps_last_good_bindings_and_reserved_area() {
    let mut dome = TestDome::new(
        "return {
            keymaps = { main = { ['meta+c'] = function(a) a.execute('kept') end } },
            reserved_area = function() return { top = 2 } end,
        }",
    );
    dome.open(window("w0"));
    assert!(dome.reload("this is not lua {{{").is_none());
    dome.press("main", "meta+c");
    dome.add_monitor(monitor("DELL", [100, 0, 100, 20]));
    assert_snapshot!(dome.snapshot(), @r#"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    effects: execute "kept"
    primary 0,2 100x18 border 1
      w0 0,2 100x18 highlighted spawn=right
    DELL 100,2 100x18 border 1
    ****************************************************************************************************
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
    ****************************************************************************************************
    "#);
}

fn assert_bundled(loaded: &LoadedConfig, bundled: &LoadedConfig, case: &str) {
    assert_eq!(loaded.config.tiling, bundled.config.tiling, "{case}");
    assert_eq!(
        loaded.config.appearance.theme, bundled.config.appearance.theme,
        "{case}"
    );
    assert_eq!(
        loaded.config.appearance.font, bundled.config.appearance.font,
        "{case}"
    );
    assert_eq!(
        loaded.config.log_level.as_str(),
        bundled.config.log_level.as_str(),
        "{case}"
    );
    assert_eq!(
        loaded.config.start_at_login, bundled.config.start_at_login,
        "{case}"
    );
    assert_eq!(loaded.config.env, bundled.config.env, "{case}");
    assert_eq!(loaded.keymaps, bundled.keymaps, "{case}");
}

#[test]
fn load_missing_broken_or_non_table_config_takes_bundled_defaults() {
    let bundled = load_config("return dome.defaults()");
    assert_bundled(&load_missing_config(), &bundled, "missing file");
    for source in [
        "return { border_size = 7, start_at_login = true",
        "local config = { border_size = 7, start_at_login = true } return 42",
        "local config = { border_size = 7, start_at_login = true }",
        "dome.log_info('valid prefix', setmetatable({}, {
            __tostring = function() error('cannot stringify') end,
        })) return { border_size = 7 }",
        "dome.log_warn(setmetatable({}, {
            __tostring = function() error({}) end,
        })) return { border_size = 7 }",
    ] {
        assert_bundled(&load_config(source), &bundled, source);
    }
}
