use std::ops::ControlFlow;
use std::sync::mpsc;
use std::time::Instant;

use super::focus::FocusState;
use super::recovery::Recovery;
use super::registry::WindowRegistry;
use super::window::{self, PlacementTarget};
use super::{FloatShow, InternalWindow, InternalWindows};
use crate::action::WorkspaceInfo;
use crate::core::MonitorId;
use crate::platform::windows::HubSender;
use crate::platform::windows::dome::HubEvent;
use crate::platform::windows::dome::events::{
    HubMessage, MonitorScene, Scene, TilingWindowShow, WindowCommand,
};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::external::ZOrder;
use crate::platform::windows::handle::OFFSCREEN_POS;
use crate::platform::windows::window_server::{
    FullscreenKind, Observation, SettleRead, SizeLimitRead, WindowServer,
};

pub(in crate::platform::windows) struct UiState {
    intake: Option<mpsc::Receiver<HubMessage>>,
    registry: WindowRegistry,
    pub(super) focus: FocusState,
    internal: Box<dyn InternalWindows>,
    hub: HubSender,
    recovery: Recovery,
    recovery_started: bool,
    monitors: Vec<MonitorScene>,
    observations_before_monitor_policy: Option<Vec<Observation>>,
    previous_scene: Option<Scene>,
    overlay_focus_request: Option<MonitorId>,
    exclusive: Option<HwndId>,
    pub(in crate::platform::windows) workspaces: Vec<WorkspaceInfo>,
}

impl UiState {
    pub(in crate::platform::windows) fn new(
        intake: mpsc::Receiver<HubMessage>,
        hub: HubSender,
        internal: Box<dyn InternalWindows>,
    ) -> Self {
        Self {
            intake: Some(intake),
            registry: WindowRegistry::default(),
            focus: FocusState::default(),
            internal,
            hub,
            recovery: Recovery::default(),
            recovery_started: false,
            monitors: Vec::new(),
            observations_before_monitor_policy: Some(Vec::new()),
            previous_scene: None,
            overlay_focus_request: None,
            exclusive: None,
            workspaces: Vec::new(),
        }
    }

    pub(in crate::platform::windows) fn observe(
        &mut self,
        windows: &mut dyn WindowServer,
        observation: Observation,
        now: Instant,
    ) {
        if self.recovery_started {
            return;
        }
        // Hooks can deliver discovery before the domain supplies monitor policy.
        // Retain the whole observation sequence so later facts cannot overtake discovery.
        if let Some(pending) = &mut self.observations_before_monitor_policy {
            pending.push(observation);
            return;
        }
        match observation {
            Observation::Discovered(mut discovery) => {
                discovery.fullscreen = if self.exclusive == Some(discovery.key) {
                    Some(FullscreenKind::Exclusive)
                } else if window::borderless(discovery.frame, discovery.monitor, &self.monitors) {
                    Some(FullscreenKind::Borderless)
                } else {
                    None
                };
                self.registry.discover(&discovery);
                self.hub.send(HubEvent::WindowsDiscovered(vec![discovery]));
            }
            Observation::Gone(key) => {
                self.registry.windows.remove(&key);
                self.recovery.remove(key);
                if self.focus.pending() == Some(key) {
                    self.focus.clear();
                }
                self.hub.send(HubEvent::WindowGone(key));
            }
            Observation::Minimized(key) => {
                if let Some(entry) = self.registry.windows.get_mut(&key)
                    && !entry.dome_minimized
                {
                    entry.minimized = true;
                    self.hub.send(HubEvent::WindowMinimized(key));
                }
            }
            Observation::Foreground { window } => self.hub.send(HubEvent::FocusReported(window)),
            Observation::ExclusiveFullscreen(key) => {
                self.exclusive = key;
                for (&window, entry) in &mut self.registry.windows {
                    if Some(window) == key && entry.fullscreen != Some(FullscreenKind::Exclusive) {
                        entry.fullscreen = Some(FullscreenKind::Exclusive);
                        entry.awaiting_response = true;
                        self.hub.send(HubEvent::FullscreenChanged {
                            key: window,
                            fullscreen: entry.fullscreen,
                        });
                    } else if Some(window) != key
                        && entry.fullscreen == Some(FullscreenKind::Exclusive)
                    {
                        entry.fullscreen = None;
                        entry.awaiting_response = true;
                        self.hub.send(HubEvent::FullscreenChanged {
                            key: window,
                            fullscreen: None,
                        });
                        windows.request_size_limits(vec![SizeLimitRead { window }]);
                    }
                }
            }
            Observation::Title {
                window,
                title: Some(title),
            } => self.hub.send(HubEvent::TitleChanged { key: window, title }),
            Observation::Title { title: None, .. } => {}
            Observation::SizeLimits { window, limits } => {
                if self.registry.get(window).is_some_and(|entry| {
                    entry.admitted.is_some() && entry.fullscreen != Some(FullscreenKind::Exclusive)
                }) {
                    self.hub.send(HubEvent::SizeConstraint {
                        key: window,
                        limits,
                    });
                }
            }
            Observation::CornerRadius { window, radius } => {
                if let Some(entry) = self.registry.windows.get_mut(&window) {
                    entry.corner_radius = radius;
                }
            }
            Observation::Settled { window, read } => self.settle(windows, window, read, now),
        }
    }

