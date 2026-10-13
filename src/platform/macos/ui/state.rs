use std::ops::ControlFlow;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use objc2_core_graphics::CGWindowID;

use super::CaptureMessage;
use super::internal_windows::{
    ContainerShow, FloatShow, InternalWindows, MonitorTilingData, TilingWindowShow,
};
use super::recovery::Recovery;
use super::registry::WindowRegistry;
use super::window::{self, PlacementTarget, WindowState};
use crate::action::WorkspaceInfo;
use crate::core::{Length, MonitorId};
use crate::platform::macos::dome::events::{
    FocusTarget, FullscreenKind, HubEvent, HubMessage, MonitorScene, Scene, WindowCommand,
    WindowPlacement,
};
use crate::platform::macos::focus::SharedFocus;
use crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa;
use crate::platform::macos::window_server::{
    ApplicationLifetime, ExistingWindow, Observation, PositionReadOrigin, WindowServer,
};

pub(in crate::platform::macos) struct UiState {
    intake: Option<mpsc::Receiver<HubMessage>>,
    registry: WindowRegistry,
    pub(super) hub: calloop::channel::Sender<HubEvent>,
    internal: Box<dyn InternalWindows>,
    pub(super) focus: Arc<SharedFocus>,
    capture_intake: mpsc::Receiver<CaptureMessage>,
    overlay_focus_request: Option<MonitorId>,
    last_focus_request: Option<FocusTarget>,
    monitors: Vec<MonitorScene>,
    displays_changed_at: Option<Instant>,
    recovery: Recovery,
    recovery_started: bool,
    pub(super) workspaces: Vec<WorkspaceInfo>,
}

impl UiState {
    pub(in crate::platform::macos) fn new(
        intake: mpsc::Receiver<HubMessage>,
        hub: calloop::channel::Sender<HubEvent>,
        internal: Box<dyn InternalWindows>,
        focus: Arc<SharedFocus>,
        capture_intake: mpsc::Receiver<CaptureMessage>,
    ) -> Self {
        Self {
            intake: Some(intake),
            registry: WindowRegistry::default(),
            hub,
            internal,
            focus,
            capture_intake,
            overlay_focus_request: None,
            last_focus_request: None,
            monitors: Vec::new(),
            displays_changed_at: None,
            recovery: Recovery::default(),
            recovery_started: false,
            workspaces: Vec::new(),
        }
    }

    pub(super) fn observe(
        &mut self,
        windows: &mut dyn WindowServer,
        observation: Observation,
        now: Instant,
    ) {
        match observation {
            Observation::AppWindows {
                lifetime,
                discovered,
                existing,
                removed,
            } => {
                for key in removed {
                    self.remove(&lifetime, key);
                }
                for discovery in &discovered {
                    self.registry.discover(&lifetime, discovery);
                }
                if !discovered.is_empty() {
                    self.report(HubEvent::WindowsDiscovered(discovered));
                }
                for read in existing {
                    self.observe_existing(
                        windows,
                        &lifetime,
                        read,
                        PositionReadOrigin::Periodic,
                        now,
                    );
                }
            }
            Observation::Settled {
                lifetime,
                origin,
                windows: reads,
            } => {
                for read in reads {
                    self.observe_existing(windows, &lifetime, read, origin, now);
                }
            }
            Observation::Space {
                lifetime,
                windows: reads,
            } => {
                for read in reads {
                    self.observe_existing(
                        windows,
                        &lifetime,
                        read,
                        PositionReadOrigin::Periodic,
                        now,
                    );
                }
            }
            Observation::FocusReported { window, .. } => {
                self.report(HubEvent::FocusReported(window.unwrap_or(0)))
            }
            Observation::TitleChanged {
                lifetime,
                key,
                title,
            } => {
                if let Some(entry) = self.registry.windows.get_mut(&key)
                    && Arc::ptr_eq(&entry.lifetime, &lifetime)
                {
                    entry.discovery.metadata.title = Some(title.clone());
                    self.report(HubEvent::TitleChanged { key, title });
                }
            }
            Observation::AppHidden { lifetime } => {
                let mut gone = Vec::new();
                for (&key, entry) in &mut self.registry.windows {
                    if Arc::ptr_eq(&entry.lifetime, &lifetime) {
                        entry.hidden = true;
                        entry.admitted = None;
                        entry.target = None;
                        entry.state = WindowState::Unplaced;
                        entry.awaiting_response = false;
                        gone.push(key);
                    }
                }
                for key in gone {
                    self.report(HubEvent::WindowGone(key));
                }
            }
            Observation::AppGone { lifetime } => {
                let keys: Vec<_> = self
                    .registry
                    .windows
                    .iter()
                    .filter(|(_, entry)| Arc::ptr_eq(&entry.lifetime, &lifetime))
                    .map(|(&key, _)| key)
                    .collect();
                for key in keys {
                    self.remove(&lifetime, key);
                }
            }
        }
    }

