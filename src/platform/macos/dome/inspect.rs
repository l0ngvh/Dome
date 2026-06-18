use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use objc2_core_graphics::CGWindowID;

use crate::config::MacosWindow;
use crate::platform::macos::accessibility::{AXApp, ExternalWindow, RejectionReason};
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::dome::registry::ManagedWindow;
use crate::platform::macos::dome::rejection_log_filter::RejectionLogFilter;
use crate::platform::macos::dome::window::{RoundedDimension, WindowState};
use crate::platform::macos::dome::{NewWindow, PendingAdd};
use crate::platform::macos::running_application::RunningApp;

/// A window currently returned by the app's `kAXWindowsAttribute` query
/// (includes minimized, excludes windows on other Spaces).
pub(in crate::platform::macos) struct ExistingWindow {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
}

pub(in crate::platform::macos) struct ExitNativeFullscreen {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
}

pub(in crate::platform::macos) struct ReconcileResult {
    pub(in crate::platform::macos) to_remove: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_minimize: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_add: Vec<PendingAdd>,
    pub(in crate::platform::macos) to_enter_native_fullscreen: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_exit_native_fullscreen: Vec<ExitNativeFullscreen>,
    pub(in crate::platform::macos) refresh: Vec<ExtRefresh>,
}

pub(in crate::platform::macos) struct ReconcileAllResult {
    pub(in crate::platform::macos) terminated_pids: Vec<i32>,
    pub(in crate::platform::macos) hidden_pids: Vec<i32>,
    pub(in crate::platform::macos) to_remove: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_minimize: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_add: Vec<PendingAdd>,
    pub(in crate::platform::macos) to_enter_native_fullscreen: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_exit_native_fullscreen: Vec<ExitNativeFullscreen>,
    pub(in crate::platform::macos) refresh: Vec<ExtRefresh>,
}

pub(in crate::platform::macos) struct ExtRefresh {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) ext: Arc<dyn ExternalWindow>,
}

