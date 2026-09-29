use crate::action::{MonitorDetails, MonitorFrame, WorkspaceInfo, WorkspaceState};
use crate::core::MonitorSelector;
use crate::core::ReportedMonitor;
use crate::core::TilingConfig;
use crate::core::node::{PixelRect, WindowRestrictions};
use crate::core::tests::{
    TilingConfigBuilder, default_rect, reported_monitor, setup, setup_with_tiling, titled,
    titled_matcher, work_area_at,
};

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn tiling_floating(titles: &[&str]) -> TilingConfig {
    TilingConfigBuilder::new()
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
}

#[test]
fn single_workspace_with_windows() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
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
    assert!(!ws0.is_focused);
    assert!(!ws0.is_visible);

    let web = ws.iter().find(|w| w.name == "web").unwrap();
    assert!(web.is_focused);
    assert!(web.is_visible);
}

#[test]
fn focused_vs_visible_multi_monitor() {
    let mut hub = setup();
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.add_monitor(reported_monitor(
        "secondary".to_string(),
        PixelRect::new(200, 0, 100, 30),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("secondary".into()));
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    // Both monitors default to a workspace named "0", so disambiguate by focus.
    let unfocused = ws.iter().find(|w| !w.is_focused).unwrap();
    assert!(unfocused.is_visible);

    let focused = ws.iter().find(|w| w.is_focused).unwrap();
    assert!(focused.is_visible);
}

#[test]
fn empty_non_active_workspace_is_skipped() {
    let mut hub = setup();
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);
    hub.focus_workspace("empty", None);
    hub.focus_workspace("0", None);
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 1);
    assert_eq!(ws[0].name, "0");
}

#[test]
fn workspace_info_json_shape() {
    let info = WorkspaceInfo {
        name: "main".to_string(),
        monitor: "DELL #1".to_string(),
        state: WorkspaceState::Attached,
        is_focused: true,
        is_visible: false,
    };
    let json: serde_json::Value = serde_json::to_value(&info).unwrap();
    assert_eq!(json["name"], "main");
    assert_eq!(json["monitor"], "DELL #1");
    assert_eq!(json["state"], "attached");
    assert_eq!(json["is_focused"], true);
    assert_eq!(json["is_visible"], false);
    let back: WorkspaceInfo = serde_json::from_value(json).unwrap();
    assert_eq!(back, info);
}

#[test]
fn monitor_details_json_shape() {
    let details = MonitorDetails {
        device_name: "DELL SE2416H".to_string(),
        unique_name: "DELL SE2416H #1".to_string(),
        cg_display_id: Some(7),
        gdi_device: Some("\\\\.\\DISPLAY1".to_string()),
        work_area: MonitorFrame {
            x: 100,
            y: -50,
            width: 1920,
            height: 1080,
        },
    };
    let json: serde_json::Value = serde_json::to_value(&details).unwrap();
    assert_eq!(json["device_name"], "DELL SE2416H");
    assert_eq!(json["unique_name"], "DELL SE2416H #1");
    assert_eq!(json["cg_display_id"], 7);
    assert_eq!(json["gdi_device"], "\\\\.\\DISPLAY1");
    assert_eq!(json["work_area"]["x"], 100);
    assert_eq!(json["work_area"]["y"], -50);
    assert_eq!(json["work_area"]["width"], 1920);
    assert_eq!(json["work_area"]["height"], 1080);
    let back: MonitorDetails = serde_json::from_value(json).unwrap();
    assert_eq!(back, details);

    // The bars test the Option fields against null, so None must serialize as an
    // explicit null rather than a dropped key.
    let missing = MonitorDetails {
        cg_display_id: None,
        gdi_device: None,
        ..details
    };
    let missing_json = serde_json::to_value(&missing).unwrap();
    assert!(missing_json["cg_display_id"].is_null());
    assert!(missing_json["gdi_device"].is_null());
}

#[test]
fn hidden_workspace_with_only_floats_is_listed() {
    let mut hub = setup_with_tiling(tiling_floating(&["w12", "w13"]));
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
    hub.focus_workspace("other", None);
    let ws = hub.query_workspaces();
    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert!(!ws0.is_visible);
}

