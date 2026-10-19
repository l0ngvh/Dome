use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::config::Keystroke;
use crate::platform::macos::corner_radius::FALLBACK_CORNER_RADIUS;
use crate::platform::macos::dome::{DebounceBurst, Dome, ManagedWindow, WindowChange, WindowMove};
use dome_core::WindowId;

use crate::platform::macos::window_server::{
    AppObservation, WindowNotification, WindowObservation,
};

pub(in crate::platform::macos) enum HubEvent {
    RunBinding {
        keymap: String,
        keystroke: Keystroke,
    },
}

impl fmt::Display for HubEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunBinding { keymap, keystroke } => {
                write!(f, "RunBinding(keymap={keymap}, keystroke={keystroke})")
            }
        }
    }
}

impl Dome {
    pub(in crate::platform::macos) fn handle_window_notification(
        &mut self,
        notification: WindowNotification,
    ) {
        match notification {
            WindowNotification::Title { id, title } => self.update_title(id, title),
            WindowNotification::AppWindows(app) => {
                self.observe_placement_starts(&app);
                let tracked = self.tracked_for_pid(app.pid);
                self.reconcile_windows(reconcile(&app, &tracked));
            }
            WindowNotification::Movement { app, observed_at } => {
                self.observe_placement_starts(&app);
                let tracked = self.tracked_for_pid(app.pid);
                apply_positions(app, tracked, observed_at, self);
            }
            WindowNotification::AllWindows(system) => {
                for app in &system.apps {
                    self.observe_placement_starts(app);
                }
                let tracked = self.all_tracked();
                let changes = system
                    .apps
                    .iter()
                    .flat_map(|app| reconcile(app, &tracked))
                    .collect();
                self.reconcile_all_windows(system.terminated_pids, system.hidden_pids, changes);
            }
            WindowNotification::Focus(id) => self.focus_window(id),
            WindowNotification::NativeFullscreen(focused) => {
                let id = focused.id;
                if focused.is_native_fullscreen == Some(true) {
                    self.enter_native_fullscreen(id, focused.pid, focused.metadata);
                } else if focused.is_native_fullscreen == Some(false)
                    && let (Some(position), Some(size)) = (focused.position, focused.size)
                {
                    self.exit_native_fullscreen(id, position, size);
                }
            }
            WindowNotification::MovementStarted { pid } => self.set_pid_moving(pid, true),
            WindowNotification::MovementEnded { pid } => self.set_pid_moving(pid, false),
            WindowNotification::AppRemoved { pid } => self.app_terminated(pid),
            WindowNotification::WindowRemoved(id) => {
                self.reconcile_windows([WindowChange::Removed(id)])
            }
        }
    }
}

impl Dome {
    fn observe_placement_starts(&mut self, app: &AppObservation) {
        for window in app.listed.iter().chain(app.requested.values()) {
            self.observe_placement_start(window.id, window.placement);
        }
    }

    fn reconcile_all_windows(
        &mut self,
        terminated_pids: Vec<i32>,
        hidden_pids: Vec<i32>,
        changes: Vec<WindowChange>,
    ) {
        for &pid in &terminated_pids {
            self.remove_untracked_app(pid);
        }
        for &pid in &hidden_pids {
            self.remove_untracked_app(pid);
        }
        self.reconcile_windows(changes);
    }
}

fn is_manageable(window: &WindowObservation) -> bool {
    let reason = if window.metadata.title.is_none() {
        "no title"
    } else if window.role.as_deref() != Some("AXWindow") {
        "role is not AXWindow"
    } else if window.subrole.as_deref() != Some("AXStandardWindow") {
        "subrole is not standard"
    } else if !window.is_root {
        "not a root window"
    } else if !window.position_settable {
        "position not settable"
    } else if !window.size_settable {
        "size not settable"
    } else if !window.main_settable {
        "main not settable"
    } else if window.minimized {
        "minimized"
    } else {
        return true;
    };
    crate::trace_once!(
        key: (window.id, window.pid, reason),
        window_id = %window.id,
        pid = window.pid,
        role = ?window.role,
        subrole = ?window.subrole,
        "not manageable: {reason}"
    );
    false
}

fn reconcile(
    app: &AppObservation,
    tracked: &HashMap<WindowId, ManagedWindow>,
) -> Vec<WindowChange> {
    let listed: HashSet<_> = app.listed.iter().map(|window| window.id).collect();
    let mut refresh = HashSet::new();
    let mut removed = Vec::new();
    let mut minimized = Vec::new();
    let mut entered = Vec::new();
    let mut exited = Vec::new();
    for (&id, entry) in tracked.iter().filter(|(id, entry)| {
        entry.pid == app.pid && (app.requested_ids.contains(id) || listed.contains(id))
    }) {
        let Some(window) = app.requested.get(&id) else {
            if listed.contains(&id) {
                refresh.insert(id);
            } else {
                removed.push(WindowChange::Removed(id));
            }
            continue;
        };
        if !window.valid {
            if listed.contains(&id) {
                refresh.insert(id);
            } else {
                removed.push(WindowChange::Removed(id));
            }
            continue;
        }
        if !entry.is_minimized() && window.minimized {
            minimized.push(WindowChange::Minimized(id));
            continue;
        }
        if entry.is_minimized() {
            continue;
        }
        if !entry.is_native_fullscreen() && window.native_fullscreen == Some(true) {
            entered.push(WindowChange::EnteredNativeFullscreen(id));
        } else if entry.is_native_fullscreen()
            && window.native_fullscreen == Some(false)
            && let Some(rect) = window.rect
        {
            exited.push(WindowChange::ExitedNativeFullscreen {
                window_id: id,
                rect,
                corner_radius: window.corner_radius.unwrap_or(entry.corner_radius),
            });
        }
    }
    let mut refreshed = Vec::new();
    let mut added = Vec::new();
    for window in &app.listed {
        let id = window.id;
        if refresh.contains(&id) {
            refreshed.push(WindowChange::Refreshed {
                window_id: id,
                corner_radius: window.corner_radius.unwrap_or(tracked[&id].corner_radius),
            });
            continue;
        }
        if tracked.contains_key(&id) || !is_manageable(window) {
            continue;
        }
        if window.native_fullscreen == Some(true) {
            added.push(WindowChange::AddedNativeFullscreen {
                window_id: window.id,
                pid: window.pid,
                metadata: window.metadata.clone(),
            });
        } else if let Some(rect) = window.rect {
            added.push(WindowChange::Added {
                window_id: window.id,
                pid: window.pid,
                metadata: window.metadata.clone(),
                rect,
                corner_radius: window.corner_radius.unwrap_or(FALLBACK_CORNER_RADIUS),
            });
        }
    }
    refreshed
        .into_iter()
        .chain(removed)
        .chain(minimized)
        .chain(added)
        .chain(entered)
        .chain(exited)
        .collect()
}

fn apply_positions(
    app: AppObservation,
    tracked: HashMap<WindowId, ManagedWindow>,
    observed_at: DebounceBurst,
    dome: &mut Dome,
) {
    let mut moves = Vec::new();
    for listed in app.listed {
        let id = listed.id;
        if !tracked.contains_key(&id) {
            continue;
        }
        let Some(window) = app.requested.get(&id) else {
            continue;
        };
        if !window.valid || window.minimized || window.native_fullscreen != Some(false) {
            continue;
        }
        if let Some(rect) = window.rect {
            if let Some(radius) = window.corner_radius {
                dome.update_corner_radius(id, radius);
            }
            moves.push(WindowMove {
                window_id: id,
                rect,
                observed_at,
            });
        }
    }
    dome.windows_moved(moves);
}
