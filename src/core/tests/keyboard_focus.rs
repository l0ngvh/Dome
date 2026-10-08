use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{PixelRect, WindowId, WindowRestrictions};
use crate::core::{PreferredTiling, Strategy};

use super::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, preferred_layout, setup_modes_on,
    snapshot, titled, titled_matcher, validate_hub,
};

const STRATEGIES: [Strategy; 2] = [Strategy::PartitionTree, Strategy::Master];

fn float_rect() -> PixelRect {
    PixelRect::new(10, 5, 40, 10)
}

fn insert(hub: &mut Hub, title: &str, rect: PixelRect) -> WindowId {
    hub.insert_window(titled(title), rect, WindowRestrictions::None)
        .unwrap()
}

fn focused_monitor_layout(hub: &Hub) -> MonitorLayout {
    let placements = hub.get_visible_placements();
    let focused_monitor = placements.focused_monitor;
    placements
        .monitors
        .into_iter()
        .find(|m| m.monitor_id == focused_monitor)
        .unwrap()
        .layout
}

/// The windows the focused monitor highlights. A fullscreen layout counts as highlighting its
/// window.
fn highlighted_windows(hub: &Hub) -> Vec<WindowId> {
    match focused_monitor_layout(hub) {
        MonitorLayout::Normal {
            tiling_windows,
            float_windows,
            ..
        } => tiling_windows
            .iter()
            .map(|p| (p.id, p.is_highlighted))
            .chain(float_windows.iter().map(|p| (p.id, p.is_highlighted)))
            .filter(|&(_, is_highlighted)| is_highlighted)
            .map(|(id, _)| id)
            .collect(),
        MonitorLayout::Fullscreen(id) => vec![id],
    }
}

/// The titles in the container the focused monitor highlights, empty when it highlights no
/// container.
fn highlighted_container_titles(hub: &Hub) -> Vec<String> {
    match focused_monitor_layout(hub) {
        MonitorLayout::Normal { containers, .. } => containers
            .into_iter()
            .find(|c| c.is_highlighted)
            .map(|c| c.titles)
            .unwrap_or_default(),
        MonitorLayout::Fullscreen(_) => Vec::new(),
    }
}

/// The window that covers the focused monitor.
fn fullscreen_window(hub: &Hub) -> Option<WindowId> {
    match focused_monitor_layout(hub) {
        MonitorLayout::Fullscreen(id) => Some(id),
        MonitorLayout::Normal { .. } => None,
    }
}

fn float_border_box(hub: &Hub, window_id: WindowId) -> Option<PixelRect> {
    hub.get_visible_placements()
        .monitors
        .iter()
        .find_map(|m| match &m.layout {
            MonitorLayout::Normal { float_windows, .. } => float_windows
                .iter()
                .find(|p| p.id == window_id)
                .map(|p| p.border_box),
            MonitorLayout::Fullscreen(_) => None,
        })
}

#[test]
fn closing_the_last_fullscreen_window_focuses_a_float_when_no_tiling_window_remains() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        let f = insert(&mut hub, "f", float_rect());
        let fs = insert(&mut hub, "fs", default_rect());

        hub.delete_window(fs);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
    }
}

#[test]
fn minimizing_the_last_fullscreen_window_focuses_a_float_when_no_tiling_window_remains() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        let f = insert(&mut hub, "f", float_rect());
        let fs = insert(&mut hub, "fs", default_rect());

        hub.minimize_window(fs);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
    }
}

#[test]
fn moving_the_last_fullscreen_window_away_focuses_the_float_left_behind() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        let f = insert(&mut hub, "f", float_rect());
        insert(&mut hub, "fs", default_rect());

        hub.move_focused_to_workspace("1", None);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
    }
}

#[test]
fn focusing_a_tiling_window_takes_focus_from_the_float() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &[]);
        let ws = hub.current_workspace();
        let t0 = insert(&mut hub, "t0", default_rect());
        insert(&mut hub, "t1", default_rect());
        insert(&mut hub, "f", float_rect());

        hub.set_focus(t0);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(t0), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [t0], "{strategy:?}");
    }
}

