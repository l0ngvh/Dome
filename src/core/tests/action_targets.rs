use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{ContainerId, PixelRect, WindowId, WindowRestrictions};
use crate::core::{PreferredTiling, Strategy, StrategyAction};

use super::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, setup_modes_on, snapshot_text,
    titled, titled_matcher, validate_hub,
};

const STRATEGIES: [Strategy; 2] = [Strategy::PartitionTree, Strategy::Master];

fn insert(hub: &mut Hub, title: &str) -> WindowId {
    hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
        .unwrap()
}

fn tabbed_container(hub: &Hub) -> ContainerId {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { containers, .. } = &placements.monitors[0].layout else {
        panic!("a fullscreen window hides the tab bars");
    };
    containers.iter().find(|c| c.is_tabbed).unwrap().id
}

#[test]
fn actions_on_the_focused_tiling_child_do_nothing_while_fullscreen_has_focus() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &["fs"]);
        insert(&mut hub, "t0");
        insert(&mut hub, "t1");
        insert(&mut hub, "t2");
        let before = snapshot_text(&hub);
        let fs = insert(&mut hub, "fs");

        hub.focus_left();
        hub.move_left();
        hub.toggle_spawn_mode();
        hub.toggle_direction();
        hub.toggle_container_layout();
        hub.focus_parent();
        hub.toggle_float();
        hub.delete_window(fs);

        validate_hub(&hub);
        assert_eq!(snapshot_text(&hub), before, "{strategy:?}");
    }
}

#[test]
fn tab_actions_do_nothing_while_fullscreen_has_focus() {
    for strategy in STRATEGIES {
        let mut hub = setup_modes_on(strategy, "0", &[], &["fs"]);
        insert(&mut hub, "t0");
        insert(&mut hub, "t1");
        insert(&mut hub, "t2");
        hub.toggle_container_layout();
        let container_id = tabbed_container(&hub);
        let before = snapshot_text(&hub);
        let fs = insert(&mut hub, "fs");

        hub.focus_next_tab();
        hub.focus_tab_index(container_id, 0);
        hub.delete_window(fs);

        validate_hub(&hub);
        assert_eq!(snapshot_text(&hub), before, "{strategy:?}");
    }
}

#[test]
fn master_layout_actions_run_while_a_float_has_focus_but_not_under_fullscreen() {
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout([LayoutWorkspaceConfigBuilder::new("0")
            .with_strategy(Strategy::Master)
            .with_master_count(2)
            .with_float(vec![titled_matcher("f")])
            .with_fullscreen(vec![titled_matcher("fs")])
            .build()])
        .build();
    let ws = hub.current_workspace();
    insert(&mut hub, "t0");
    insert(&mut hub, "t1");
    let master_layout = |hub: &Hub| {
        let PreferredTiling::Master(master) = hub.export_workspace(ws).tiling else {
            panic!("workspace 0 runs master");
        };
        (master.master_ratio, master.master_count)
    };
    let actions = [
        StrategyAction::GrowMaster,
        StrategyAction::ShrinkMaster,
        StrategyAction::MoreMaster,
        StrategyAction::FewerMaster,
    ];

    let fs = insert(&mut hub, "fs");
    for action in actions {
        hub.handle_tiling_action(action);
        assert_eq!(master_layout(&hub), (None, Some(2)), "under fullscreen");
    }
    hub.delete_window(fs);

    hub.insert_window(
        titled("f"),
        PixelRect::new(10, 5, 40, 10),
        WindowRestrictions::None,
    );
    let mut layouts = Vec::new();
    for action in [
        StrategyAction::GrowMaster,
        StrategyAction::ShrinkMaster,
        StrategyAction::MoreMaster,
        StrategyAction::FewerMaster,
    ] {
        hub.handle_tiling_action(action);
        layouts.push(master_layout(&hub));
    }
    validate_hub(&hub);
    assert_eq!(
        layouts,
        [
            (Some(0.55), Some(2)),
            (Some(0.5), Some(2)),
            (Some(0.5), Some(3)),
            (Some(0.5), Some(2)),
        ]
    );
}
