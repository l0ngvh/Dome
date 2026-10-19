use crate::platform::windows::event_loop::{EventSender, RunLoop};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use super::{Dome, HubEvent};
use crate::platform::messages::HubSender;
use crate::platform::windows::window_server::WindowNotification;

impl Dome {
    pub(in crate::platform::windows) fn register_sources(
        notifications: UnboundedReceiver<WindowNotification>,
    ) -> (EventSender<HubEvent>, HubSender, EventSender<()>) {
        let (events, source) = RunLoop::channel();
        RunLoop::insert(source, |event, dome: &mut Dome| {
            let HubEvent::RunBinding { keymap, keystroke } = event;
            dome.run_binding(&keymap, &keystroke);
            dome.update_tray();
        });
        RunLoop::insert(notifications, |notification, dome: &mut Dome| {
            let refresh_menu = !matches!(
                notification,
                WindowNotification::MovementStarted(_) | WindowNotification::PeriodicTick
            );
            dome.handle_window_notification(notification);
            if refresh_menu {
                dome.update_tray();
            }
        });
        let (messages, source) = RunLoop::channel();
        RunLoop::insert(source, |message, dome: &mut Dome| {
            dome.handle_hub_message(message);
            dome.update_tray();
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
            dome.config_changed(config);
            dome.update_tray();
        });
        (events, hub, config)
    }
}