    fn settle(
        &mut self,
        windows: &mut dyn WindowServer,
        key: HwndId,
        read: SettleRead,
        _now: Instant,
    ) {
        let Some(entry) = self.registry.windows.get_mut(&key) else {
            return;
        };
        entry.is_maximized = read.is_maximized;
        if let Some((_, monitor)) = read.placement {
            let changed = entry.monitor != monitor;
            entry.monitor = monitor;
            if changed
                && entry.admitted.is_some()
                && entry.fullscreen != Some(FullscreenKind::Exclusive)
            {
                windows.request_size_limits(vec![SizeLimitRead { window: key }]);
            }
        }
        if read.minimized {
            if !entry.dome_minimized && !entry.minimized {
                entry.minimized = true;
                self.hub.send(HubEvent::WindowMinimized(key));
            }
            return;
        }
        if entry.minimized {
            entry.minimized = false;
            self.hub.send(HubEvent::WindowRestored(key));
        }
        let Some((frame, monitor)) = read.placement else {
            return;
        };
        if entry.admitted.is_none() || entry.fullscreen == Some(FullscreenKind::Exclusive) {
            return;
        }
        if entry
            .placement
            .placed_at()
            .is_some_and(|at| read.observed_at < at)
        {
            return;
        }
        let shaped = window::borderless(frame, monitor, &self.monitors);
        let assigned = entry.target.and_then(PlacementTarget::frame) == Some(frame);
        if shaped && !assigned {
            if entry.dome_minimized {
                if entry.placement.allow_correction(true) {
                    self.recovery.track(key, entry.is_maximized);
                    windows.minimize(key);
                }
            } else if entry.fullscreen != Some(FullscreenKind::Borderless) {
                entry.fullscreen = Some(FullscreenKind::Borderless);
                entry.awaiting_response = true;
                self.hub.send(HubEvent::FullscreenChanged {
                    key,
                    fullscreen: entry.fullscreen,
                });
                if entry.target == Some(PlacementTarget::Hidden) {
                    entry.placement.reset(PlacementTarget::Hidden);
                    self.recovery.track(key, entry.is_maximized);
                    windows.minimize(key);
                    entry.dome_minimized = true;
                }
            }
            return;
        }
        if entry.fullscreen == Some(FullscreenKind::Borderless) && !shaped {
            entry.fullscreen = None;
            entry.actual = frame;
            entry.awaiting_response = true;
            self.hub.send(HubEvent::FullscreenChanged {
                key,
                fullscreen: None,
            });
            if entry.target == Some(PlacementTarget::Hidden) {
                entry.awaiting_response = false;
                entry.placement.reset(PlacementTarget::Hidden);
                self.recovery.track(key, entry.is_maximized);
                if entry.dome_minimized {
                    windows.unminimize(key);
                    entry.dome_minimized = false;
                }
                windows.move_offscreen(key);
            }
            return;
        }
        if entry.awaiting_response {
            return;
        }
        entry.actual = frame;
        match entry.target {
            Some(
                PlacementTarget::Tiling { frame: target, .. }
                | PlacementTarget::Fullscreen { frame: target, .. },
            ) if frame != target => {
                if entry.placement.allow_correction(true) {
                    self.recovery.track(key, entry.is_maximized);
                    windows.set_frame(key, target);
                }
            }
            Some(PlacementTarget::Float { .. }) => {
                if self.monitors.iter().any(|m| m.native == monitor) {
                    self.hub.send(HubEvent::FloatMoved {
                        key,
                        frame,
                        monitor,
                    });
                }
            }
            Some(PlacementTarget::Hidden)
                if frame.x() > OFFSCREEN_POS
                    && frame.y() > OFFSCREEN_POS
                    && entry.placement.allow_correction(false) =>
            {
                self.recovery.track(key, entry.is_maximized);
                windows.move_offscreen(key);
            }
            _ => {}
        }
    }