pub(in crate::platform::macos) fn compute_reconciliation(
    app: &Arc<AXApp>,
    tracked: &HashMap<CGWindowID, ManagedWindow>,
    ignore_rules: &[MacosWindow],
    log_filter: &RejectionLogFilter,
    marker: &DispatcherMarker,
) -> ReconcileResult {
    let pid = app.pid();

    let ax_windows = match app.clone().windows(marker) {
        Ok(list) => list,
        Err(e) => {
            // AX is unavailable (lock screen, suspended, or transient error).
            // Keep all tracked entries unchanged this pass.
            tracing::trace!(%pid, "Failed to retrieve list of windows for {app}: {e}");
            return ReconcileResult {
                to_remove: Vec::new(),
                to_minimize: Vec::new(),
                to_add: Vec::new(),
                to_enter_native_fullscreen: Vec::new(),
                to_exit_native_fullscreen: Vec::new(),
                refresh: Vec::new(),
            };
        }
    };

    let cg_ids_in_app: HashSet<CGWindowID> = ax_windows.iter().map(|w| w.cg_id()).collect();

    let mut to_remove = Vec::new();
    let mut to_minimize = Vec::new();
    let mut to_enter_native_fullscreen = Vec::new();
    let mut to_exit_native_fullscreen = Vec::new();
    let mut needs_refresh: HashSet<CGWindowID> = HashSet::new();

    for (&cg_id, entry) in tracked.iter().filter(|(_, e)| e.ext.pid() == pid) {
        // Since AXApp::windows was successful earlier, we can assume that the system hasn't been
        // suspended.
        if !entry.ext.is_valid(marker) {
            // In cases the window got invalidated due to Cocoa recycle windows views.
            //
            // Also, in rare cases where the system got suspend and AX is unavailable after
            // AXApp::windows but before is_valid check, the window handle just gets reset rather
            // than being deleted, no harm is done.
            if cg_ids_in_app.contains(&cg_id) {
                needs_refresh.insert(cg_id);
            } else {
                to_remove.push(cg_id);
                continue;
            }
        }

        let already_minimized =
            entry.is_minimized || matches!(entry.state, WindowState::BorderlessMinimized { .. });
        if !already_minimized && entry.ext.is_minimized(marker) {
            to_minimize.push(cg_id);
            continue;
        }
        if already_minimized {
            continue;
        }
        // macOS does not reliably emit kAXMoved/kAXResized on native fullscreen
        // enter/exit, and SpaceChanged only covers the frontmost focused window,
        // so the periodic reconcile cycle is the only reliable signal for
        // non-focused windows transitioning in/out of native fullscreen.
        let is_fs = entry.ext.is_native_fullscreen(marker);
        if !matches!(entry.state, WindowState::NativeFullscreen) && is_fs {
            to_enter_native_fullscreen.push(cg_id);
        } else if matches!(entry.state, WindowState::NativeFullscreen) && !is_fs {
            let Ok((x, y)) = entry.ext.get_position(marker) else {
                tracing::trace!(%entry, "native fullscreen exit: position read failed, skipping");
                continue;
            };
            let Ok((w, h)) = entry.ext.get_size(marker) else {
                tracing::trace!(%entry, "native fullscreen exit: size read failed, skipping");
                continue;
            };
            to_exit_native_fullscreen.push(ExitNativeFullscreen {
                cg_id,
                x: x.value() as i32,
                y: y.value() as i32,
                w: w.value() as i32,
                h: h.value() as i32,
            });
        }
    }

    let mut to_add = Vec::new();
    let mut refresh = Vec::new();
    for ax in ax_windows {
        let cg_id = ax.cg_id();
        if needs_refresh.contains(&cg_id) {
            refresh.push(ExtRefresh {
                cg_id,
                ext: Arc::new(ax),
            });
            continue;
        }
        if tracked.contains_key(&cg_id) {
            continue;
        }
        if let Some(reason) = ax.check_unmanageable() {
            let pid = ax.pid();
            if log_filter.record_and_should_log(cg_id, pid, reason, Instant::now()) {
                tracing::trace!(window = %ax, ?reason, "not manageable");
            }
            continue;
        }
        let app_name = ax.app_name().map(str::to_owned);
        let bundle_id = ax.bundle_id().map(str::to_owned);
        let title = ax.title().map(str::to_owned);
        let new = NewWindow {
            ax: Arc::new(ax),
            app_name,
            bundle_id,
            title,
        };
        if should_ignore(&new, ignore_rules, cg_id, pid, log_filter) {
            continue;
        }
        if new.ax.is_native_fullscreen(marker) {
            to_add.push(PendingAdd::NativeFullscreen { new });
            continue;
        }
        let Ok((x, y)) = new.ax.get_position(marker) else {
            continue;
        };
        let Ok((w, h)) = new.ax.get_size(marker) else {
            continue;
        };
        to_add.push(PendingAdd::Positioned {
            new,
            dim: RoundedDimension {
                x: x.value() as i32,
                y: y.value() as i32,
                width: w.value() as i32,
                height: h.value() as i32,
            },
        });
    }

    ReconcileResult {
        to_remove,
        to_minimize,
        to_add,
        to_enter_native_fullscreen,
        to_exit_native_fullscreen,
        refresh,
    }
}

pub(in crate::platform::macos) fn compute_window_positions(
    app: &Arc<AXApp>,
    tracked: &HashMap<CGWindowID, ManagedWindow>,
    marker: &DispatcherMarker,
) -> Vec<ExistingWindow> {
    let mut existing = Vec::new();
    let ax_windows = match app.clone().windows(marker) {
        Ok(ax_windows) => ax_windows,
        Err(e) => {
            tracing::trace!("Failed to retrieve list of windows for {app}: {e}");
            return Vec::new();
        }
    };
    for ax in ax_windows {
        let cg_id = ax.cg_id();
        if let Some(window) = tracked.get(&cg_id)
            && let Some(observed) = read_existing_window(window, cg_id, marker)
        {
            existing.push(observed);
        }
    }
    existing
}

