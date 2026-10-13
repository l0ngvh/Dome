pub(super) mod events;
mod layout;
pub(super) mod monitor;
pub(super) mod window;
pub(super) use monitor::{MonitorInfo, QueryDisplay, Win32Display};
pub(super) use window::WindowsMetadata;

use super::external::HwndId;
use super::taskbar::{ManageTaskbar, TaskbarRestoreGuard};
use super::window_server::{DiscoveredWindow, FullscreenKind};
use crate::action::{Actions, MinimizedWindow, Query};
use crate::config::{Config, Keystroke, LuaRuntime, PlatformEffects, PreferredLayouts};
use crate::core::{
    ContainerId, Hub, LimitObservation, Physical, PixelRect, TilingConfig, WindowId,
    WindowRestrictions,
};
use crate::platform::keymap::KeymapPublisher;
use events::{FocusTarget, HubMessage, SceneSender, WindowCommand};
use monitor::MonitorRegistry;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostQuitMessage, PostThreadMessageW, WM_QUIT};

struct WindowsPlatformEffects<'a> {
    native_keys: &'a HashMap<WindowId, HwndId>,
    sender: &'a mut dyn SceneSender,
    main_thread_id: u32,
    env: &'a HashMap<String, String>,
}
impl PlatformEffects for WindowsPlatformEffects<'_> {
    fn close(&mut self, id: WindowId) {
        if let Some(&key) = self.native_keys.get(&id) {
            self.sender
                .send(HubMessage::Command(WindowCommand::Close(key)));
        }
    }
    fn unminimize(&mut self, id: WindowId) {
        if let Some(&key) = self.native_keys.get(&id) {
            self.sender
                .send(HubMessage::Command(WindowCommand::Unminimize(key)));
        }
    }
    fn execute(&mut self, command: &str) {
        if let Err(error) = super::spawn::spawn(command, self.env) {
            tracing::warn!(%command, %error, "Failed to execute");
        }
    }
    fn exit(&mut self) {
        unsafe {
            PostThreadMessageW(self.main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok();
            PostQuitMessage(0);
        }
    }
}

pub(super) enum HubEvent {
    WindowsDiscovered(Vec<DiscoveredWindow>),
    WindowGone(HwndId),
    WindowMinimized(HwndId),
    WindowRestored(HwndId),
    FocusReported(HwndId),
    TitleChanged {
        key: HwndId,
        title: String,
    },
    SizeConstraint {
        key: HwndId,
        limits: LimitObservation,
    },
    FloatMoved {
        key: HwndId,
        frame: PixelRect<Physical>,
        monitor: isize,
    },
    FullscreenChanged {
        key: HwndId,
        fullscreen: Option<FullscreenKind>,
    },
    Action(Actions),
    Query {
        query: Query,
        sender: std::sync::mpsc::SyncSender<String>,
    },
    RunBinding {
        keymap: String,
        keystroke: Keystroke,
    },
    ReloadConfig,
    ApplyLayout(String),
    SaveLayout(String),
    TabClicked(ContainerId, usize),
    DpiChanged,
    WorkAreaChanged,
    DisplayChanged,
    Shutdown,
}

