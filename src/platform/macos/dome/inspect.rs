use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFString, CFType};
use objc2_core_graphics::{CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};

use crate::config::MacosWindow;
use crate::platform::macos::accessibility::AXApp;
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::dome::NewWindow;
use crate::platform::macos::dome::registry::WindowEntry;
use crate::platform::macos::dome::window::WindowState;
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;
use crate::platform::macos::running_application::RunningApp;

/// A still in display window (unminimized, in current space, returned by AXWindowsAttribute)
pub(in crate::platform::macos) struct ExistingWindow {
    pub(in crate::platform::macos) cg_id: CGWindowID,
    pub(in crate::platform::macos) x: i32,
    pub(in crate::platform::macos) y: i32,
    pub(in crate::platform::macos) w: i32,
    pub(in crate::platform::macos) h: i32,
    pub(in crate::platform::macos) is_native_fullscreen: bool,
}

pub(in crate::platform::macos) struct ReconcileAllResult {
    pub(in crate::platform::macos) terminated_pids: Vec<i32>,
    pub(in crate::platform::macos) hidden_pids: Vec<i32>,
    pub(in crate::platform::macos) to_remove: Vec<CGWindowID>,
    pub(in crate::platform::macos) to_add: Vec<NewWindow>,
}

pub(in crate::platform::macos) fn compute_reconciliation(
    app: &Arc<AXApp>,
    tracked: &HashMap<CGWindowID, WindowEntry>,
    ignore_rules: &[MacosWindow],
    marker: &DispatcherMarker,
) -> (Vec<CGWindowID>, Vec<NewWindow>) {
    let pid = app.pid();
    let cg_window_ids = list_cg_window_ids();

    let mut to_remove = Vec::new();
    for (&cg_id, entry) in tracked.iter().filter(|(_, e)| e.ax.pid() == pid) {
        if !cg_window_ids.contains(&cg_id)
            || !entry.ax.is_valid(marker)
        // Skip minimized check for windows Dome minimized (borderless fullscreen hiding)
            || (!matches!(entry.state, WindowState::Minimized) && entry.ax.is_minimized(marker))
        {
            to_remove.push(cg_id);
        }
    }

    let mut to_add = Vec::new();

    let ax_windows = match app.clone().windows(marker) {
        Ok(ax_windows) => ax_windows,
        Err(e) => {
            tracing::trace!("Failed to retrieve list of windows for {app}: {e}");
            return (to_remove, Vec::new());
        }
    };
    for ax in ax_windows {
        if tracked.contains_key(&ax.cg_id()) {
            continue;
        }
        if !ax.is_manageable() {
            continue;
        }
        let app_name = ax.app_name().map(str::to_owned);
        let bundle_id = ax.bundle_id().map(str::to_owned);
        let title = ax.title().map(str::to_owned);
        if should_ignore(
            app_name.as_deref(),
            bundle_id.as_deref(),
            title.as_deref(),
            ignore_rules,
        ) {
            continue;
        }
        let Ok((x, y)) = ax.get_position() else {
            continue;
        };
        let Ok((w, h)) = ax.get_size() else {
            continue;
        };
        to_add.push(NewWindow {
            x,
            y,
            w,
            h,
            is_native_fullscreen: ax.is_native_fullscreen(),
            app_name,
            bundle_id,
            title,
            ax: Arc::new(ax),
        });
    }

    (to_remove, to_add)
}

pub(in crate::platform::macos) fn compute_window_positions(
    app: &Arc<AXApp>,
    tracked: &HashMap<CGWindowID, WindowEntry>,
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
        let Some(window) = tracked.get(&ax.cg_id()) else {
            continue;
        };
        let Ok((x, y)) = window.ax.get_position(marker) else {
            continue;
        };
        let Ok((w, h)) = window.ax.get_size(marker) else {
            continue;
        };
        existing.push(ExistingWindow {
            cg_id: ax.cg_id(),
            x,
            y,
            w,
            h,
            is_native_fullscreen: window.ax.is_native_fullscreen(marker),
        });
    }
    existing
}

pub(in crate::platform::macos) fn compute_reconcile_all(
    observed_pids: HashSet<i32>,
    tracked: HashMap<CGWindowID, WindowEntry>,
    ignore_rules: Vec<MacosWindow>,
    marker: &DispatcherMarker,
) -> ReconcileAllResult {
    let running: Vec<_> = RunningApp::all().collect();
    let running_pids: HashSet<_> = running.iter().map(|app| app.pid()).collect();

    let terminated_pids: Vec<_> = observed_pids
        .iter()
        .filter(|pid| !running_pids.contains(pid))
        .copied()
        .collect();

    let mut hidden_pids = Vec::new();
    let mut to_remove = Vec::new();
    let mut to_add = Vec::new();
    for app in &running {
        if app.is_hidden() {
            hidden_pids.push(app.pid());
        } else {
            let ax_app = app.ax_app();
            let (removed, added) = compute_reconciliation(&ax_app, &tracked, &ignore_rules, marker);
            to_remove.extend(removed);
            to_add.extend(added);
        }
    }

    // Refresh the cached kAXEnhancedUserInterfaceAttribute probe on tracked
    // windows, deduped by PID so each app is probed at most once.
    let mut refreshed_pids = HashSet::new();
    for entry in tracked.values() {
        if refreshed_pids.insert(entry.ax.pid()) {
            entry.ax.refresh_enhanced_ui(marker);
        }
    }

    ReconcileAllResult {
        terminated_pids,
        hidden_pids,
        to_remove,
        to_add,
    }
}

fn should_ignore(
    app_name: Option<&str>,
    bundle_id: Option<&str>,
    title: Option<&str>,
    rules: &[MacosWindow],
) -> bool {
    let matched = rules.iter().find(|r| r.matches(app_name, bundle_id, title));
    if let Some(rule) = matched {
        tracing::debug!(?app_name, ?title, ?rule, "Window ignored by rule");
        return true;
    }
    false
}

fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}