#[test]
fn hidden_workspace_with_only_fullscreen_is_listed() {
    let mut hub = setup();
    let first = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let second = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_fullscreen(first, WindowRestrictions::None);
    hub.set_fullscreen(second, WindowRestrictions::None);
    hub.focus_workspace("other", None);
    let ws = hub.query_workspaces();
    let ws0 = ws.iter().find(|w| w.name == "0").unwrap();
    assert!(!ws0.is_visible);
}

#[test]
fn multi_monitor_no_windows() {
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "secondary".to_string(),
        PixelRect::new(200, 0, 100, 30),
        1.0,
    ));
    let ws = hub.query_workspaces();
    assert_eq!(ws.len(), 2);

    // Both monitors default to a workspace named "0", so disambiguate by focus.
    let focused = ws.iter().find(|w| w.is_focused).unwrap();
    assert!(focused.is_visible);

    let unfocused = ws.iter().find(|w| !w.is_focused).unwrap();
    assert!(unfocused.is_visible);
}

#[test]
fn monitors_report_stamped_cg_display_id() {
    let mut hub = setup();
    let primary = hub.primary_monitor();
    hub.add_monitor(ReportedMonitor {
        device_name: "external".to_string(),
        work_area: work_area_at(150, 0),
        scale: 1.0,
        cg_display_id: Some(7),
        gdi_device: None,
    });
    // Stamp the primary in place, with its geometry unchanged.
    hub.update_monitor(
        primary,
        ReportedMonitor {
            device_name: "primary".to_string(),
            work_area: PixelRect::new(0, 0, 150, 30),
            scale: 1.0,
            cg_display_id: Some(1),
            gdi_device: None,
        },
        None,
    );

    let monitors = hub.query_monitors();
    assert_eq!(monitors.len(), 2);
    let p = monitors
        .iter()
        .find(|m| m.unique_name == "primary")
        .unwrap();
    assert_eq!(p.cg_display_id, Some(1));
    assert_eq!(p.gdi_device, None);
    let e = monitors
        .iter()
        .find(|m| m.unique_name == "external")
        .unwrap();
    assert_eq!(e.cg_display_id, Some(7));
}

#[test]
fn restamping_gdi_device_replaces_the_previous_value() {
    let mut hub = setup();
    let primary = hub.primary_monitor();
    hub.update_monitor(
        primary,
        ReportedMonitor {
            device_name: "primary".to_string(),
            work_area: PixelRect::new(0, 0, 150, 30),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: Some("\\\\.\\DISPLAY1".to_string()),
        },
        None,
    );
    hub.update_monitor(
        primary,
        ReportedMonitor {
            device_name: "primary".to_string(),
            work_area: PixelRect::new(0, 0, 150, 30),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: Some("\\\\.\\DISPLAY2".to_string()),
        },
        None,
    );

    // Windows can move a device string between displays, so the newest wins.
    let monitors = hub.query_monitors();
    assert_eq!(monitors[0].gdi_device.as_deref(), Some("\\\\.\\DISPLAY2"));
}

#[test]
fn recomputing_monitor_names_preserves_identifiers() {
    let mut hub = setup();
    hub.add_monitor(ReportedMonitor {
        device_name: "twin".to_string(),
        work_area: work_area_at(150, 0),
        scale: 1.0,
        cg_display_id: Some(11),
        gdi_device: None,
    });
    hub.add_monitor(ReportedMonitor {
        device_name: "twin".to_string(),
        work_area: work_area_at(300, 0),
        scale: 1.0,
        cg_display_id: Some(22),
        gdi_device: None,
    });

    // A third twin lands between them and reranks every suffix. The stamps must
    // not travel with the names.
    hub.add_monitor(ReportedMonitor {
        device_name: "twin".to_string(),
        work_area: work_area_at(225, 0),
        scale: 1.0,
        cg_display_id: Some(33),
        gdi_device: None,
    });

    let monitors = hub.query_monitors();
    let stamped = |name: &str| {
        monitors
            .iter()
            .find(|m| m.unique_name == name)
            .unwrap()
            .cg_display_id
    };
    assert_eq!(stamped("twin #1"), Some(11));
    assert_eq!(stamped("twin #2"), Some(33));
    assert_eq!(stamped("twin #3"), Some(22));
}

#[test]
fn monitors_are_ordered_by_screen_position() {
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "right".to_string(),
        work_area_at(300, 0),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "middle".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    let monitors = hub.query_monitors();
    let names: Vec<&str> = monitors.iter().map(|m| m.unique_name.as_str()).collect();
    assert_eq!(names, ["primary", "middle", "right"]);
}
