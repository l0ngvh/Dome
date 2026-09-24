use insta::assert_snapshot;

use super::TilingConfigBuilder;
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{PixelRect, WindowHost, WindowId, WindowRestrictions};
use crate::core::strategy::TilingAction;
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, reported_monitor, setup,
    setup_with_tiling, snapshot, snapshot_text, titled, titled_matcher, work_area_at,
};
use crate::core::{MonitorId, MonitorSelector, TilingConfig, WindowMatcher};

fn tiling_floating(titles: &[&str]) -> TilingConfig {
    TilingConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

fn insert_sticky(hub: &mut Hub, title: &str, rect: PixelRect) -> WindowId {
    let id = hub
        .insert_window(titled(title), rect, WindowRestrictions::None)
        .unwrap();
    assert!(
        hub.access.windows.get(id).is_float(),
        "{title} needs a float matcher"
    );
    hub.handle_tiling_action(TilingAction::ToggleSticky);
    assert!(hub.access.windows.get(id).is_sticky());
    id
}

fn sticky_placement_ids(hub: &Hub) -> Vec<WindowId> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { float_windows, .. } = &placements.monitors[0].layout else {
        panic!("the first monitor shows a fullscreen window");
    };
    float_windows
        .iter()
        .filter(|p| p.sticky)
        .map(|p| p.id)
        .collect()
}

#[test]
fn toggle_sticky_detaches_a_tiling_window() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.handle_tiling_action(TilingAction::ToggleSticky);

    assert!(hub.access.windows.get(w1).is_sticky());
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, float, sticky, highlighted)
      )

    +--------------------------------------------------------------------------***************************************************************************
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                         W*                                    F1                                   *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    |                                                                          *                                                                         *
    +--------------------------------------------------------------------------***************************************************************************
    ");
}

#[test]
fn sticky_window_shows_on_every_workspace_of_its_monitor() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));
    assert_eq!(sticky_placement_ids(&hub), vec![pin]);

    hub.focus_workspace("1", None);

    assert_eq!(sticky_placement_ids(&hub), vec![pin]);
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, sticky, highlighted)
      )
    ");
}

#[test]
fn sticky_window_keeps_focus_across_a_workspace_change() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.focus_workspace("1", None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.focus_workspace("0", None);
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.focus_workspace("1", None);

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(pin));
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(2), x=10.00, y=5.00, w=40.00, h=10.00, float, sticky, highlighted)
      )
    ");
}

#[test]
fn a_sticky_window_does_not_keep_a_hidden_workspace_listed() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.focus_workspace("1", None);
    let workspaces = hub.query_workspaces();

    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name, "1");
}

#[test]
fn export_omits_a_sticky_window() {
    let mut hub = setup_with_tiling(tiling_floating(&["float-keep", "pin"]));
    hub.insert_window(
        titled("tiling-keep"),
        default_rect(),
        WindowRestrictions::None,
    );
    hub.insert_window(
        titled("float-keep"),
        PixelRect::new(60, 5, 40, 10),
        WindowRestrictions::None,
    );
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    let export = hub.export_workspace(hub.current_workspace());

    assert_eq!(export.float.len(), 1);
    assert!(export.fullscreen.is_empty());
    let text = format!("{export:?}");
    assert!(text.contains("tiling-keep") && text.contains("float-keep"));
    assert!(
        !text.contains("pin"),
        "export names the sticky window: {text}"
    );
}

#[test]
fn focusing_a_tiling_window_clears_sticky_focus() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.set_focus(w0);

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w0));
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, sticky)
      )
    ");
}

#[test]
fn unminimized_float_takes_focus_from_a_sticky_window() {
    let mut hub = setup_with_tiling(tiling_floating(&["float", "pin"]));
    let float = hub
        .insert_window(
            titled("float"),
            PixelRect::new(60, 5, 40, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));
    hub.minimize_window(float);
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(pin));

    hub.unminimize_window(float);

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(float));
}

#[test]
fn tiling_window_is_not_highlighted_while_a_sticky_window_has_focus() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, sticky, highlighted)
      )
    ");
}

#[test]
fn fullscreen_window_hides_and_outranks_a_sticky_window() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.set_fullscreen(w0, WindowRestrictions::None);

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w0));
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(0))
      )
    ");
}

fn add_external(hub: &mut Hub) -> MonitorId {
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ))
}

fn sticky_rect(hub: &Hub, id: WindowId) -> (WindowHost, PixelRect) {
    let window = hub.access.windows.get(id);
    let crate::core::node::DisplayMode::Float { border_box, .. } = window.mode else {
        panic!("{id} is not Float");
    };
    (window.host, border_box)
}

#[test]
fn minimized_sticky_window_returns_sticky_on_the_current_monitor() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let external = add_external(&mut hub);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.minimize_window(pin);
    hub.focus_monitor(&MonitorSelector::Name("external".to_string()));
    hub.unminimize_window(pin);

    assert_eq!(
        sticky_rect(&hub, pin),
        (
            WindowHost::Monitor(external),
            PixelRect::new(150, 5, 40, 10)
        )
    );
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(pin));
}

#[test]
fn removed_monitor_sends_sticky_windows_to_the_primary() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let external = add_external(&mut hub);
    hub.focus_monitor(&MonitorSelector::Name("external".to_string()));
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(160, 5, 40, 10));
    let primary = hub.primary_monitor();

    hub.remove_monitor(external);

    let on_primary = (WindowHost::Monitor(primary), PixelRect::new(25, 5, 40, 10));
    assert_eq!(sticky_rect(&hub, pin), on_primary);
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(pin));

    add_external(&mut hub);

    assert_eq!(sticky_rect(&hub, pin), on_primary);
}