    fn remove(&mut self, lifetime: &Arc<ApplicationLifetime>, key: CGWindowID) {
        if self
            .registry
            .get(key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.lifetime, lifetime))
        {
            self.registry.windows.remove(&key);
            self.recovery.untrack(key);
            self.report(HubEvent::WindowGone(key));
        }
    }

    fn observe_existing(
        &mut self,
        windows: &mut dyn WindowServer,
        lifetime: &Arc<ApplicationLifetime>,
        read: ExistingWindow,
        origin: PositionReadOrigin,
        now: Instant,
    ) {
        let Some(entry) = self.registry.windows.get_mut(&read.key) else {
            return;
        };
        if !Arc::ptr_eq(&entry.lifetime, lifetime) || window::is_stale(entry, origin) {
            return;
        }
        let mut reports = Vec::new();
        let key = read.key;
        entry.corner_radius = read.corner_radius;
        if entry.hidden {
            entry.hidden = false;
            let mut discovery = entry.discovery.clone();
            discovery.frame = read.frame;
            discovery.corner_radius = read.corner_radius;
            discovery.minimized = read.minimized;
            discovery.fullscreen = read.fullscreen;
            reports.push(HubEvent::WindowsDiscovered(vec![discovery]));
        }
        let dome_minimized = matches!(entry.state, WindowState::BorderlessMinimized { .. });
        if read.minimized != entry.minimized {
            entry.minimized = read.minimized;
            if !dome_minimized {
                reports.push(if read.minimized {
                    HubEvent::WindowMinimized(key)
                } else {
                    HubEvent::WindowRestored(key)
                });
                entry.awaiting_response = true;
            }
        }
        if !read.minimized {
            if read.fullscreen == Some(FullscreenKind::Native) {
                if entry.fullscreen != read.fullscreen {
                    entry.fullscreen = read.fullscreen;
                    entry.awaiting_response = true;
                    reports.push(HubEvent::FullscreenChanged {
                        key,
                        fullscreen: read.fullscreen,
                        frame: None,
                    });
                }
                entry.state = WindowState::NativeFullscreen;
            } else if let Some(frame) = read.frame {
                if entry.admitted.is_some() {
                    self.recovery.track(
                        key,
                        entry.actual.or(Some(frame)),
                        self.monitors
                            .iter()
                            .find(|m| m.is_primary)
                            .map(|m| m.system_work_area),
                    );
                    window::observe_frame(
                        entry,
                        key,
                        frame,
                        origin,
                        window::PlacementContext {
                            monitors: &self.monitors,
                            display_changed_at: self.displays_changed_at,
                            now,
                        },
                        windows,
                        &mut reports,
                    );
                } else {
                    entry.actual = Some(frame);
                    entry.fullscreen = read.fullscreen;
                }
            }
        }
        for report in reports {
            self.report(report);
        }
    }

    fn on_scene_focus(&mut self, request: Option<FocusTarget>) {
        if request != self.last_focus_request || request.is_none() {
            self.overlay_focus_request = None;
            match request {
                None => self.focus.clear_pending(),
                Some(FocusTarget::Window(key)) => self.focus.request(key),
                Some(FocusTarget::Monitor(monitor)) => {
                    self.focus.clear_external();
                    self.overlay_focus_request = Some(monitor);
                }
            }
        }
        self.last_focus_request = request;
    }

    pub(super) fn drain_intake(&mut self) -> ControlFlow<(), (Option<Scene>, Vec<WindowCommand>)> {
        let mut scene = None;
        let mut commands = Vec::new();
        let mut appearances = Vec::new();
        loop {
            let message = self
                .intake
                .as_ref()
                .and_then(|intake| intake.try_recv().ok());
            let Some(message) = message else { break };
            match message {
                HubMessage::Scene(next) => {
                    self.on_scene_focus(next.focus_request);
                    scene = Some(next);
                }
                HubMessage::Command(command) => commands.push(command),
                HubMessage::AppearanceChanged(appearance) => appearances.push(appearance),
                HubMessage::Shutdown => return ControlFlow::Break(()),
            }
        }
        for appearance in appearances {
            self.internal.set_appearance(&appearance);
        }
        ControlFlow::Continue((scene, commands))
    }

    pub(super) fn apply_placements(
        &mut self,
        windows: &mut dyn WindowServer,
        scene: &Scene,
        now: Instant,
    ) {
        self.monitors = scene.monitors.clone();
        self.displays_changed_at = Some(scene.displays_changed_at);
        self.workspaces = scene.workspaces.clone();
        let hidden =
            window::hidden_position(&self.monitors).map(|(x, y)| PlacementTarget::Hidden(x, y));
        let primary = self
            .monitors
            .iter()
            .find(|m| m.is_primary)
            .map(|m| m.system_work_area);
        for (&key, entry) in &mut self.registry.windows {
            let Some(expected) = scene.windows.get(&key).filter(|_| !entry.hidden) else {
                entry.admitted = None;
                entry.target = None;
                entry.state = WindowState::Unplaced;
                entry.awaiting_response = false;
                continue;
            };
            entry.admitted = Some(expected.id);
            let target = match expected.placement {
                WindowPlacement::Tiling { placement, .. } => {
                    if placement.visible_content_box.is_empty() {
                        hidden
                    } else {
                        Some(PlacementTarget::Tiling(placement.visible_content_box))
                    }
                }
                WindowPlacement::Float { placement, .. } => {
                    if placement.content_box.is_empty() || scene.focused_window != Some(key) {
                        hidden
                    } else {
                        Some(PlacementTarget::Float(placement.content_box))
                    }
                }
                WindowPlacement::Fullscreen { monitor } => self
                    .monitors
                    .iter()
                    .find(|m| m.id == monitor)
                    .map(|m| PlacementTarget::Fullscreen(m.work_area)),
                WindowPlacement::Hidden => hidden,
                WindowPlacement::Minimized => Some(PlacementTarget::Minimized),
            };
            if let Some(target) = target {
                if entry.target != Some(target) {
                    self.recovery.track(key, entry.actual, primary);
                }
                window::apply_target(entry, key, target, &self.monitors, now, windows);
            }
        }
    }

    pub(super) fn apply_command(&mut self, windows: &mut dyn WindowServer, command: WindowCommand) {
        let key = match command {
            WindowCommand::Close(key) | WindowCommand::Unminimize(key) => key,
        };
        let Some(entry) = self
            .registry
            .get(key)
            .filter(|entry| entry.admitted.is_some())
        else {
            return;
        };
        self.recovery.track(
            key,
            entry.actual,
            self.monitors
                .iter()
                .find(|m| m.is_primary)
                .map(|m| m.system_work_area),
        );
        match command {
            WindowCommand::Close(key) => windows.close(key),
            WindowCommand::Unminimize(key) => windows.unminimize(key),
        }
    }

    pub(super) fn show_internal_windows(&mut self, scene: &Scene) {
        self.internal.set_monitors(&scene.monitors);
        let height = Length::new(
            scene
                .monitors
                .iter()
                .find(|m| m.is_primary)
                .map_or(0.0, |m| m.full_height),
        );
        let mut floats = Vec::new();
        let mut containers = Vec::new();
        for monitor in &scene.monitors {
            let mut tiling = Vec::new();
            for (&key, expected) in &scene.windows {
                let Some(entry) = self
                    .registry
                    .get(key)
                    .filter(|entry| entry.admitted.is_some())
                else {
                    continue;
                };
                match expected.placement {
                    WindowPlacement::Tiling {
                        monitor: id,
                        placement,
                    } if id == monitor.id && !placement.visible_content_box.is_empty() => {
                        tiling.push(TilingWindowShow {
                            placement,
                            corner_radius: entry.corner_radius,
                        });
                    }
                    WindowPlacement::Float {
                        monitor: id,
                        placement,
                    } if id == monitor.id && !placement.content_box.is_empty() => {
                        floats.push(FloatShow {
                            cg_id: key,
                            placement,
                            cocoa_frame: dimension_to_ns_rect_cocoa(
                                height,
                                placement.border_box.to_dimension(),
                            ),
                            scale: monitor.scale,
                            border_thickness: monitor.border_thickness,
                            content_dim: placement.content_box.to_dimension(),
                            corner_radius: entry.corner_radius,
                        });
                    }
                    _ => {}
                }
            }
            let monitor_containers: Vec<_> = monitor
                .containers
                .iter()
                .map(|placement| {
                    let tab_bar_dim = placement.tab_bar_band.to_dimension();
                    ContainerShow {
                        placement: placement.clone(),
                        tab_bar_dim,
                        tab_bar_cocoa_frame: dimension_to_ns_rect_cocoa(height, tab_bar_dim),
                        scale: monitor.scale,
                        border_thickness: monitor.border_thickness,
                    }
                })
                .collect();
            self.internal.show_tiling(&MonitorTilingData {
                monitor_id: monitor.id,
                monitor_dim: monitor.work_area.to_dimension(),
                cocoa_frame: dimension_to_ns_rect_cocoa(height, monitor.work_area.to_dimension()),
                scale: monitor.scale,
                border_thickness: monitor.border_thickness,
                windows: tiling,
                containers: monitor_containers.clone(),
            });
            containers.extend(monitor_containers);
        }
        self.internal.show_floats(&floats, scene.focused_window);
        self.internal.show_tab_bars(&containers);
    }

    pub(super) fn apply_overlay_focus(&mut self) {
        if let Some(monitor) = self.overlay_focus_request.take() {
            self.focus.clear_external();
            self.internal.focus_tiling_overlay(monitor);
        }
    }

    pub(super) fn process_capture(&mut self) {
        self.internal
            .apply_capture_messages(self.capture_intake.try_iter().collect());
    }

    pub(super) fn recover(&mut self, windows: &mut dyn WindowServer) {
        if self.recovery_started {
            return;
        }
        self.recovery_started = true;
        self.intake.take();
        self.overlay_focus_request = None;
        self.focus.clear_external();
        windows.shutdown(self.recovery.take());
        self.report(HubEvent::Shutdown);
    }

    fn report(&self, event: HubEvent) {
        self.hub.send(event).ok();
    }
}
