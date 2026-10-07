use crate::action::{Action, Actions, WindowId};
use crate::config::lua::test_support::{RecordingEffects, RecordingKeymap};
use crate::core::node::WindowRestrictions;
use crate::core::tests::{
    TestHubBuilder, TilingConfigBuilder, default_rect, setup, snapshot, titled,
};
use crate::core::{Hub, PreferredTiling, Strategy};

fn run(hub: &mut Hub, actions: &[&str]) -> (RecordingEffects, RecordingKeymap) {
    let actions = Actions::new(actions.iter().map(|s| s.parse().unwrap()).collect());
    let mut effects = RecordingEffects::default();
    let mut keymap = RecordingKeymap::default();
    hub.handle_actions(&actions, &mut effects, &mut keymap);
    (effects, keymap)
}

#[test]
fn platform_verbs_reach_the_effects() {
    let mut hub = setup();
    let (mut effects, keymap) = run(&mut hub, &["execute open -a Safari", "exit", "mode resize"]);
    let unminimize = Actions::new(vec![Action::UnminimizeWindow {
        id: WindowId::new(7),
    }]);
    hub.handle_actions(&unminimize, &mut effects, &mut RecordingKeymap::default());

    assert_eq!(effects.executed, vec!["open -a Safari".to_string()]);
    assert!(effects.exited);
    assert_eq!(effects.unminimized, vec![WindowId::new(7)]);
    assert_eq!(keymap.switched, vec!["resize".to_string()]);
}

#[test]
fn close_targets_the_focused_window() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_focus(w0);

    let (effects, _) = run(&mut hub, &["close"]);

    assert_eq!(effects.closed, vec![w0]);
}

#[test]
fn close_on_an_empty_workspace_closes_nothing() {
    let mut hub = setup();

    let (effects, _) = run(&mut hub, &["close"]);

    assert!(effects.closed.is_empty());
}

#[test]
fn tiling_verbs_drive_the_hub() {
    let mut hub = setup();

    let (effects, _) = run(&mut hub, &["focus workspace 3"]);

    let focused = hub
        .query_workspaces()
        .into_iter()
        .find(|w| w.is_focused)
        .expect("one workspace is focused");
    assert_eq!(focused.name, "3");
    assert!(effects.executed.is_empty() && effects.closed.is_empty() && !effects.exited);
}

#[test]
fn grow_and_shrink_resize_the_master_area() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let ws = hub.current_workspace();
    let master_ratio = |hub: &Hub| {
        let PreferredTiling::Master(master) = hub.export_workspace(ws).tiling else {
            panic!("the workspace runs master");
        };
        master.master_ratio
    };

    run(&mut hub, &["grow"]);
    assert_eq!(master_ratio(&hub), Some(0.55));

    run(&mut hub, &["shrink"]);
    assert_eq!(master_ratio(&hub), Some(0.5));
}

#[test]
fn grow_and_shrink_do_nothing_on_a_partition_tree_workspace() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let before = snapshot(&hub);

    for verb in ["grow", "shrink"] {
        run(&mut hub, &[verb]);
        assert_eq!(snapshot(&hub), before, "{verb}");
    }
}

#[test]
fn master_verbs_do_nothing_on_a_partition_tree_workspace() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    let before = snapshot(&hub);

    for verb in ["master more", "master fewer"] {
        run(&mut hub, &[verb]);
        assert_eq!(snapshot(&hub), before, "{verb}");
    }
}

#[test]
fn partition_tree_verbs_do_nothing_on_a_master_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    let before = snapshot(&hub);

    for verb in ["toggle spawn", "toggle direction", "focus parent"] {
        run(&mut hub, &[verb]);
        assert_eq!(snapshot(&hub), before, "{verb}");
    }
}