#[test]
fn toggling_float_moves_focus_with_the_window() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &[]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t0", default_rect());
        let t1 = insert(&mut hub, "t1", default_rect());

        hub.toggle_float();
        validate_hub(&hub);
        assert_eq!(highlighted_windows(&hub), [t1], "{strategy:?}: floating");
        assert_eq!(hub.export_workspace(ws).float.len(), 1, "{strategy:?}");

        hub.toggle_float();
        validate_hub(&hub);
        assert_eq!(
            highlighted_windows(&hub),
            [t1],
            "{strategy:?}: tiling again"
        );
        assert_eq!(hub.export_workspace(ws).float.len(), 0, "{strategy:?}");
    }
}

#[test]
fn ending_the_topmost_fullscreen_window_focuses_the_one_below_it() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &["fs1", "fs2"]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let fs1 = insert(&mut hub, "fs1", default_rect());
        let fs2 = insert(&mut hub, "fs2", default_rect());

        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs1), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [fs1], "{strategy:?}");

        hub.delete_window(fs1);

        validate_hub(&hub);
        assert_eq!(
            hub.focused_window(ws),
            Some(fs2),
            "{strategy:?}: the window that left fullscreen holds tiling focus"
        );
    }
}

#[test]
fn an_app_ending_a_covered_fullscreen_window_leaves_focus_on_the_topmost() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &["fs1", "fs2"]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let fs1 = insert(&mut hub, "fs1", default_rect());
        let fs2 = insert(&mut hub, "fs2", default_rect());

        hub.unset_fullscreen(fs1);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs2), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [fs2], "{strategy:?}");

        hub.delete_window(fs2);

        validate_hub(&hub);
        assert_eq!(
            hub.focused_window(ws),
            Some(fs1),
            "{strategy:?}: the window that left fullscreen holds tiling focus"
        );
    }
}

#[test]
fn ending_fullscreen_focuses_the_window_over_a_float_selected_underneath() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());
        let fs = insert(&mut hub, "fs", default_rect());
        hub.set_focus(f);
        assert_eq!(hub.focused_window(ws), Some(fs), "{strategy:?}");

        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [fs], "{strategy:?}");
    }
}

#[test]
fn an_app_reporting_fullscreen_again_changes_only_its_restrictions() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &["fs1", "fs2"]);
        let ws = hub.current_workspace();
        let fs1 = insert(&mut hub, "fs1", default_rect());
        let fs2 = insert(&mut hub, "fs2", default_rect());

        hub.set_fullscreen(fs1, WindowRestrictions::BlockAll);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs2), "{strategy:?}");
        assert_eq!(
            hub.access.windows.get(fs1).restrictions,
            WindowRestrictions::BlockAll
        );
        hub.delete_window(fs2);
        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs1), "{strategy:?}");
    }
}

#[test]
fn toggling_fullscreen_on_a_float_and_back_tiles_it() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &[]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());

        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
        let export = hub.export_workspace(ws);
        assert_eq!((export.float.len(), export.fullscreen.len()), (0, 1));

        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
        let export = hub.export_workspace(ws);
        assert_eq!(
            (export.float.len(), export.fullscreen.len()),
            (0, 0),
            "{strategy:?}: the window tiles"
        );
    }
}

#[test]
fn toggling_fullscreen_from_tiling_or_from_a_float_covers_the_monitor() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &[]);
        let ws = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());
        hub.set_focus(t);

        hub.toggle_fullscreen();

        let covered = snapshot(&hub);
        assert_eq!(fullscreen_window(&hub), Some(t), "{strategy:?}");
        hub.set_focus(f);
        assert_eq!(
            snapshot(&hub),
            covered,
            "{strategy:?}: the fullscreen window outranks the float"
        );

        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(fullscreen_window(&hub), None, "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [t], "{strategy:?}");
        let export = hub.export_workspace(ws);
        assert_eq!(
            (export.float.len(), export.fullscreen.len()),
            (1, 0),
            "{strategy:?}: t tiles again and f still floats"
        );

        hub.set_focus(f);
        hub.toggle_fullscreen();

        validate_hub(&hub);
        assert_eq!(fullscreen_window(&hub), Some(f), "{strategy:?}");
        let export = hub.export_workspace(ws);
        assert_eq!(
            (export.float.len(), export.fullscreen.len()),
            (0, 1),
            "{strategy:?}: f leaves the float list"
        );
    }
}