pub(super) struct Dome {
    hub: Hub,
    window_ids: HashMap<HwndId, WindowId>,
    native_keys: HashMap<WindowId, HwndId>,
    discoveries: HashMap<HwndId, DiscoveredWindow>,
    monitors: MonitorRegistry,
    displayed_windows: HashSet<WindowId>,
    taskbar: TaskbarRestoreGuard,
    display: Box<dyn QueryDisplay>,
    window: Box<dyn SceneSender>,
    last_focus_target: Option<FocusTarget>,
    focus_request: Option<FocusTarget>,
    keymap_publisher: KeymapPublisher,
    env: HashMap<String, String>,
}
impl Drop for Dome {
    fn drop(&mut self) {
        self.window.send(HubMessage::Shutdown);
    }
}
impl Dome {
    pub(super) fn handle_report(&mut self, event: HubEvent) -> Result<(), HubEvent> {
        match event {
            HubEvent::WindowsDiscovered(windows) => self.windows_discovered(windows),
            HubEvent::WindowGone(key) => self.window_destroyed(key),
            HubEvent::WindowMinimized(key) => self.window_minimized(key),
            HubEvent::WindowRestored(key) => self.window_restored(key),
            HubEvent::FocusReported(key) => self.handle_focus(key),
            HubEvent::TitleChanged { key, title } => self.title_changed(key, title),
            HubEvent::SizeConstraint { key, limits } => self.set_constraints_for(key, limits),
            HubEvent::FloatMoved {
                key,
                frame,
                monitor,
            } => self.float_moved(key, frame, monitor),
            HubEvent::FullscreenChanged { key, fullscreen } => {
                self.fullscreen_changed(key, fullscreen)
            }
            other => return Err(other),
        }
        self.apply_layout();
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "platform collaborators and domain configuration"
    )]
    pub(super) fn new(
        tiling: TilingConfig,
        workspace_overrides: PreferredLayouts,
        taskbar: Rc<dyn ManageTaskbar>,
        display: Box<dyn QueryDisplay>,
        mut window: Box<dyn SceneSender>,
        runtime: LuaRuntime,
        keymap_publisher: KeymapPublisher,
        env: HashMap<String, String>,
    ) -> anyhow::Result<Self> {
        let monitors = display.get_all_monitors()?;
        anyhow::ensure!(!monitors.is_empty(), "No monitors detected");
        let primary = monitors
            .iter()
            .find(|m| m.is_primary)
            .unwrap_or(&monitors[0]);
        let mut hub = Hub::new(primary.into(), tiling, workspace_overrides, runtime);
        let mut registry = MonitorRegistry::new();
        registry.insert(hub.primary_monitor(), primary);
        for monitor in &monitors {
            if monitor.handle != primary.handle {
                let id = hub.add_monitor(monitor.into());
                registry.insert(id, monitor);
            }
        }
        window.send(HubMessage::DisplayRefreshed(monitors));
        Ok(Self {
            hub,
            window_ids: HashMap::new(),
            native_keys: HashMap::new(),
            discoveries: HashMap::new(),
            monitors: registry,
            displayed_windows: HashSet::new(),
            taskbar: TaskbarRestoreGuard::new(taskbar),
            display,
            window,
            last_focus_target: None,
            focus_request: None,
            keymap_publisher,
            env,
        })
    }
    fn current_id(&self, key: HwndId) -> Option<WindowId> {
        self.window_ids.get(&key).copied()
    }
    pub(super) fn windows_discovered(&mut self, windows: Vec<DiscoveredWindow>) {
        for discovery in windows {
            let key = discovery.key;
            self.discoveries.insert(key, discovery);
            self.admit_discovery(key);
        }
    }
    fn admit_discovery(&mut self, key: HwndId) {
        if self.current_id(key).is_some() {
            return;
        }
        let Some(discovery) = self.discoveries.get(&key) else {
            return;
        };
        let restrictions = match discovery.fullscreen {
            Some(FullscreenKind::Exclusive) => WindowRestrictions::BlockAll,
            Some(FullscreenKind::Borderless) => WindowRestrictions::ProtectFullscreen,
            None => WindowRestrictions::None,
        };
        let Some(id) = self.hub.insert_window(
            Box::new(discovery.metadata.clone()),
            discovery.frame,
            restrictions,
        ) else {
            tracing::trace!(%key, process_id = discovery.process_id, "ignored by rule");
            return;
        };
        if let Some(limits) = discovery.limits {
            self.hub.set_window_constraint(id, limits);
        }
        tracing::info!(%id, %key, process_id = discovery.process_id, metadata = %discovery.metadata, "New window");
        self.window_ids.insert(key, id);
        self.native_keys.insert(id, key);
        self.taskbar.track(key);
    }
    pub(super) fn window_destroyed(&mut self, key: HwndId) {
        self.discoveries.remove(&key);
        self.taskbar.remove(key);
        if let Some(id) = self.window_ids.remove(&key) {
            self.native_keys.remove(&id);
            self.displayed_windows.remove(&id);
            self.hub.delete_window(id);
        }
    }
    pub(super) fn window_minimized(&mut self, key: HwndId) {
        if let Some(id) = self.current_id(key)
            && !self.hub.window(id).is_minimized()
        {
            self.hub.minimize_window(id);
        }
    }
    pub(super) fn window_restored(&mut self, key: HwndId) {
        if let Some(id) = self.current_id(key)
            && self.hub.window(id).is_minimized()
        {
            self.hub.unminimize_window(id);
        }
    }
    pub(super) fn handle_focus(&mut self, key: HwndId) {
        self.focus_request = None;
        self.window_restored(key);
        if let Some(id) = self.current_id(key)
            && self.hub.window(id).workspace().is_some()
        {
            self.hub.set_focus(id);
        }
        self.last_focus_target = Some(self.current_focus_target());
    }
    pub(super) fn title_changed(&mut self, key: HwndId, title: String) {
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.metadata.title = Some(title.clone());
        }
        if let Some(id) = self.current_id(key) {
            self.hub.set_window_title(id, title);
        }
    }
    pub(super) fn set_constraints_for(&mut self, key: HwndId, limits: LimitObservation) {
        if let Some(id) = self.current_id(key)
            && !self.hub.window(id).is_minimized()
            && !self.hub.window(id).is_fullscreen()
            && self.hub.window(id).workspace().is_some()
        {
            self.hub.set_window_constraint(id, limits);
        }
    }
    pub(super) fn float_moved(&mut self, key: HwndId, frame: PixelRect<Physical>, monitor: isize) {
        let Some(id) = self.current_id(key) else {
            return;
        };
        let Some(monitor_id) = self.monitors.id_for_handle(monitor) else {
            return;
        };
        if frame.is_empty() || self.monitors.is_borderless_fullscreen_at(frame, monitor) {
            return;
        }
        let window = self.hub.window(id);
        if window.is_float()
            && !window.is_minimized()
            && window
                .workspace()
                .is_some_and(|workspace| self.hub.visible_workspaces().contains(&workspace))
        {
            self.hub.update_float_rect(id, frame, monitor_id);
        }
    }
    pub(super) fn fullscreen_changed(&mut self, key: HwndId, fullscreen: Option<FullscreenKind>) {
        let Some(id) = self.current_id(key) else {
            return;
        };
        let window = self.hub.window(id);
        if window.is_minimized() || window.workspace().is_none() {
            return;
        }
        let previous = self.discoveries.get(&key).and_then(|d| d.fullscreen);
        match fullscreen {
            Some(FullscreenKind::Exclusive) => {
                self.hub.set_fullscreen(id, WindowRestrictions::BlockAll)
            }
            Some(FullscreenKind::Borderless) => self
                .hub
                .set_fullscreen(id, WindowRestrictions::ProtectFullscreen),
            None if previous.is_some() => self.hub.unset_fullscreen(id),
            None => return,
        }
        if let Some(discovery) = self.discoveries.get_mut(&key) {
            discovery.fullscreen = fullscreen;
        }
    }
    pub(super) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        if monitors.is_empty() {
            return;
        }
        self.monitors.reconcile(&mut self.hub, &monitors);
        self.window.send(HubMessage::DisplayRefreshed(monitors));
    }
    pub(super) fn handle_display_change(&mut self) {
        match self.display.get_all_monitors() {
            Ok(monitors) => self.monitors_changed(monitors),
            Err(error) => tracing::warn!(%error, "Failed to enumerate monitors"),
        }
    }
    pub(super) fn tab_clicked(&mut self, id: ContainerId, index: usize) {
        self.hub.focus_tab_index(id, index);
        self.apply_layout();
    }
    pub(super) fn apply_layout_file(&mut self, path: &str) {
        match PreferredLayouts::load(path) {
            Ok(layout) => {
                self.hub.apply_preferred_layouts(layout);
                self.apply_layout();
            }
            Err(error) => tracing::error!(%error, "Apply layout failed"),
        }
    }
    pub(super) fn save_layout_file(&mut self, path: &std::path::Path) {
        if let Err(error) = self.hub.save_layout(path) {
            tracing::error!(%error, "Save layout failed");
        }
    }
    pub(super) fn query_workspaces_json(&self) -> String {
        serde_json::to_string(&self.hub.query_workspaces())
            .expect("WorkspaceInfo is infallibly serializable")
    }

    pub(super) fn query_monitors_json(&self) -> String {
        serde_json::to_string(&self.hub.query_monitors())
            .expect("MonitorDetails is infallibly serializable")
    }

    pub(super) fn query_minimized_windows_json(&self) -> String {
        let entries: Vec<MinimizedWindow> = self
            .hub
            .minimized_window_entries()
            .into_iter()
            .map(|e| MinimizedWindow {
                id: e.id,
                title: e.title,
                app_name: e.app_name,
                bundle_id: e.bundle_id,
                executable_path: e.executable_path,
            })
            .collect();
        serde_json::to_string(&entries).expect("MinimizedWindow is infallibly serializable")
    }

    pub(super) fn run_binding(&mut self, keymap: &str, keystroke: &Keystroke, main_thread_id: u32) {
        {
            let mut effects = WindowsPlatformEffects {
                native_keys: &self.native_keys,
                sender: self.window.as_mut(),
                main_thread_id,
                env: &self.env,
            };
            self.hub
                .run_binding(keymap, keystroke, &mut effects, &mut self.keymap_publisher);
        }
        self.apply_layout();
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn handle_actions(&mut self, actions: &Actions, main_thread_id: u32) {
        let mut effects = WindowsPlatformEffects {
            native_keys: &self.native_keys,
            sender: self.window.as_mut(),
            main_thread_id,
            env: &self.env,
        };
        self.hub
            .handle_actions(actions, &mut effects, &mut self.keymap_publisher);
        self.apply_layout();
    }

    pub(super) fn reload(&mut self) -> Option<Box<Config>> {
        let config = self.hub.reload_config(&mut self.keymap_publisher)?;
        self.env = config.env.clone();
        self.window
            .send(HubMessage::AppearanceChanged(config.appearance.clone()));
        let ignored: Vec<_> = self
            .discoveries
            .keys()
            .copied()
            .filter(|key| self.current_id(*key).is_none())
            .collect();
        for key in ignored {
            self.admit_discovery(key);
        }
        self.apply_layout();
        Some(config)
    }
    #[cfg(test)]
    pub(super) fn window_id_for(&self, key: HwndId) -> Option<WindowId> {
        self.current_id(key)
    }
}
// Fallback display string derived from the executable name. Prefer
// FileDescription from version info when available.
pub(super) fn display_from_process(process: &str) -> String {
    process.strip_suffix(".exe").unwrap_or(process).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_from_process_strips_exe() {
        assert_eq!(display_from_process("chrome.exe"), "chrome");
        assert_eq!(display_from_process("notepad"), "notepad");
    }
}