#[test]
fn primary_change_maps_sticky_windows_against_the_new_primary_geometry() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let dell = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("DELL".to_string()));
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(160, 5, 40, 10));
    let primary = hub.primary_monitor();

    hub.update_monitor(
        primary,
        reported_monitor("DELL".to_string(), work_area_at(150, 0), 1.0),
        Some(dell),
    );

    assert_eq!(
        sticky_rect(&hub, pin),
        (WindowHost::Monitor(primary), PixelRect::new(160, 5, 40, 10))
    );
}

#[test]
fn fullscreen_sticky_window_fills_the_active_workspace_only() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.toggle_fullscreen();
    assert!(hub.access.windows.get(pin).is_fullscreen());
    hub.focus_workspace("1", None);

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");

    hub.focus_workspace("0", None);
    hub.toggle_fullscreen();

    assert_eq!(
        sticky_rect(&hub, pin),
        (
            WindowHost::Monitor(hub.primary_monitor()),
            PixelRect::new(10, 5, 40, 10)
        )
    );
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(pin));
}

#[test]
fn toggle_sticky_on_a_fullscreen_window_does_nothing() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_fullscreen(w0, WindowRestrictions::None);
    let before = snapshot_text(&hub);

    hub.handle_tiling_action(TilingAction::ToggleSticky);

    assert!(hub.access.windows.get(w0).is_fullscreen());
    assert_eq!(snapshot_text(&hub), before);
}

#[test]
fn toggle_sticky_on_a_sticky_window_makes_a_float() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.handle_tiling_action(TilingAction::ToggleSticky);

    assert_eq!(
        sticky_rect(&hub, pin),
        (
            WindowHost::Workspace(hub.current_workspace()),
            PixelRect::new(10, 5, 40, 10)
        )
    );
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, highlighted)
      )
    ");
}

#[test]
fn toggle_float_on_a_sticky_window_makes_it_tiling() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.toggle_float();

    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, pin])
      )
    ");
}

#[test]
fn move_to_workspace_does_nothing_on_a_sticky_window() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));
    let before = (snapshot_text(&hub), sticky_rect(&hub, pin));

    hub.move_focused_to_workspace("1", None);

    assert_eq!((snapshot_text(&hub), sticky_rect(&hub, pin)), before);
}

#[test]
fn move_to_monitor_keeps_the_window_sticky() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let external = add_external(&mut hub);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.move_focused_to_monitor(&MonitorSelector::Name("external".to_string()));

    assert_eq!(
        sticky_rect(&hub, pin),
        (
            WindowHost::Monitor(external),
            PixelRect::new(150, 5, 40, 10)
        )
    );
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(0), x=150.00, y=5.00, w=40.00, h=10.00, float, sticky)
      )
    "#);
}

#[test]
fn drag_to_another_monitor_keeps_the_window_sticky() {
    let mut hub = setup_with_tiling(tiling_floating(&["pin"]));
    let external = add_external(&mut hub);
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));
    let border = hub.access.border(external);
    let dragged = PixelRect::new(160, 5, 40, 10);

    hub.update_float_rect(pin, dragged.inset_by(border), external);

    assert_eq!(
        sticky_rect(&hub, pin),
        (WindowHost::Monitor(external), dragged)
    );
}

#[test]
fn deleting_the_focused_sticky_window_returns_focus_to_the_float() {
    let mut hub = setup_with_tiling(tiling_floating(&["float", "pin"]));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    let float = hub
        .insert_window(
            titled("float"),
            PixelRect::new(60, 5, 40, 10),
            WindowRestrictions::None,
        )
        .unwrap();
    let pin = insert_sticky(&mut hub, "pin", PixelRect::new(10, 5, 40, 10));

    hub.delete_window(pin);

    assert_eq!(hub.focused_window(hub.current_workspace()), Some(float));
    assert_snapshot!(snapshot_text(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=60.00, y=5.00, w=40.00, h=10.00, float, highlighted)
      )
    ");
}

#[test]
fn global_sticky_matcher_routes_a_new_window() {
    let title = |pattern: &str| WindowMatcher {
        title: Some(pattern.to_string()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_fullscreen(vec![title("pin-fs")])
                .with_sticky(vec![title("/^pin/")])
                .with_float(vec![title("/^pin/")])
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_float(vec![title("pin-ws")])
                .build(),
        ])
        .build();
    let insert = |hub: &mut Hub, name: &str| {
        hub.insert_window(
            titled(name),
            PixelRect::new(10, 5, 40, 10),
            WindowRestrictions::None,
        )
        .unwrap()
    };

    let pin = insert(&mut hub, "pin");
    let fullscreen = insert(&mut hub, "pin-fs");
    let workspace_float = insert(&mut hub, "pin-ws");

    assert!(
        hub.access.windows.get(pin).is_sticky(),
        "sticky outranks a global float"
    );
    assert!(
        hub.access.windows.get(fullscreen).is_fullscreen(),
        "a global fullscreen outranks sticky"
    );
    let window = hub.access.windows.get(workspace_float);
    assert!(
        window.is_float() && !window.is_sticky(),
        "a workspace float outranks a global sticky"
    );
}
