use std::collections::HashSet;

use calloop::EventLoop;
use calloop::channel::{Channel, Event as ChannelEvent};

use objc2::rc::autoreleasepool;

use crate::config::Config;
use crate::core::Hub;
use crate::platform::macos::ui::MessageSender;

use super::inspect::{AsyncResult, GcdDispatcher};
use super::monitor::{MonitorInfo, MonitorRegistry};
use super::placement_tracker::PlacementTracker;
use super::registry::Registry;
use super::{Dome, HubEvent, HubMessage, recovery};

use crate::platform::macos::running_application::RunningApp;

impl Dome {
    pub(in crate::platform::macos) fn start(
        config: Config,
        screens: Vec<MonitorInfo>,
        sender: MessageSender,
        channel: Channel<HubEvent>,
    ) {
        recovery::install_handlers();
        let mut event_loop =
            EventLoop::<'static, Self>::try_new().expect("Failed to create event loop");
        let handle = event_loop.handle();
        let signal = event_loop.get_signal();

        let (async_tx, async_rx) = calloop::channel::channel();
        let dispatcher = GcdDispatcher::new(async_tx);

        let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
        let mut hub = Hub::new(primary.dimension, config.clone().into());
        let primary_monitor_id = hub.focused_monitor();
        let mut monitor_registry = MonitorRegistry::new(primary, primary_monitor_id);
        tracing::info!(%primary, "Primary monitor");

        for screen in &screens {
            if screen.display_id != primary.display_id {
                let id = hub.add_monitor(screen.name.clone(), screen.dimension);
                monitor_registry.insert(screen, id);
                tracing::info!(%screen, "Monitor");
            }
        }

        // Drain initial allocations from Hub::new() and add_monitor()
        hub.drain_changes();

        let mut dome = Self {
            hub,
            registry: Registry::new(),
            monitor_registry,
            config,
            primary_screen: primary.dimension,
            primary_full_height: primary.full_height,
            observed_pids: HashSet::new(),
            sender: Box::new(sender),
            dispatcher,
            signal,
            placement_tracker: PlacementTracker::new(handle.clone()),
            last_focused: None,
        };

        handle
            .insert_source(channel, |event, _, dome| match event {
                ChannelEvent::Msg(hub_event) => dome.handle_external_event(hub_event),
                ChannelEvent::Closed => dome.signal.stop(),
            })
            .expect("Failed to insert channel source");

        handle
            .insert_source(async_rx, |event, _, dome| match event {
                ChannelEvent::Msg(result) => dome.handle_inspect_result(result),
                ChannelEvent::Closed => {}
            })
            .expect("Failed to insert async channel source");

        dome.dispatcher.reconcile_all(
            dome.observed_pids.clone(),
            dome.registry
                .iter()
                .map(|(id, e)| (id, e.clone()))
                .collect(),
            dome.config.macos.ignore.clone(),
        );
        event_loop
            .run(None, &mut dome, |_| {})
            .expect("Event loop failed");
    }

    #[tracing::instrument(skip(self), fields(%event))]
    fn handle_external_event(&mut self, event: HubEvent) {
        autoreleasepool(|_| {
            match event {
                HubEvent::Shutdown => {
                    tracing::info!("Shutdown requested");
                    self.signal.stop();
                    return;
                }
                HubEvent::ConfigChanged(new_config) => {
                    self.hub.sync_config(new_config.clone().into());
                    self.sender
                        .send(HubMessage::ConfigChanged(new_config.clone()));
                    self.config = new_config;
                    tracing::info!("Config reloaded");
                }
                HubEvent::VisibleWindowsChanged { pid } => {
                    self.dispatch_refresh_windows(pid);
                }
                HubEvent::SyncFocus { pid } => {
                    if let Some(app) = RunningApp::new(pid) {
                        self.sync_app_focus(&app);
                    }
                }
                HubEvent::AppTerminated { pid } => {
                    tracing::debug!(pid, "App terminated");
                    self.remove_app_windows(pid);
                }
                HubEvent::TitleChanged(cg_id) => {
                    if let Some(entry) = self.registry.get_mut(cg_id) {
                        entry.ax.update_title();
                        tracing::trace!(%entry, "Title changed");
                    }
                }
                HubEvent::WindowMovedOrResized { pid } => {
                    self.placement_tracker.window_moved(pid);
                    return;
                }
                HubEvent::Action(actions) => {
                    tracing::debug!(%actions, "Executing actions");
                    self.execute_actions(&actions);
                }
                HubEvent::Sync => {
                    self.dispatcher.reconcile_all(
                        self.observed_pids.clone(),
                        self.registry
                            .iter()
                            .map(|(id, e)| (id, e.clone()))
                            .collect(),
                        self.config.macos.ignore.clone(),
                    );
                }
                HubEvent::ScreensChanged(screens) => {
                    tracing::info!(count = screens.len(), "Screens changed");
                    self.update_screens(screens);
                }
                HubEvent::MirrorClicked(window_id) => {
                    let entry = self.registry.by_id(window_id);
                    if let Err(e) = entry.ax.focus() {
                        tracing::debug!("Failed to focus window: {e:#}");
                    }
                    self.hub.set_focus(window_id);
                }
                HubEvent::TabClicked(container_id, tab_idx) => {
                    self.hub.focus_tab_index(container_id, tab_idx);
                }
                HubEvent::SpaceChanged => {
                    self.handle_space_changed();
                }
            }

            self.flush_layout();
        });
    }

    fn handle_inspect_result(&mut self, result: AsyncResult) {
        autoreleasepool(|_| match result {
            AsyncResult::AppWindowsReconciled { to_remove, to_add } => {
                self.apply_windows_reconciled(to_remove, to_add);
                self.flush_layout();
            }
            AsyncResult::AppWindowPositions(positions) => {
                self.apply_window_positions(positions);
                self.flush_layout();
            }
            AsyncResult::AllWindowsReconciled {
                terminated_pids,
                new_apps,
                hidden_pids,
                to_remove,
                to_add,
            } => {
                for pid in terminated_pids {
                    // FIXME: cleanup observer for terminated apps
                    self.observed_pids.remove(&pid);
                    self.remove_app_windows(pid);
                }
                for pid in hidden_pids.clone() {
                    self.remove_app_windows(pid);
                }
                self.apply_windows_reconciled(to_remove, to_add);
                if !new_apps.is_empty() {
                    for app in &new_apps {
                        self.observed_pids.insert(app.pid());
                    }
                    self.sender.send(HubMessage::RegisterObservers(new_apps));
                }
                // Windows moved/resized events aren't fired from time to time, like when windows
                // are brought into view after new monitors are plugged in, or when windows moved
                // from fullscreen.
                for &pid in &self.observed_pids {
                    if !hidden_pids.contains(&pid) && !self.placement_tracker.is_moving(pid) {
                        self.placement_tracker.window_moved(pid);
                    }
                }
                self.flush_layout();
            }
        });
    }
}