fn read_existing_window(
    window: &ManagedWindow,
    cg_id: CGWindowID,
    marker: &DispatcherMarker,
) -> Option<ExistingWindow> {
    // This window is minimized before the move/resize event is processed
    if window.ext.is_minimized(marker) {
        return None;
    }
    // Skip native fullscreen: window_moved's rect-shape heuristic misreads the
    // macOS native-FS frame as non-fullscreen and would clear ProtectFullscreen.
    if window.ext.is_native_fullscreen(marker) {
        return None;
    }
    let (x, y) = window.ext.get_position(marker).ok()?;
    let (w, h) = window.ext.get_size(marker).ok()?;
    Some(ExistingWindow {
        cg_id,
        x: x.value() as i32,
        y: y.value() as i32,
        w: w.value() as i32,
        h: h.value() as i32,
    })
}

pub(in crate::platform::macos) fn compute_reconcile_all(
    observed_pids: HashSet<i32>,
    tracked: HashMap<CGWindowID, ManagedWindow>,
    ignore_rules: Vec<MacosWindow>,
    log_filter: Arc<RejectionLogFilter>,
    marker: &DispatcherMarker,
) -> ReconcileAllResult {
    log_filter.prune(Instant::now());

    let running: Vec<_> = RunningApp::all().collect();
    let running_pids: HashSet<_> = running.iter().map(|app| app.pid()).collect();

    let terminated_pids: Vec<_> = observed_pids
        .iter()
        .filter(|pid| !running_pids.contains(pid))
        .copied()
        .collect();

    let mut hidden_pids = Vec::new();
    let mut to_remove = Vec::new();
    let mut to_minimize = Vec::new();
    let mut to_add = Vec::new();
    let mut to_enter_native_fullscreen = Vec::new();
    let mut to_exit_native_fullscreen = Vec::new();
    let mut refresh = Vec::new();
    for app in &running {
        if app.is_hidden() {
            hidden_pids.push(app.pid());
        } else {
            let ax_app = app.ax_app();
            let result =
                compute_reconciliation(&ax_app, &tracked, &ignore_rules, &log_filter, marker);
            to_remove.extend(result.to_remove);
            to_minimize.extend(result.to_minimize);
            to_add.extend(result.to_add);
            to_enter_native_fullscreen.extend(result.to_enter_native_fullscreen);
            to_exit_native_fullscreen.extend(result.to_exit_native_fullscreen);
            refresh.extend(result.refresh);
        }
    }

    // Refresh the cached kAXEnhancedUserInterfaceAttribute probe on tracked
    // windows, deduped by PID so each app is probed at most once.
    let mut refreshed_pids = HashSet::new();
    for entry in tracked.values() {
        if refreshed_pids.insert(entry.ext.pid()) {
            entry.ext.refresh_enhanced_ui(marker);
        }
    }

    ReconcileAllResult {
        terminated_pids,
        hidden_pids,
        to_remove,
        to_minimize,
        to_add,
        to_enter_native_fullscreen,
        to_exit_native_fullscreen,
        refresh,
    }
}

fn should_ignore(
    new: &NewWindow,
    rules: &[MacosWindow],
    cg_id: CGWindowID,
    pid: i32,
    log_filter: &RejectionLogFilter,
) -> bool {
    let matched = rules.iter().find(|r| {
        r.matches(
            new.app_name.as_deref(),
            new.bundle_id.as_deref(),
            new.title.as_deref(),
        )
    });
    if matched.is_some() {
        if log_filter.record_and_should_log(
            cg_id,
            pid,
            RejectionReason::IgnoredByRule,
            Instant::now(),
        ) {
            tracing::trace!(%cg_id, %pid, "not manageable");
        }
        return true;
    }
    false
}
