use crate::action::MonitorTarget;
use crate::action::{WorkspaceInfo, WorkspaceState};
use crate::core::GlobalLayoutConfig;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{
    LayoutConfigBuilder, default_rect, setup, setup_with_layout, titled, titled_matcher,
};

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn empty_hub() {
    let hub = setup();
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].name, "0");
    assert!(ws[0].is_focused);
    assert!(ws[0].is_visible);
    assert_eq!(ws[0].window_count, 0);
}

#[test]
fn single_workspace_with_windows() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].window_count, 3);
    assert!(ws[0].is_focused);
    assert!(ws[0].is_visible);
}

#[test]
fn multiple_workspaces() {
    let mut hub = setup();
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w4"), default_rect(), WindowRestrictions::None);
    hub.focus_workspace("web", None);
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert_eq!(ws0.window_count, 2);
    assert!(!ws0.is_focused);
    assert!(!ws0.is_visible);

    let web = ws.iter().find(|w| w.name == "web").unwrap();
    assert_eq!(web.window_count, 1);
    assert!(web.is_focused);
    assert!(web.is_visible);
}

#[test]
fn workspace_with_floats_and_fullscreen() {
    let mut hub = setup_with_layout(layout_floating(&["w7"]));
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);
    hub.insert_window(
        titled("w7"),
        PixelRect::new(0, 0, 200, 100),
        WindowRestrictions::None,
    )
    .unwrap();
    let third = hub
        .insert_window(titled("w8"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_fullscreen(third, WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    // 1 tiling + 1 float + 1 fullscreen = 3, no double-counting
    assert_eq!(ws[0].window_count, 3);
}

#[test]
fn focused_vs_visible_multi_monitor() {
    let mut hub = setup();
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.add_monitor(
        "secondary".to_string(),
        PixelRect::new(200, 0, 100, 30),
        1.0,
    );
    hub.focus_monitor(&MonitorTarget::Name("secondary".into()));
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    // Both monitors default to a workspace named "0", so disambiguate by focus.
    let unfocused = ws.iter().find(|w| !w.is_focused).unwrap();
    assert!(unfocused.is_visible);
    assert_eq!(unfocused.window_count, 1);

    let focused = ws.iter().find(|w| w.is_focused).unwrap();
    assert!(focused.is_visible);
    assert_eq!(focused.window_count, 1);
}

#[test]
fn empty_non_active_workspace_persists() {
    let mut hub = setup();
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.focus_workspace("empty", None);
    hub.focus_workspace("0", None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);
    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert_eq!(ws0.window_count, 1);
    let empty = ws.iter().find(|w| w.name == "empty").unwrap();
    assert_eq!(empty.window_count, 0);
}

#[test]
fn workspace_info_json_shape() {
    let info = WorkspaceInfo {
        name: "main".to_string(),
        monitor: "DELL #1".to_string(),
        state: WorkspaceState::Attached,
        is_focused: true,
        is_visible: false,
        window_count: 3,
    };
    let v: serde_json::Value = serde_json::to_value(&info).unwrap();
    assert_eq!(v["name"], "main");
    assert_eq!(v["monitor"], "DELL #1");
    assert_eq!(v["state"], "Attached");
    assert_eq!(v["is_focused"], true);
    assert_eq!(v["is_visible"], false);
    assert_eq!(v["window_count"], 3);
    let back: WorkspaceInfo = serde_json::from_value(v).unwrap();
    assert_eq!(back, info);
}

#[test]
fn workspace_with_only_floats() {
    let mut hub = setup_with_layout(layout_floating(&["w12", "w13"]));
    hub.insert_window(
        titled("w12"),
        PixelRect::new(0, 0, 200, 100),
        WindowRestrictions::None,
    )
    .unwrap();
    hub.insert_window(
        titled("w13"),
        PixelRect::new(0, 0, 200, 100),
        WindowRestrictions::None,
    )
    .unwrap();
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].window_count, 2);
}

#[test]
fn workspace_with_only_fullscreen() {
    let mut hub = setup();
    let first = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let second = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_fullscreen(first, WindowRestrictions::None);
    hub.set_fullscreen(second, WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    // Both detached from tiling by set_fullscreen, so tiling count is 0
    assert_eq!(ws[0].window_count, 2);
}

#[test]
fn multi_monitor_no_windows() {
    let mut hub = setup();
    hub.add_monitor(
        "secondary".to_string(),
        PixelRect::new(200, 0, 100, 30),
        1.0,
    );
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    // Both monitors default to a workspace named "0", so disambiguate by focus.
    let focused = ws.iter().find(|w| w.is_focused).unwrap();
    assert!(focused.is_visible);
    assert_eq!(focused.window_count, 0);

    let unfocused = ws.iter().find(|w| !w.is_focused).unwrap();
    assert!(unfocused.is_visible);
    assert_eq!(unfocused.window_count, 0);
}
