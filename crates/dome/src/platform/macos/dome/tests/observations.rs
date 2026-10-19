use super::fixture::{PARTITION_CONFIG, TestDome, monitor};
use crate::action::Query;
use crate::config::PreferredLayouts;
use crate::platform::macos::window_server::{
    NativeFullscreenObservation, SystemObservation, WindowNotification,
};
use crate::platform::messages::HubMessage;
use dome_core::{FractionalLength, PixelRect};

#[test]
fn reconcile_system_batch_waits_for_accessibility_and_rejects_own_process() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let external = dome.pending_window("External", |window| window.size_settable = false);
    let own = dome.pending_window("Dome", |window| window.pid = std::process::id() as i32);
    dome.deliver_notification(WindowNotification::AllWindows(SystemObservation {
        apps: vec![
            dome.app_observation(dome.pid(external)),
            dome.app_observation(dome.pid(own)),
        ],
        terminated_pids: vec![],
        hidden_pids: vec![],
    }));
    assert_eq!(dome.frame(external), PixelRect::new(5, 5, 40, 10));
    assert_eq!(dome.frame(own), PixelRect::new(5, 5, 40, 10));
    dome.update_window(external, |window| window.size_settable = true);
    assert_eq!(dome.frame(external), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(own), PixelRect::new(5, 5, 40, 10));
}

#[test]
fn remove_and_readopt_app_parks_then_places_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let displayed = dome.frame(window);
    dome.deliver_notification(WindowNotification::AppRemoved {
        pid: dome.pid(window),
    });
    assert_eq!(dome.frame(window), PixelRect::new(99, 19, 98, 18));
    dome.deliver_notification(WindowNotification::AppWindows(
        dome.app_observation(dome.pid(window)),
    ));
    assert_eq!(dome.frame(window), displayed);
    assert_eq!(dome.focused(), Some(window));
}

#[test]
fn enter_native_fullscreen_space_blocks_placement_until_exit() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open_with("Fullscreen", |window| {
        window.size_settable = false;
        window.native_fullscreen = Some(true);
    });
    let original = dome.frame(window);
    let metadata = dome
        .app_observation(dome.pid(window))
        .listed
        .remove(0)
        .metadata;
    dome.deliver_notification(WindowNotification::NativeFullscreen(
        NativeFullscreenObservation::new(
            window,
            dome.pid(window),
            metadata.clone(),
            Some(true),
            None,
            None,
        ),
    ));
    dome.run("toggle float");
    assert_eq!(dome.frame(window), original);
    for (valid, fullscreen) in [(false, Some(false)), (true, None)] {
        let mut observation = dome.app_observation(dome.pid(window));
        let requested = observation.requested.get_mut(&window).unwrap();
        requested.valid = valid;
        requested.native_fullscreen = fullscreen;
        requested.rect = Some(PixelRect::new(10, 4, 30, 8));
        dome.deliver_notification(WindowNotification::AppWindows(observation));
        dome.run("toggle float");
        assert_eq!(dome.frame(window), original);
        assert!(dome.overlays().floats.is_empty());
    }
    dome.deliver_notification(WindowNotification::NativeFullscreen(
        NativeFullscreenObservation::new(
            window,
            dome.pid(window),
            metadata.clone(),
            None,
            Some((original.to_dimension().x, original.to_dimension().y)),
            Some((
                original.to_dimension().width,
                original.to_dimension().height,
            )),
        ),
    ));
    dome.run("toggle float");
    assert_eq!(dome.frame(window), original);
    let restored = original.to_dimension();
    dome.deliver_notification(WindowNotification::NativeFullscreen(
        NativeFullscreenObservation::new(
            window,
            dome.pid(window),
            metadata,
            Some(false),
            Some((restored.x, restored.y)),
            Some((restored.width, restored.height)),
        ),
    ));
    assert_eq!(dome.frame(window), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.focused(), Some(window));
}