#[test]
fn closing_a_window_an_app_made_fullscreen_returns_focus_to_the_float() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &[]);
        let ws = hub.current_workspace();
        insert(&mut hub, "t0", default_rect());
        let t1 = insert(&mut hub, "t1", default_rect());
        let f = insert(&mut hub, "f", float_rect());

        hub.set_fullscreen(t1, WindowRestrictions::ProtectFullscreen);
        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(t1), "{strategy:?}");

        hub.delete_window(t1);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [f], "{strategy:?}");
    }
}

#[test]
fn moving_a_fullscreen_window_clears_float_focus_on_the_target() {
    for strategy in STRATEGIES {
        let mut hub = TestHubBuilder::new()
            .with_preferred_layout([
                LayoutWorkspaceConfigBuilder::new("0")
                    .with_strategy(strategy)
                    .with_fullscreen(vec![titled_matcher("fs")])
                    .build(),
                LayoutWorkspaceConfigBuilder::new("1")
                    .with_strategy(strategy)
                    .with_float(vec![titled_matcher("f")])
                    .build(),
            ])
            .build();
        hub.focus_workspace("1", None);
        let target = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        insert(&mut hub, "f", float_rect());
        hub.focus_workspace("0", None);
        let fs = insert(&mut hub, "fs", default_rect());

        hub.move_focused_to_workspace("1", None);
        hub.delete_window(fs);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(target), Some(t), "{strategy:?}");
    }
}

#[test]
fn a_container_moved_onto_a_workspace_whose_float_has_focus_takes_focus() {
    for strategy in STRATEGIES {
        let mut hub = TestHubBuilder::new()
            .with_preferred_layout([LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(strategy)
                .with_float(vec![titled_matcher("f")])
                .build()])
            .build();
        hub.focus_workspace("1", None);
        let target = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());
        hub.focus_workspace("0", None);
        insert(&mut hub, "a", default_rect());
        let b = insert(&mut hub, "b", default_rect());
        hub.focus_parent();
        assert_eq!(
            highlighted_container_titles(&hub),
            ["a", "b"],
            "{strategy:?}: a container has focus before the move"
        );

        hub.move_focused_to_workspace("1", None);

        validate_hub(&hub);
        hub.focus_workspace("1", None);
        match strategy {
            Strategy::PartitionTree => {
                assert_eq!(hub.focused_window(target), None, "{strategy:?}");
                assert_eq!(
                    highlighted_container_titles(&hub),
                    ["a", "b"],
                    "{strategy:?}"
                );
            }
            Strategy::Master => assert_eq!(highlighted_windows(&hub), [b], "{strategy:?}"),
            Strategy::Scrolling => unreachable!("STRATEGIES lists no scrolling workspace"),
        }
        assert!(
            !highlighted_windows(&hub).contains(&f),
            "{strategy:?}: the float lost its highlight"
        );
    }
}

#[test]
fn a_container_moved_under_a_fullscreen_window_takes_focus_once_it_closes() {
    for strategy in STRATEGIES {
        let mut hub = TestHubBuilder::new()
            .with_preferred_layout([LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(strategy)
                .with_fullscreen(vec![titled_matcher("fs")])
                .build()])
            .build();
        hub.focus_workspace("1", None);
        let target = hub.current_workspace();
        insert(&mut hub, "t", default_rect());
        let fs = insert(&mut hub, "fs", default_rect());
        hub.focus_workspace("0", None);
        insert(&mut hub, "a", default_rect());
        let b = insert(&mut hub, "b", default_rect());
        hub.focus_parent();
        assert_eq!(
            highlighted_container_titles(&hub),
            ["a", "b"],
            "{strategy:?}: a container has focus before the move"
        );

        hub.move_focused_to_workspace("1", None);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(target), Some(fs), "{strategy:?}");
        hub.delete_window(fs);
        validate_hub(&hub);
        hub.focus_workspace("1", None);
        match strategy {
            Strategy::PartitionTree => {
                assert_eq!(hub.focused_window(target), None, "{strategy:?}");
                assert_eq!(
                    highlighted_container_titles(&hub),
                    ["a", "b"],
                    "{strategy:?}"
                );
            }
            Strategy::Master => assert_eq!(highlighted_windows(&hub), [b], "{strategy:?}"),
            Strategy::Scrolling => unreachable!("STRATEGIES lists no scrolling workspace"),
        }
    }
}

