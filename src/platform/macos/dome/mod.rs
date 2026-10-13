pub(super) mod events;
mod layout;
mod monitor;

pub(super) use events::{HubEvent, HubMessage};
pub(super) use monitor::{MonitorInfo, get_all_monitors};

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use calloop::LoopSignal;
use objc2_core_graphics::CGWindowID;

use crate::action::{Actions, MinimizedWindow};
use crate::config::{Config, Keystroke, LuaRuntime, PlatformEffects, PreferredLayouts};
use crate::core::{
    ContainerId, Hub, LimitObservation, PixelRect, TilingConfig, WindowId, WindowRestrictions,
};
use crate::platform::keymap::KeymapPublisher;

use events::{DiscoveredWindow, FocusTarget, FullscreenKind, WindowCommand};
use monitor::MonitorRegistry;

struct MacOsPlatformEffects<'a> {
    native_keys: &'a HashMap<WindowId, CGWindowID>,
    minimized: HashSet<WindowId>,
    sender: &'a dyn SceneSender,
    signal: &'a LoopSignal,
    env: &'a HashMap<String, String>,
}

impl PlatformEffects for MacOsPlatformEffects<'_> {
    fn close(&mut self, id: WindowId) {
        if let Some(&key) = self.native_keys.get(&id) {
            self.sender
                .send(HubMessage::Command(WindowCommand::Close(key)));
        }
    }

    fn execute(&mut self, command: &str) {
        if let Err(error) = crate::platform::macos::spawn::spawn_disclaimed_sh(command, self.env) {
            tracing::warn!(%command, "Failed to execute: {error}");
        }
    }

    fn exit(&mut self) {
        self.signal.stop();
    }

    fn unminimize(&mut self, id: WindowId) {
        if self.minimized.contains(&id)
            && let Some(&key) = self.native_keys.get(&id)
        {
            self.sender
                .send(HubMessage::Command(WindowCommand::Unminimize(key)));
        }
    }
}

pub(in crate::platform::macos) trait SceneSender: Send {
    fn send(&self, message: HubMessage);
}

/// Domain policy retains identities and discovery metadata, never native handles.
pub(in crate::platform::macos) struct Dome {
    hub: Hub,
    window_ids: HashMap<CGWindowID, WindowId>,
    native_keys: HashMap<WindowId, CGWindowID>,
    // Ignored discoveries remain eligible for admission after a rule reload.
    discoveries: HashMap<CGWindowID, DiscoveredWindow>,
    monitor_registry: MonitorRegistry,
    sender: Box<dyn SceneSender>,
    focus_request: Option<FocusTarget>,
    last_focus_target: Option<FocusTarget>,
    monitors: Vec<MonitorInfo>,
    displays_changed_at: Instant,
    keymap_publisher: KeymapPublisher,
    env: HashMap<String, String>,
}

impl Dome {
    #[cfg(test)]
    pub(in crate::platform::macos) fn with_hub<R>(
        &mut self,
        change: impl FnOnce(&mut Hub) -> R,
    ) -> R {
        let result = change(&mut self.hub);
        self.flush_layout();
        result
    }

    pub(in crate::platform::macos) fn new(
        monitors: &[MonitorInfo],
        tiling: TilingConfig,
        workspace_overrides: PreferredLayouts,
        sender: Box<dyn SceneSender>,
        runtime: LuaRuntime,
        keymap_publisher: KeymapPublisher,
        env: HashMap<String, String>,
    ) -> Self {
        let primary = monitors
            .iter()
            .find(|monitor| monitor.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.into(), tiling, workspace_overrides, runtime);
        let mut monitor_registry = MonitorRegistry::new(primary, hub.primary_monitor());
        for monitor in monitors {
            if monitor.display_id != primary.display_id {
                let id = hub.add_monitor(monitor.into());
                monitor_registry.insert(monitor, id);
            }
        }
        Self {
            hub,
            window_ids: HashMap::new(),
            native_keys: HashMap::new(),
            discoveries: HashMap::new(),
            monitor_registry,
            sender,
            focus_request: None,
            last_focus_target: None,
            monitors: monitors.to_vec(),
            displays_changed_at: Instant::now(),
            keymap_publisher,
            env,
        }
    }

    fn current_id(&self, key: CGWindowID) -> Option<WindowId> {
        self.window_ids.get(&key).copied()
    }

    pub(in crate::platform::macos) fn windows_discovered(
        &mut self,
        windows: Vec<DiscoveredWindow>,
    ) {
        for mut discovery in windows {
            if discovery.fullscreen.is_none()
                && discovery
                    .frame
                    .is_some_and(|frame| self.monitor_registry.is_borderless_fullscreen_at(frame))
            {
                discovery.fullscreen = Some(FullscreenKind::Borderless);
            }
            let key = discovery.key;
            if self
                .discoveries
                .get(&key)
                .is_some_and(|old| old.process_id != discovery.process_id)
            {
                self.window_gone(key);
            }
            self.discoveries.insert(key, discovery);
            self.admit_discovery(key);
        }
    }