#[test]
fn resize_borderless_then_switch_restore_exit_and_remove_restores_siblings() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    dome.run("focus workspace first");
    let sibling = dome.open("Sibling");
    let fullscreen = dome.open("Fullscreen");
    let tiled = (dome.frame(sibling), dome.frame(fullscreen));
    dome.moved(fullscreen, PixelRect::new(0, 0, 100, 20));
    assert_eq!(dome.frame(fullscreen), PixelRect::new(0, 0, 100, 20));
    assert_eq!(dome.frame(sibling).x().value(), 99);
    dome.run("focus workspace second");
    assert!(dome.minimized(fullscreen));
    dome.run("focus workspace first");
    assert!(!dome.minimized(fullscreen));
    assert_eq!(dome.frame(fullscreen), PixelRect::new(0, 0, 100, 20));
    dome.moved(fullscreen, PixelRect::new(10, 4, 30, 8));
    assert_eq!((dome.frame(sibling), dome.frame(fullscreen)), tiled);
    dome.moved(fullscreen, PixelRect::new(0, 0, 100, 20));
    assert_eq!(dome.frame(sibling).x().value(), 99);
    dome.remove(fullscreen);
    assert_eq!(dome.frame(sibling), PixelRect::new(1, 1, 98, 18));
    let admitted = dome.open_with("Already fullscreen", |window| {
        window.rect = Some(PixelRect::new(0, 0, 100, 20));
    });
    assert_eq!(dome.frame(admitted), PixelRect::new(0, 0, 100, 20));
    assert_eq!(dome.frame(sibling), PixelRect::new(99, 19, 98, 18));
}

#[test]
fn refresh_listed_window_preserves_geometry_and_propagates_replacement_radius() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open_with("Window", |window| {
        window.corner_radius = Some(FractionalLength::new(3.5));
    });
    let original = dome.frame(window);
    {
        let overlays = dome.overlays();
        let shown = overlays
            .requested
            .as_ref()
            .unwrap()
            .tiling
            .values()
            .flat_map(|tiling| &tiling.windows)
            .find(|shown| shown.placement.id == window)
            .unwrap();
        assert_eq!(shown.corner_radius, FractionalLength::new(3.5));
    }
    let mut observation = dome.app_observation(dome.pid(window));
    observation.requested.remove(&window);
    observation
        .listed
        .iter_mut()
        .find(|listed| listed.id == window)
        .unwrap()
        .corner_radius = Some(FractionalLength::new(7.25));
    dome.deliver_notification(WindowNotification::AppWindows(observation));
    dome.run("toggle float");
    assert_eq!(dome.frame(window), original);
    assert_eq!(
        dome.overlays().floats[&window].corner_radius,
        FractionalLength::new(7.25)
    );
    dome.run("toggle float");
    let overlays = dome.overlays();
    let shown = overlays
        .requested
        .as_ref()
        .unwrap()
        .tiling
        .values()
        .flat_map(|tiling| &tiling.windows)
        .find(|shown| shown.placement.id == window)
        .unwrap();
    assert_eq!(shown.corner_radius, FractionalLength::new(7.25));
    assert_eq!(dome.frame(window), original);
}

#[test]
fn update_monitor_with_fractional_observation_places_inside_rounded_work_area() {
    let mut dome = TestDome::new(&PARTITION_CONFIG.replace("border_size = 1", "border_size = 0"));
    let window = dome.open("Window");
    let mut monitor = monitor(1, -100.75);
    monitor.y = -20.25;
    monitor.width = 100.5;
    monitor.height = 40.75;
    monitor.top_inset = 2.5;
    monitor.bottom_inset = 3.25;
    dome.monitors_changed(vec![monitor]);
    assert_eq!(dome.frame(window), PixelRect::new(-100, -17, 99, 34));
}

#[test]
fn resize_occupied_monitor_then_unplug_parks_window_and_focuses_remaining_overlay() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    let mut primary = monitor(1, 0.0);
    primary.width = 120.0;
    primary.height = 40.0;
    primary.top_inset = 5.0;
    primary.bottom_inset = 3.0;
    dome.monitors_changed(vec![primary, monitor(2, 120.0)]);
    assert_eq!(dome.frame(window), PixelRect::new(1, 6, 118, 30));
    dome.run("move monitor right");
    dome.run("focus monitor right");
    assert_eq!(dome.frame(window), PixelRect::new(121, 1, 98, 18));
    let mut primary = monitor(1, 0.0);
    primary.width = 120.0;
    primary.height = 30.0;
    dome.monitors_changed(vec![primary]);
    assert_eq!(dome.frame(window), PixelRect::new(119, 29, 98, 18));
    assert_eq!(dome.focused(), None);
    let state = dome.overlays();
    let placements = state.requested.as_ref().unwrap();
    assert_eq!(
        placements.focus_tiling,
        placements.tiling.keys().next().copied()
    );
}