#[test]
fn restoring_a_fullscreen_window_clears_float_focus() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());
        let fs = insert(&mut hub, "fs", default_rect());
        hub.minimize_window(fs);
        hub.set_focus(f);

        hub.unminimize_window(fs);
        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(fs), "{strategy:?}");

        hub.delete_window(fs);

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(t), "{strategy:?}");
    }
}

#[test]
fn a_reset_focuses_the_topmost_fullscreen_window_and_keeps_both_stacks() {
    for strategy in STRATEGIES {
        let layout = LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(strategy)
            .with_float(vec![titled_matcher("f1"), titled_matcher("f2")])
            .with_fullscreen(vec![titled_matcher("fs1"), titled_matcher("fs2")])
            .build();
        let mut hub = TestHubBuilder::new()
            .with_preferred_layout([layout.clone()])
            .build();
        let ws = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        let f1 = insert(&mut hub, "f1", float_rect());
        let f2 = insert(&mut hub, "f2", float_rect());
        let fs1 = insert(&mut hub, "fs1", default_rect());
        let fs2 = insert(&mut hub, "fs2", default_rect());
        hub.set_focus(t);

        hub.apply_preferred_layouts(preferred_layout([layout]));

        validate_hub(&hub);
        let mut order = Vec::new();
        for window_id in [fs2, fs1, f2, f1, t] {
            order.push(hub.focused_window(ws));
            hub.delete_window(window_id);
            validate_hub(&hub);
        }
        assert_eq!(
            order,
            [Some(fs2), Some(fs1), Some(f2), Some(f1), Some(t)],
            "{strategy:?}: focus after each close"
        );
    }
}

#[test]
fn a_reset_focuses_the_topmost_float_over_the_tiling_windows() {
    for strategy in STRATEGIES {
        let layout = LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(strategy)
            .with_float(vec![titled_matcher("f1"), titled_matcher("f2")])
            .build();
        let mut hub = TestHubBuilder::new()
            .with_preferred_layout([layout.clone()])
            .build();
        let ws = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        insert(&mut hub, "f1", float_rect());
        let f2 = insert(&mut hub, "f2", float_rect());
        hub.set_focus(t);

        hub.apply_preferred_layouts(preferred_layout([layout]));

        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f2), "{strategy:?}");
        assert_eq!(highlighted_windows(&hub), [f2], "{strategy:?}");
    }
}

#[test]
fn a_reset_into_another_strategy_keeps_each_window_in_its_display_mode() {
    for (from, to) in [
        (Strategy::PartitionTree, Strategy::Master),
        (Strategy::Master, Strategy::PartitionTree),
    ] {
        let mut hub = setup_modes_on(from, "0", &["f"], &["fs"]);
        let ws = hub.current_workspace();
        let t = insert(&mut hub, "t", default_rect());
        let f = insert(&mut hub, "f", float_rect());
        let fs = insert(&mut hub, "fs", default_rect());

        hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(to)
            .build()]));

        validate_hub(&hub);
        let export = hub.export_workspace(ws);
        assert_eq!(export.tiling.strategy(), to);
        assert!(
            matches!(
                export.tiling,
                PreferredTiling::PartitionTree { tree: Some(_) }
            ) || matches!(&export.tiling, PreferredTiling::Master(m) if m.master.children.len() == 1),
            "{from:?} to {to:?}: t tiles"
        );
        assert_eq!(hub.focused_window(ws), Some(fs), "{from:?} to {to:?}");
        hub.delete_window(fs);
        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(f), "{from:?} to {to:?}");
        assert_eq!(float_border_box(&hub, f), Some(float_rect()));
        hub.delete_window(f);
        validate_hub(&hub);
        assert_eq!(hub.focused_window(ws), Some(t), "{from:?} to {to:?}");
    }
}