    pub(in crate::platform::windows) fn drain_intake(
        &mut self,
        windows: &mut dyn WindowServer,
    ) -> ControlFlow<(), (Option<Scene>, Vec<WindowCommand>)> {
        let mut scene = None;
        let mut commands = Vec::new();
        while let Some(message) = self
            .intake
            .as_ref()
            .and_then(|intake| intake.try_recv().ok())
        {
            match message {
                HubMessage::Shutdown => return ControlFlow::Break(()),
                HubMessage::Scene(next) => {
                    if self.focus.request_changed(next.focus_request) {
                        self.overlay_focus_request = self.focus.on_scene_focus(next.focus_request);
                    }
                    scene = Some(next);
                }
                HubMessage::Command(command) => commands.push(command),
                HubMessage::AppearanceChanged(appearance) => {
                    self.internal.set_appearance(&appearance)
                }
                HubMessage::DisplayRefreshed(monitors) => {
                    if !monitors.is_empty() {
                        windows.observe_displays(&monitors);
                        for monitor in &mut self.monitors {
                            if let Some(info) = monitors.iter().find(|m| m.handle == monitor.native)
                            {
                                monitor.system_work_area = info.work_area;
                                monitor.scale = info.scale;
                            }
                        }
                        windows.request_size_limits(
                            self.registry
                                .windows
                                .iter()
                                .filter(|(_, entry)| {
                                    entry.admitted.is_some()
                                        && entry.fullscreen != Some(FullscreenKind::Exclusive)
                                })
                                .map(|(&window, _)| SizeLimitRead { window })
                                .collect(),
                        );
                    }
                }
            }
        }
        ControlFlow::Continue((scene, commands))
    }

    pub(in crate::platform::windows) fn apply_placements(
        &mut self,
        windows: &mut dyn WindowServer,
        scene: &Scene,
        now: Instant,
    ) {
        self.monitors = scene.monitors.clone();
        self.workspaces = scene.workspaces.clone();
        if !self.monitors.is_empty()
            && let Some(pending) = self.observations_before_monitor_policy.take()
        {
            for observation in pending {
                self.observe(windows, observation, now);
            }
        }
        for (&key, entry) in &mut self.registry.windows {
            let Some(expected) = scene.windows.get(&key) else {
                entry.admitted = None;
                entry.target = None;
                entry.awaiting_response = false;
                continue;
            };
            if entry.admitted != Some(expected.id) {
                entry.target = None;
                if entry.fullscreen != Some(FullscreenKind::Exclusive) {
                    windows.request_size_limits(vec![SizeLimitRead { window: key }]);
                }
            }
            entry.admitted = Some(expected.id);
            if let Some(target) =
                PlacementTarget::from_placement(expected.placement, &self.monitors)
            {
                window::apply_target(entry, key, target, now, &mut self.recovery, windows);
            }
        }
        self.validate_focus();
    }

    fn validate_focus(&mut self) {
        if let Some(key) = self.focus.pending()
            && self.registry.get(key).is_none_or(|entry| {
                entry.admitted.is_none() || entry.fullscreen == Some(FullscreenKind::Exclusive)
            })
        {
            self.focus.clear();
        }
    }

    pub(in crate::platform::windows) fn apply_command(
        &mut self,
        windows: &mut dyn WindowServer,
        command: WindowCommand,
    ) {
        let key = match command {
            WindowCommand::Close(key) | WindowCommand::Unminimize(key) => key,
        };
        let Some(entry) = self
            .registry
            .get(key)
            .filter(|e| e.admitted.is_some() && e.fullscreen != Some(FullscreenKind::Exclusive))
        else {
            return;
        };
        self.recovery.track(key, entry.is_maximized);
        match command {
            WindowCommand::Close(key) => windows.close(key),
            WindowCommand::Unminimize(key) => windows.unminimize(key),
        }
    }

    pub(in crate::platform::windows) fn show_internal_windows(&mut self, scene: &Scene) {
        self.internal.set_monitors(&scene.monitors);
        let mut floats = Vec::new();
        for monitor in &scene.monitors {
            let tiling: Vec<_> = monitor
                .tiling_windows
                .iter()
                .filter(|p| !p.content_box.is_empty())
                .filter_map(|placement| {
                    let (_, entry) = self.registry.by_id(placement.id)?;
                    Some(TilingWindowShow {
                        placement: *placement,
                        corner_radius: entry.corner_radius,
                    })
                })
                .collect();
            let containers: Vec<_> = monitor
                .containers
                .iter()
                .filter(|placement| placement.is_tabbed || placement.is_highlighted)
                .cloned()
                .collect();
            self.internal.show_tiling(monitor.id, &tiling, &containers);
            for placement in monitor
                .float_windows
                .iter()
                .filter(|p| !p.content_box.is_empty())
            {
                if let Some((window, entry)) = self.registry.by_id(placement.id) {
                    floats.push(FloatShow {
                        window,
                        placement: *placement,
                        scale: monitor.scale,
                        border_thickness: monitor.border_thickness,
                        corner_radius: entry.corner_radius,
                    });
                }
            }
        }
        self.internal.show_floats(&floats);
        self.internal.show_tab_bars(&scene.monitors);
    }