#[test]
fn update_title_after_minimization_updates_entry_but_after_retirement_does_not_restore_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    dome.update_window(window, |window| window.minimized = true);
    dome.deliver_notification(WindowNotification::Title {
        id: window,
        title: Some("Delivered".into()),
    });
    let entries = dome.hub_request(|reply| HubMessage::Query {
        query: Query::MinimizedWindows,
        reply,
    });
    assert_eq!(entries[0]["title"], "Delivered");
    dome.remove(window);
    dome.deliver_notification(WindowNotification::Title {
        id: window,
        title: Some("Late".into()),
    });
    let entries = dome.hub_request(|reply| HubMessage::Query {
        query: Query::MinimizedWindows,
        reply,
    });
    assert_eq!(entries, serde_json::json!([]));
    assert!(dome.overlays().requested.as_ref().unwrap().tabs.is_empty());
}

#[test]
fn reconcile_app_batch_when_older_preserves_newer_admission() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let older = dome.app_observation(dome.pid(first));
    let later = dome.open("Later");
    let frames = (dome.frame(first), dome.frame(later));
    dome.deliver_notification(WindowNotification::AppWindows(older));
    assert_eq!((dome.frame(first), dome.frame(later)), frames);
    assert_eq!(dome.focused(), Some(later));
    dome.run("toggle layout");
    let container = dome.tab_container(&["First", "Later"], None);
    assert_eq!(
        dome.overlays().requested.as_ref().unwrap().tabs[&container]
            .1
            .placement
            .active_tab_index,
        1
    );
}

#[test]
fn reconcile_system_batch_removal_parks_departures_and_places_survivor() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let survivor = dome.open("Survivor");
    let second = dome.open("Second");
    let third = dome.open("Third");
    let second_size = dome.frame(second);
    let third_size = dome.frame(third);
    dome.pending_removals(&[second, third]);
    dome.deliver_notification(WindowNotification::AllWindows(SystemObservation {
        apps: vec![dome.app_observation(dome.pid(survivor))],
        terminated_pids: vec![],
        hidden_pids: vec![],
    }));
    assert_eq!(dome.frame(survivor), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.focused(), Some(survivor));
    assert_eq!(
        dome.frame(second),
        PixelRect::new(
            99,
            19,
            second_size.width().value(),
            second_size.height().value()
        )
    );
    assert_eq!(
        dome.frame(third),
        PixelRect::new(
            99,
            19,
            third_size.width().value(),
            third_size.height().value()
        )
    );
    dome.pending_removals(&[survivor]);
    dome.deliver_notification(WindowNotification::AppWindows(
        dome.app_observation(dome.pid(survivor)),
    ));
    assert_eq!(dome.focused(), None);
    assert!(
        dome.overlays()
            .requested
            .as_ref()
            .unwrap()
            .focus_tiling
            .is_some()
    );
}

#[test]
fn reconcile_system_batch_matches_admissions_to_workspaces_and_parks_inactive_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let layouts = PreferredLayouts::load_or_default(
        "layout.lua",
        Ok(r#"return { ["Display 1"] = {
        hidden = { layout = "partition_tree", tree = { { title = "Hidden" } } },
        active = { layout = "partition_tree", tree = { { title = "Active" } } },
    } }"#
            .to_owned()),
    );
    dome.hub_request(|reply| HubMessage::ApplyLayouts { layouts, reply });
    let hidden = dome.pending_window("Hidden", |_| {});
    let active = dome.pending_window("Active", |_| {});
    dome.deliver_notification(WindowNotification::AllWindows(SystemObservation {
        apps: vec![dome.app_observation(dome.pid(active))],
        terminated_pids: vec![],
        hidden_pids: vec![],
    }));
    assert_eq!(dome.focused(), Some(active));
    assert_eq!(dome.frame(hidden), PixelRect::new(99, 19, 40, 10));
    assert_eq!(dome.frame(active), PixelRect::new(1, 1, 98, 18));
    dome.run("focus workspace hidden");
    assert_eq!(dome.focused(), Some(hidden));
    assert_eq!(dome.frame(hidden), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(active), PixelRect::new(99, 19, 98, 18));
}
