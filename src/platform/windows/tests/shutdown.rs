use super::*;
use crate::platform::windows::dome::events::{HubMessage, WindowCommand};
use crate::platform::windows::window_server::{Observation, SettleRead};

#[test]
fn stop_hands_first_managed_recovery_facts_once() {
    let mut env = TestEnv::new();
    env.hold_scenes(true);
    let retained = env.open();
    env.inject_observation(Observation::Settled {
        window: retained,
        read: SettleRead {
            observed_at: env.now(),
            minimized: false,
            is_maximized: true,
            placement: Some((PixelRect::new(10, 10, 800, 600), 1)),
        },
    });
    env.hold_scenes(false);
    env.release();
    env.inject_observation(Observation::Settled {
        window: retained,
        read: SettleRead {
            observed_at: env.now(),
            minimized: false,
            is_maximized: false,
            placement: Some((PixelRect::from_dimension(env.dim(retained)), 1)),
        },
    });
    let removed = env.open();
    env.destroy_window(removed);
    let mut revoked = env.latest_scene();
    revoked.windows.remove(&retained);
    revoked.monitors.iter_mut().for_each(|m| {
        m.tiling_windows.clear();
        m.float_windows.clear();
    });
    revoked.focus_request = None;
    env.send_ui(HubMessage::Scene(revoked));
    env.send_ui(HubMessage::Command(WindowCommand::Close(retained)));
    env.ui_wake();
    assert!(
        env.close_submissions().is_empty(),
        "revocation must also reject commands"
    );
    env.hold_scenes(true);
    let never_written = env.open();
    env.send_ui(HubMessage::Shutdown);
    env.ui_wake();
    env.stop();
    assert_eq!(env.recovery_records(), vec![(retained, true)]);
    assert!(
        !env.recovery_records()
            .iter()
            .any(|(key, _)| *key == removed || *key == never_written)
    );
    assert_eq!(env.recovery_calls(), 1);
}
