use crate::action::{Action, Actions, WindowId};
use crate::config::lua::test_support::{RecordingEffects, RecordingKeymap};
use crate::core::Hub;
use crate::core::node::WindowRestrictions;
use crate::core::tests::{default_rect, setup, titled};

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