    fn admit_discovery(&mut self, key: CGWindowID) {
        if self.current_id(key).is_some() {
            return;
        }
        let Some(discovery) = self.discoveries.get(&key) else {
            return;
        };
        let restrictions = if discovery.fullscreen.is_some() {
            WindowRestrictions::ProtectFullscreen
        } else {
            WindowRestrictions::None
        };
        let frame = match discovery.frame {
            Some(frame) => frame,
            None if discovery.minimized || discovery.fullscreen.is_some() => {
                PixelRect::new(0, 0, 1, 1)
            }
            None => return,
        };
        let Some(id) =
            self.hub
                .insert_window(Box::new(discovery.metadata.clone()), frame, restrictions)
        else {
            return;
        };
        self.window_ids.insert(key, id);
        self.native_keys.insert(id, key);
        if discovery.minimized {
            self.hub.minimize_window(id);
        }
        tracing::info!(%id, key, process_id = discovery.process_id, metadata = %discovery.metadata, "New window");
    }

    pub(in crate::platform::macos) fn window_gone(&mut self, key: CGWindowID) {
        self.discoveries.remove(&key);
        if let Some(id) = self.window_ids.remove(&key) {
            self.native_keys.remove(&id);
            self.hub.delete_window(id);
        }
    }

    pub(in crate::platform::macos) fn window_minimized(&mut self, key: CGWindowID) {
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.minimized = true;
        }
        if let Some(id) = self.current_id(key)
            && !self.hub.window(id).is_minimized()
        {
            self.hub.minimize_window(id);
        }
    }

    pub(in crate::platform::macos) fn window_restored(&mut self, key: CGWindowID) {
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.minimized = false;
        }
        if let Some(id) = self.current_id(key)
            && self.hub.window(id).is_minimized()
        {
            self.hub.unminimize_window(id);
        }
    }

    pub(in crate::platform::macos) fn focus_reported(&mut self, key: CGWindowID) {
        self.focus_request = None;
        if let Some(id) = self.current_id(key)
            && self.hub.window(id).workspace().is_some()
            && !self.hub.window(id).is_minimized()
        {
            self.hub.set_focus(id);
        }
        // An unresolved foreground must not reassert the Hub's previous focus.
        self.last_focus_target = Some(self.current_focus_target());
    }

    pub(in crate::platform::macos) fn title_changed(&mut self, key: CGWindowID, title: String) {
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.metadata.title = Some(title.clone());
        }
        if let Some(id) = self.current_id(key) {
            self.hub.set_window_title(id, title);
        }
    }

    pub(in crate::platform::macos) fn size_constraint(
        &mut self,
        key: CGWindowID,
        limits: LimitObservation,
    ) {
        let Some(id) = self.current_id(key) else {
            return;
        };
        let window = self.hub.window(id);
        if window.is_minimized()
            || window.workspace().is_none()
            || window.is_float()
            || window.is_fullscreen()
        {
            return;
        }
        let visible = self.hub.get_visible_placements();
        if visible
            .monitors
            .iter()
            .any(|monitor| match &monitor.layout {
                crate::core::MonitorLayout::Normal { tiling_windows, .. } => {
                    tiling_windows.iter().any(|placement| placement.id == id)
                }
                crate::core::MonitorLayout::Fullscreen(_) => false,
            })
        {
            self.hub.set_window_constraint(id, limits);
        }
    }

    pub(in crate::platform::macos) fn float_moved(&mut self, key: CGWindowID, frame: PixelRect) {
        let Some(id) = self.current_id(key) else {
            return;
        };
        let window = self.hub.window(id);
        if !window.is_float()
            || window.is_minimized()
            || !window
                .workspace()
                .is_some_and(|workspace| self.hub.visible_workspaces().contains(&workspace))
        {
            return;
        }
        let monitor = self
            .monitor_registry
            .find_closest_monitor(frame.to_dimension())
            .map(|(monitor, _)| monitor)
            .unwrap_or_else(|| self.hub.primary_monitor());
        if frame.is_empty() || self.monitor_registry.is_borderless_fullscreen_at(frame) {
            return;
        }
        self.hub.update_float_rect(id, frame, monitor);
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.frame = Some(frame);
        }
    }

    pub(in crate::platform::macos) fn fullscreen_changed(
        &mut self,
        key: CGWindowID,
        fullscreen: Option<FullscreenKind>,
        frame: Option<PixelRect>,
    ) {
        match fullscreen {
            Some(FullscreenKind::Borderless)
                if !frame.is_some_and(|frame| {
                    self.monitor_registry.is_borderless_fullscreen_at(frame)
                }) =>
            {
                return;
            }
            None if frame.is_none_or(|frame| {
                frame.is_empty() || self.monitor_registry.is_borderless_fullscreen_at(frame)
            }) =>
            {
                return;
            }
            _ => {}
        }
        let Some(id) = self.current_id(key) else {
            if let Some(discovery) = self.discoveries.get_mut(&key) {
                discovery.fullscreen = fullscreen;
            }
            return;
        };
        let window = self.hub.window(id);
        if window.is_minimized() || window.workspace().is_none() {
            return;
        }
        let previous = self
            .discoveries
            .get(&key)
            .and_then(|discovery| discovery.fullscreen);
        if fullscreen.is_some() {
            self.hub
                .set_fullscreen(id, WindowRestrictions::ProtectFullscreen);
        } else if previous.is_some() && window.is_fullscreen() {
            self.hub.unset_fullscreen(id);
        }
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.fullscreen = fullscreen;
        }
    }

    pub(in crate::platform::macos) fn apply_layout_file(&mut self, path: &str) {
        let layouts = match PreferredLayouts::load(path) {
            Ok(layouts) => layouts,
            Err(error) => {
                tracing::error!("Apply layout failed: {error:#}");
                return;
            }
        };
        self.hub.apply_preferred_layouts(layouts);
        tracing::info!(%path, "Layout applied");
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        if monitors.is_empty() {
            return;
        }
        self.monitors = monitors;
        self.reconcile_monitors();
        self.displays_changed_at = Instant::now();
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn mirror_clicked(&mut self, key: CGWindowID) {
        let Some(id) = self.current_id(key) else {
            return;
        };
        if self.hub.window(id).is_minimized() {
            self.sender
                .send(HubMessage::Command(WindowCommand::Unminimize(key)));
            return;
        }
        self.hub.set_focus(id);
        self.focus_request = Some(FocusTarget::Window(key));
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn tab_clicked(&mut self, container: ContainerId, index: usize) {
        // A click can outlive the scene that created its tab bar.
        let current = self.hub.get_visible_placements();
        if current
            .monitors
            .iter()
            .any(|monitor| match &monitor.layout {
                crate::core::MonitorLayout::Normal { containers, .. } => containers
                    .iter()
                    .any(|placement| placement.id == container && index < placement.titles.len()),
                crate::core::MonitorLayout::Fullscreen(_) => false,
            })
        {
            self.hub.focus_tab_index(container, index);
            self.flush_layout();
        }
    }

    pub(in crate::platform::macos) fn save_layout_file(&mut self, path: &std::path::Path) {
        if let Err(error) = self.hub.save_layout(path) {
            tracing::error!("Save layout failed: {error:#}");
        }
    }

    pub(in crate::platform::macos) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(in crate::platform::macos) fn query_monitors_json(&self) -> String {
        serde_json::to_string(&self.hub.query_monitors())
            .expect("MonitorDetails is infallibly serializable")
    }

    pub(in crate::platform::macos) fn query_minimized_windows_json(&self) -> String {
        let entries: Vec<MinimizedWindow> = self
            .hub
            .minimized_window_entries()
            .into_iter()
            .map(|entry| MinimizedWindow {
                id: entry.id,
                title: entry.title,
                app_name: entry.app_name,
                bundle_id: entry.bundle_id,
                executable_path: entry.executable_path,
            })
            .collect();
        serde_json::to_string(&entries).expect("MinimizedWindow is infallibly serializable")
    }

    pub(in crate::platform::macos) fn run_binding(
        &mut self,
        keymap: &str,
        keystroke: &Keystroke,
        signal: &LoopSignal,
    ) {
        let mut effects = MacOsPlatformEffects {
            native_keys: &self.native_keys,
            minimized: self
                .hub
                .minimized_window_entries()
                .into_iter()
                .map(|entry| entry.id)
                .collect(),
            sender: self.sender.as_ref(),
            signal,
            env: &self.env,
        };
        self.hub
            .run_binding(keymap, keystroke, &mut effects, &mut self.keymap_publisher);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn handle_actions(
        &mut self,
        actions: &Actions,
        signal: &LoopSignal,
    ) {
        let mut effects = MacOsPlatformEffects {
            native_keys: &self.native_keys,
            minimized: self
                .hub
                .minimized_window_entries()
                .into_iter()
                .map(|entry| entry.id)
                .collect(),
            sender: self.sender.as_ref(),
            signal,
            env: &self.env,
        };
        self.hub
            .handle_actions(actions, &mut effects, &mut self.keymap_publisher);
        self.flush_layout();
    }

    pub(in crate::platform::macos) fn reload(&mut self) -> Option<Box<Config>> {
        let config = self.hub.reload_config(&mut self.keymap_publisher)?;
        self.env = config.env.clone();
        self.sender
            .send(HubMessage::AppearanceChanged(config.appearance.clone()));
        let mut ignored: Vec<_> = self
            .discoveries
            .keys()
            .copied()
            .filter(|key| self.current_id(*key).is_none())
            .collect();
        ignored.sort_unstable();
        for key in ignored {
            self.admit_discovery(key);
        }
        tracing::info!("Config reloaded");
        self.flush_layout();
        Some(config)
    }
}
