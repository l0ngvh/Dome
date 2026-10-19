use super::{
    PARTITION_CONFIG,
    fixture::{TestDome, monitor},
};
use crate::platform::windows::window_server::WindowNotification;
use dome_core::{ContainerPlacement, MonitorId, PixelRect};
use dome_ipc::action::Action;

#[test]
fn focus_workspace_restores_application_focus_and_empty_monitor_focus() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace home");
    let home = dome.open("Home");
    assert_eq!(dome.focused(), Some(home));
    assert!(dome.in_taskbar(home));
    dome.run("focus workspace other");
    let other = dome.open("Other");
    assert!(dome.in_taskbar(other));
    assert_eq!(dome.focused(), Some(other));
    assert_eq!(dome.frame(home), PixelRect::new(-32000, -32000, 98, 18));
    assert!(!dome.in_taskbar(home));

    dome.run("focus workspace home");
    assert_eq!(dome.focused(), Some(home));
    assert_eq!(dome.frame(home), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(other), PixelRect::new(-32000, -32000, 98, 18));
    assert!(dome.in_taskbar(home));
    assert!(!dome.in_taskbar(other));

    dome.run("focus workspace empty");
    let primary = dome
        .overlays()
        .requested
        .as_ref()
        .unwrap()
        .focus_tiling
        .expect("empty monitor focus");
    dome.display_changed(vec![monitor(1), monitor(2)], None);
    let right = dome
        .overlays()
        .requested
        .as_ref()
        .unwrap()
        .tiling
        .iter()
        .find(|(_, placement)| placement.monitor.x().value() == 100)
        .map(|(id, _)| *id)
        .expect("right monitor");
    assert_ne!(right, primary);
    dome.run("focus monitor right");
    assert_eq!(
        dome.overlays().requested.as_ref().unwrap().focus_tiling,
        Some(right)
    );

    let mut left_display = monitor(1);
    left_display.name = "Left".into();
    let mut right_display = monitor(2);
    right_display.name = "Right".into();
    dome.display_changed(vec![left_display.clone(), right_display.clone()], None);
    dome.run("focus workspace remote");
    let remote = dome.open("Remote");
    dome.run("focus monitor left");
    dome.run("focus workspace home");
    let remote_frame = dome.frame(remote);
    assert_eq!(remote_frame, PixelRect::new(101, 1, 98, 18));
    left_display.scale = 2.0;
    dome.display_changed(vec![left_display.clone(), right_display.clone()], None);
    assert_eq!(dome.frame(home), PixelRect::new(2, 2, 96, 16));
    assert_eq!(dome.frame(remote), remote_frame);

    dome.display_changed(vec![left_display.clone()], None);
    assert_eq!(dome.frame(remote).x().value(), -32000);
    assert!(!dome.in_taskbar(remote));
    let workspaces = dome.dome.query_workspaces();
    let parked = workspaces
        .iter()
        .find(|workspace| workspace.name == "remote")
        .unwrap();
    assert_eq!(parked.monitor, "Right");
    assert_eq!(parked.state, dome_ipc::action::WorkspaceState::Parked);
    assert!(!parked.is_visible);
    dome.run("focus workspace remote --monitor Right");
    assert_eq!(dome.frame(remote), PixelRect::new(2, 2, 96, 16));
    assert_eq!(dome.frame(home).x().value(), -32000);
    assert!(dome.in_taskbar(remote));

    left_display.scale = 1.0;
    dome.display_changed(vec![left_display.clone(), right_display.clone()], None);
    dome.run("focus workspace home --monitor Left");
    assert_eq!(dome.frame(home), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(remote), remote_frame);
    left_display.is_primary = false;
    right_display.is_primary = true;
    dome.display_changed(vec![left_display, right_display], None);
    assert_eq!(dome.frame(home), PixelRect::new(101, 1, 98, 18));
    assert_eq!(dome.frame(remote).x().value(), -32000);
    let workspaces = dome.dome.query_workspaces();
    let carried = workspaces
        .iter()
        .find(|workspace| workspace.name == "home")
        .unwrap();
    assert_eq!(carried.monitor, "Right");
    assert!(carried.is_visible);
    assert!(carried.is_focused);
    let displaced = workspaces
        .iter()
        .find(|workspace| workspace.name == "remote")
        .unwrap();
    assert_eq!(displaced.state, dome_ipc::action::WorkspaceState::Parked);
    assert!(!displaced.is_visible);
}

#[test]
fn focus_minimized_window_reattaches_without_movement_notification() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open("Second");
    dome.change(second, |window| window.state.minimized = true);
    dome.deliver_notification(WindowNotification::WindowMinimized(second));
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
    assert!(dome.minimized(second));
    dome.run("focus workspace restored");

    dome.dome
        .handle_action(&Action::UnminimizeWindow { id: second })
        .expect("restore minimized window");
    assert!(!dome.minimized(second));
    dome.focus(second);
    assert_eq!(dome.frame(second), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(first), PixelRect::new(-32000, -32000, 98, 18));
    let state = dome.overlays();
    let shown = &state
        .requested
        .as_ref()
        .unwrap()
        .tiling
        .values()
        .next()
        .unwrap()
        .windows;
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].placement.id, second);
    assert_eq!(shown[0].placement.border_box, PixelRect::new(0, 0, 100, 20));
    assert!(shown[0].placement.is_highlighted);
}

#[test]
fn click_tab_restores_selected_window_and_tracks_title_and_membership_changes() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open("Second");
    dome.run("toggle layout");
    let container = tabbed_container(&dome, &["First", "Second"], None);
    assert_eq!(container.active_tab_index, 1);
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(first).x().value(), -32000);
    assert!(!dome.in_taskbar(first));
    assert!(dome.in_taskbar(second));

    dome.dome.tab_clicked(container.id, 0);

    assert_eq!(dome.focused(), Some(first));
    assert_eq!(dome.frame(first).x().value(), 1);
    assert_eq!(dome.frame(second).x().value(), -32000);
    assert!(dome.in_taskbar(first));
    assert!(!dome.in_taskbar(second));
    assert_eq!(
        tabbed_container(&dome, &["First", "Second"], None).active_tab_index,
        0
    );
    dome.change(second, |window| {
        window.metadata.title = Some("Renamed".into())
    });
    dome.deliver_notification(WindowNotification::TitleChanged {
        window_id: second,
        title: Some("Renamed".into()),
    });
    let renamed = tabbed_container(&dome, &["First", "Renamed"], None);
    assert_eq!(renamed.id, container.id);
    assert_eq!(renamed.active_tab_index, 0);
    assert_eq!(dome.focused(), Some(first));

    dome.remove(first);

    assert!(!dome.in_taskbar(first));
    assert!(dome.in_taskbar(second));
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(dome.frame(second), PixelRect::new(1, 1, 98, 18));
    assert!(dome.overlays().requested.as_ref().unwrap().tabs.is_empty());
}

fn tabbed_container(
    dome: &TestDome,
    titles: &[&str],
    monitor: Option<MonitorId>,
) -> ContainerPlacement {
    let state = dome.overlays();
    let requested = state.requested.as_ref().unwrap();
    let mut matching = requested.tabs.values().filter(|(id, placement)| {
        monitor.is_none_or(|monitor| monitor == *id) && placement.titles == titles
    });
    let (_, placement) = matching
        .next()
        .expect("tabbed container with ordered titles");
    assert!(matching.next().is_none(), "tabbed container must be unique");
    placement.clone()
}
