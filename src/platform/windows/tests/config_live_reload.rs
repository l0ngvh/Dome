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
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");
    let w3 = env.open(3, "App3", "app3.exe", SPAWN_DIM);

    // Open the picker so self.picker is Some and config_changed dispatches
    // apply_theme on it.
    env.minimize_window(w3);
    env.run_actions("toggle minimized");

    let w2_id = env.dome.registry_get_id(w2).unwrap();

    // Sanity: both overlays and picker start at the default Mocha flavor.
    // w2 is the floated window; w1 is tiling (no float overlay). Only w2
    // has a float overlay entry.
    let snap = env.snapshot();
    assert_eq!(snap.tiling[0].flavor, crate::theme::Flavor::Mocha);
    let w2_float = snap.floats.iter().find(|f| f.window_id == w2_id).unwrap();
    assert_eq!(w2_float.flavor, crate::theme::Flavor::Mocha);
    assert_eq!(env.picker_flavor(), crate::theme::Flavor::Mocha);

    let mut new_config = env.config.clone();
    new_config.theme = crate::theme::Flavor::Latte;
    env.dome.config_changed(new_config);

    // After a flavor change, both overlays and picker must end up holding Latte.
    let snap = env.snapshot();
    assert_eq!(snap.tiling[0].flavor, crate::theme::Flavor::Latte);
    let w2_float = snap.floats.iter().find(|f| f.window_id == w2_id).unwrap();
    assert_eq!(w2_float.flavor, crate::theme::Flavor::Latte);
    assert_eq!(env.picker_flavor(), crate::theme::Flavor::Latte);
}

#[test]
fn config_reload_dispatches_apply_font_on_font_change() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);
    env.run_actions("toggle float");

    let w2_id = env.dome.registry_get_id(w2).unwrap();

    let new_font = crate::font::FontConfig {
        text_size: 18.0,
        subtext_size: 12.0,
        family: None,
    };
    // Sanity: overlays start at the default font (different from `new_font`).
    let snap = env.snapshot();
    assert_ne!(snap.tiling[0].font, new_font);
    assert_ne!(
        snap.floats
            .iter()
            .find(|f| f.window_id == w2_id)
            .unwrap()
            .font,
        new_font
    );

    let mut new_config = env.config.clone();
    new_config.font = new_font.clone();
    env.dome.config_changed(new_config);

    // After a font change, both overlays must hold the new font.
    let snap = env.snapshot();
    assert_eq!(snap.tiling[0].font, new_font);
    assert_eq!(
        snap.floats
            .iter()
            .find(|f| f.window_id == w2_id)
            .unwrap()
            .font,
        new_font
    );
}

#[test]
fn config_reload_dispatches_apply_font_on_picker() {
    let mut env = TestEnv::new();
    let _w1 = env.open(1, "App1", "app1.exe", SPAWN_DIM);
    let w2 = env.open(2, "App2", "app2.exe", SPAWN_DIM);

    // Open the picker so config_changed dispatches set_config on it.
    env.minimize_window(w2);
    env.run_actions("toggle minimized");

    let mut new_config = env.config.clone();
    new_config.font.text_size += 2.0;
    env.dome.config_changed(new_config.clone());

    assert_eq!(env.picker_font(), new_config.font);
}