    pub(in crate::platform::windows) fn apply_overlay_focus(
        &mut self,
        windows: &mut dyn WindowServer,
    ) {
        if let Some(monitor) = self.overlay_focus_request.take() {
            self.focus.clear();
            windows.invalidate_foreground();
            self.internal.focus_tiling_overlay(monitor);
        }
    }

    pub(in crate::platform::windows) fn restack(
        &mut self,
        windows: &mut dyn WindowServer,
        scene: &Scene,
    ) {
        use crate::platform::windows::dome::events::WindowPlacement;
        if let Some(previous) = &self.previous_scene {
            for (&key, expected) in &previous.windows {
                if matches!(expected.placement, WindowPlacement::Float { .. })
                    && scene.windows.get(&key).is_some_and(|next| {
                        !matches!(next.placement, WindowPlacement::Float { .. })
                    })
                    && let Some(entry) = self.registry.get(key).filter(|e| {
                        e.admitted.is_some() && e.fullscreen != Some(FullscreenKind::Exclusive)
                    })
                {
                    self.recovery.track(key, entry.is_maximized);
                    windows.set_z_order(key, ZOrder::NotTopmost);
                }
            }
        }
        for monitor in &scene.monitors {
            for placement in monitor
                .float_windows
                .iter()
                .filter(|p| !p.content_box.is_empty())
            {
                let Some((key, entry)) = self.registry.by_id(placement.id) else {
                    continue;
                };
                if entry.fullscreen == Some(FullscreenKind::Exclusive) {
                    continue;
                }
                let previous = self
                    .previous_scene
                    .as_ref()
                    .and_then(|s| s.windows.get(&key));
                let old_frame = previous.and_then(|e| match e.placement {
                    WindowPlacement::Float { placement, .. } => Some(placement.content_box),
                    _ => None,
                });
                let newly_focused = scene.focused_window == Some(key)
                    && self
                        .previous_scene
                        .as_ref()
                        .is_none_or(|s| s.focused_window != Some(key));
                if old_frame.is_none() || newly_focused || old_frame != Some(placement.content_box)
                {
                    self.recovery.track(key, entry.is_maximized);
                    if old_frame.is_none() || newly_focused {
                        windows.set_z_order(key, ZOrder::Topmost);
                    }
                    self.internal
                        .stack(InternalWindow::FloatOverlay(key), ZOrder::After(key));
                }
            }
            if let Some(reference) = monitor
                .tiling_windows
                .iter()
                .find(|p| p.is_highlighted && !p.content_box.is_empty())
                .and_then(|p| self.registry.by_id(p.id).map(|(key, _)| key))
            {
                self.internal.stack(
                    InternalWindow::TilingOverlay(monitor.id),
                    ZOrder::After(reference),
                );
                for placement in monitor
                    .tiling_windows
                    .iter()
                    .filter(|p| !p.is_highlighted && !p.content_box.is_empty())
                {
                    if let Some((key, entry)) = self.registry.by_id(placement.id)
                        && entry.fullscreen != Some(FullscreenKind::Exclusive)
                    {
                        self.recovery.track(key, entry.is_maximized);
                        windows.set_z_order(key, ZOrder::After(reference));
                    }
                }
            }
        }
        self.previous_scene = Some(scene.clone());
    }

    pub(in crate::platform::windows) fn submit_pending_focus(
        &mut self,
        windows: &mut dyn WindowServer,
        now: Instant,
    ) {
        self.validate_focus();
        if self.focus.due(now)
            && let Some(key) = self.focus.pending()
            && let Some(entry) = self.registry.get(key)
        {
            self.recovery.track(key, entry.is_maximized);
        }
        self.focus.submit_pending_focus(now, windows);
    }
    pub(in crate::platform::windows) fn focus_deadline(&self, now: Instant) -> Option<Instant> {
        self.focus.next_deadline(now)
    }
    pub(in crate::platform::windows) fn recover(&mut self, windows: &mut dyn WindowServer) {
        if self.recovery_started {
            return;
        }
        self.recovery_started = true;
        self.intake = None;
        self.observations_before_monitor_policy = None;
        self.focus.clear();
        self.overlay_focus_request = None;
        windows.shutdown(self.recovery.take());
    }
}
