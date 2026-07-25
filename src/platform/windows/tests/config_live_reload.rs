use super::*;

#[test]
fn border_size_changed_resize_managed_windows() {
    let mut env = TestEnv::new();
    let w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let prev_d1 = env.dim(w1);
    let prev_d2 = env.dim(w2);
    let prev_d3 = env.dim(w3);
    let mut new_config = env.config.clone();
    new_config.border_size = env.config.border_size + 2.0;
    env.dome.config_changed(new_config);
    env.dome.apply_layout();

    let d1 = env.dim(w1);
    let d2 = env.dim(w2);
    let d3 = env.dim(w3);
    assert_eq!(d1.width, prev_d1.width - Length::new(4.0));
    assert_eq!(d1.height, prev_d1.height - Length::new(4.0));
    assert_eq!(d2.width, prev_d2.width - Length::new(4.0));
    assert_eq!(d2.height, prev_d2.height - Length::new(4.0));
    assert_eq!(d3.width, prev_d3.width - Length::new(4.0));
    assert_eq!(d3.height, prev_d3.height - Length::new(4.0));
}

#[test]
fn config_reload_dispatches_apply_theme_on_flavor_change() {
    let mut env = TestEnv::new(); // default Mocha
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    let _w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    // Sanity: overlays start at the default Mocha flavor.
    // w2 is the floated window. w1 is tiling (no float overlay). Only w2
    // has a float overlay entry.
    let tiling = env.tiling_overlays();
    assert_eq!(tiling[0].flavor, crate::theme::Flavor::Mocha);
    for f in &env.float_overlays() {
        assert_eq!(f.flavor, crate::theme::Flavor::Mocha);
    }

    let mut new_config = env.config.clone();
    new_config.theme = crate::theme::Flavor::Latte;
    env.dome.config_changed(new_config);

    // After a flavor change, overlays must end up holding Latte.
    let tiling = env.tiling_overlays();
    assert_eq!(tiling[0].flavor, crate::theme::Flavor::Latte);
    for f in &env.float_overlays() {
        assert_eq!(f.flavor, crate::theme::Flavor::Latte);
    }
}

#[test]
fn config_reload_dispatches_apply_font_on_font_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let _w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let new_font = crate::font::FontConfig {
        text_size: 18.0,
        family: None,
    };
    // Sanity: overlays start at the default font (different from `new_font`).
    let tiling = env.tiling_overlays();
    assert_ne!(tiling[0].font, new_font);
    for f in &env.float_overlays() {
        assert_ne!(f.font, new_font);
    }

    let mut new_config = env.config.clone();
    new_config.font = new_font.clone();
    env.dome.config_changed(new_config);

    // After a font change, both overlays must hold the new font.
    let tiling = env.tiling_overlays();
    assert_eq!(tiling[0].font, new_font);
    for f in &env.float_overlays() {
        assert_eq!(f.font, new_font);
    }
}
