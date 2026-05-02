use crate::action::MonitorTarget;
use crate::core::WorkspaceInfo;
use crate::core::node::{Dimension, WindowRestrictions};
use crate::core::tests::setup;

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
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].window_count, 3);
    assert!(ws[0].is_focused);
    assert!(ws[0].is_visible);
}

#[test]
fn multiple_workspaces() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.focus_workspace("web");
    hub.insert_tiling();
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
    let mut hub = setup();
    hub.insert_tiling();
    hub.insert_float(Dimension {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 100.0,
    });
    let third = hub.insert_tiling();
    hub.set_fullscreen(third, WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    // 1 tiling + 1 float + 1 fullscreen = 3, no double-counting
    assert_eq!(ws[0].window_count, 3);
}

#[test]
fn focused_vs_visible_multi_monitor() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.add_monitor(
        "secondary".to_string(),
        Dimension {
            x: 200.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
        1.0,
    );
    hub.focus_monitor(&MonitorTarget::Name("secondary".into()));
    hub.insert_tiling();
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert!(ws0.is_visible);
    assert!(!ws0.is_focused);
    assert_eq!(ws0.window_count, 1);

    let sec = ws.iter().find(|w| w.name == "secondary").unwrap();
    assert!(sec.is_visible);
    assert!(sec.is_focused);
    assert_eq!(sec.window_count, 1);
}

#[test]
fn pruned_workspace_not_in_results() {
    let mut hub = setup();
    hub.insert_tiling();
    hub.focus_workspace("empty");
    hub.focus_workspace("0");
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].name, "0");
}

#[test]
fn workspace_info_json_shape() {
    let info = WorkspaceInfo {
        name: "main".to_string(),
        is_focused: true,
        is_visible: false,
        window_count: 3,
    };
    let v: serde_json::Value = serde_json::to_value(&info).unwrap();
    assert_eq!(v["name"], "main");
    assert_eq!(v["is_focused"], true);
    assert_eq!(v["is_visible"], false);
    assert_eq!(v["window_count"], 3);
    let back: WorkspaceInfo = serde_json::from_value(v).unwrap();
    assert_eq!(back, info);
}

#[test]
fn workspace_with_only_floats() {
    let mut hub = setup();
    hub.insert_float(Dimension {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 100.0,
    });
    hub.insert_float(Dimension {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 100.0,
    });
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].window_count, 2);
}

#[test]
fn workspace_with_only_fullscreen() {
    let mut hub = setup();
    let first = hub.insert_tiling();
    let second = hub.insert_tiling();
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
        Dimension {
            x: 200.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        },
        1.0,
    );
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert!(ws0.is_focused);
    assert!(ws0.is_visible);
    assert_eq!(ws0.window_count, 0);

    let sec = ws.iter().find(|w| w.name == "secondary").unwrap();
    assert!(!sec.is_focused);
    assert!(sec.is_visible);
    assert_eq!(sec.window_count, 0);
}
