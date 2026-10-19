use crate::platform::macos::event_loop::{EventSender, RunLoop};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use super::{Dome, HubEvent};
use crate::platform::macos::window_server::WindowNotification;
use crate::platform::messages::HubSender;

impl Dome {
    pub(in crate::platform::macos) fn register_sources(
        events: UnboundedReceiver<HubEvent>,
        notifications: UnboundedReceiver<WindowNotification>,
    ) -> (HubSender, EventSender<()>) {
        RunLoop::insert(events, |event, dome: &mut Self| {
            let HubEvent::RunBinding { keymap, keystroke } = event;
            dome.run_binding(&keymap, &keystroke);
            dome.update_menu_bar();
        });
        RunLoop::insert(notifications, |notification, dome: &mut Dome| {
            let refresh_menu = !matches!(
                notification,
                WindowNotification::MovementStarted { .. }
                    | WindowNotification::MovementEnded { .. }
            );
            dome.handle_window_notification(notification);
            if refresh_menu {
                dome.update_menu_bar();
            }
        });
        let (messages, source) = RunLoop::channel();
        RunLoop::insert(source, |message, dome: &mut Self| {
            dome.handle_hub_message(message);
            dome.update_menu_bar();
        });
        let hub = HubSender(messages);
        let (config, changes) = RunLoop::channel();
        let loaded = changes.filter_map(|()| async {
            let path = RunLoop::with_dome(|dome| dome.hub.config_path().to_owned());
            let source = RunLoop::dispatch(move |_| std::fs::read_to_string(path));
            let source = source
                .await
                .map_err(anyhow::Error::from)
                .and_then(|source| source.map_err(Into::into));
            RunLoop::with_dome(|dome| dome.hub.load_config(source, &mut dome.keymap_publisher))
        });
        RunLoop::insert(loaded, |config, dome: &mut Self| {
            let start_at_login = config.start_at_login;
            dome.config_changed(config);
            crate::platform::macos::login_item::sync_login_item(
                start_at_login,
                dome.bundle_path.as_deref(),
            );
            dome.update_menu_bar();
        });
        (hub, config)
    }
}
